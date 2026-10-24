//! The departure suite (ADR-33 D1 to D3, GDD 11.15): Standard and Full keep the aircraft out of
//! a departure whatever the pilot does; Reduced and Manual can be put into one and RECOVER brings
//! them out.

mod common;

use common::*;
use fcs::{Assist, PilotInput};
use flight::actuators::Surfaces;
use flight::env::SphereSea;

/// Conditions of D1: (speed m/s true, altitude m). About M0.4 at 5 km, M0.6 at 1 km and M0.9 at
/// 5 km.
const D1_CONDITIONS: [(f64, f64); 3] = [(128.0, 5_000.0), (204.0, 1_000.0), (288.0, 5_000.0)];
/// Conditions of D2: about 1.1 `V_s`, clean, 10 t.
const D2_CONDITIONS: [(f64, f64); 3] = [(105.0, 5_000.0), (100.0, 1_000.0), (150.0, 9_000.0)];

fn rig<'a>(
    model: &'a flight::aircraft::AircraftModel,
    fcs: &'a fcs::FcsModel,
    assist: Assist,
    speed: f64,
    alt: f64,
) -> Rig<'a> {
    Rig::trimmed(
        SphereSea::open(),
        model,
        fcs,
        assist,
        speed,
        alt,
        10_000.0,
        Surfaces::NEUTRAL,
    )
    .unwrap_or_else(|| panic!("{speed} m/s at {alt} m trims"))
}

#[test]
fn d1_standard_and_full_do_not_depart_under_full_aft_lateral_and_pedal() {
    let (model, fcs) = kestrel();
    let limit = model.limits.alpha_limit_rad.to_degrees();
    let mut worst = (0.0f64, 0.0f64);
    let mut failures = Vec::new();
    for assist in [Assist::Standard, Assist::Full] {
        for (speed, alt) in D1_CONDITIONS {
            for (roll, yaw) in [(1.0, 1.0), (-1.0, 1.0), (1.0, -1.0), (0.0, 1.0), (1.0, 0.0)] {
                let mut r = rig(&model, &fcs, assist, speed, alt);
                let data = r.run(10.0, |_, _| stick(1.0, roll, yaw));
                let name = format!("{assist:?} {speed} m/s {alt} m roll {roll} yaw {yaw}");
                let alpha = data
                    .iter()
                    .map(|d| d.alpha.abs())
                    .fold(0.0, f64::max)
                    .to_degrees();
                let yaw_rate = data
                    .iter()
                    .map(|d| d.yaw_rate_stability().abs())
                    .fold(0.0, f64::max)
                    .to_degrees();
                assert_eq!(r.faults, 0, "{name}");
                if alpha > limit + 2.0 || yaw_rate >= 30.0 {
                    failures.push(format!("{name}: alpha {alpha:.1}, yaw rate {yaw_rate:.1}"));
                }
                worst = (worst.0.max(alpha), worst.1.max(yaw_rate));
            }
        }
    }
    println!(
        "D1: worst alpha {:.1} deg, worst yaw rate {:.1} deg/s",
        worst.0, worst.1
    );
    assert!(failures.is_empty(), "departures:\n{}", failures.join("\n"));
}

/// The pro-spin entry: full aft stick, ailerons neutral, full rudder.
fn spin_entry() -> PilotInput {
    stick(1.0, 0.0, 1.0)
}

#[test]
fn d2_reduced_and_manual_depart_under_a_pro_spin_entry() {
    let (model, fcs) = kestrel();
    for assist in [Assist::Reduced, Assist::Manual] {
        for (speed, alt) in D2_CONDITIONS {
            let mut r = rig(&model, &fcs, assist, speed, alt);
            let data = r.run(12.0, |_, _| spin_entry());
            let name = format!("{assist:?} {speed} m/s {alt} m");
            let alpha = data
                .iter()
                .map(|d| d.alpha)
                .fold(f64::MIN, f64::max)
                .to_degrees();
            let rate = data
                .iter()
                .map(|d| d.rates.z.abs())
                .fold(0.0, f64::max)
                .to_degrees();
            assert_eq!(r.faults, 0, "{name}");
            assert!(
                alpha > 40.0 && rate > 40.0,
                "{name}: alpha {alpha:.1} r {rate:.1}"
            );
        }
    }
}

#[test]
fn d3_recover_brings_a_spin_back_within_fifteen_seconds() {
    let (model, fcs) = kestrel();
    let mut slowest = 0.0f64;
    for assist in [Assist::Reduced, Assist::Manual] {
        for (speed, alt) in D2_CONDITIONS {
            let mut r = rig(&model, &fcs, assist, speed, alt);
            // Six seconds into the spin the pilot lets go and holds RECOVER.
            r.run(6.0, |_, _| spin_entry());
            assert!(
                fcs::recover::departed(&r.sensed()),
                "in a spin at the start"
            );
            let recover = PilotInput {
                recover: true,
                ..PilotInput::default()
            };
            let name = format!("{assist:?} {speed} m/s {alt} m");
            let mut t_recovered = None;
            for i in 0..(15.0 / DT) as usize {
                let s = r.step(&recover);
                if s.rates.z.abs() < 10f64.to_radians() && s.alpha.abs() < 30f64.to_radians() {
                    t_recovered = Some(i as f64 * DT);
                    break;
                }
            }
            assert_eq!(r.faults, 0, "{name}");
            let t = t_recovered.unwrap_or_else(|| panic!("{name}: not recovered in 15 s"));
            slowest = slowest.max(t);
        }
    }
    println!("D3: slowest recovery {slowest:.1} s");
}
