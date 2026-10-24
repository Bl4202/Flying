//! The FCS handling suite (ADR-33 section 2, H1 to H12). Run on the grid M 0.3 to 1.6 and
//! h 0 to 12 km at 9 000 and 12 500 kg, restricted to the operating envelope: dynamic pressure of
//! at least 5 kPa and calibrated speed below the onset of the overspeed wall (92 % of `vmo`) (above it the overspeed wall, which
//! is a feature, is acting). Step sizes follow the response that is reachable at the condition.

mod common;

use common::*;
use fcs::sense::Sensed;
use fcs::{Assist, FcsModel};
use flight::aircraft::AircraftModel;
use simmath::dmath;

const MACHS: [f64; 6] = [0.3, 0.5, 0.7, 0.9, 1.2, 1.6];
const ALTS: [f64; 5] = [0.0, 3000.0, 6000.0, 9000.0, 12000.0];
const MASSES: [f64; 2] = [9000.0, 12500.0];

/// Every grid condition inside the operating envelope, trimmed, with its initial sense.
fn grid<'a>(
    model: &'a AircraftModel,
    fcs: &'a FcsModel,
    assist: Assist,
) -> Vec<(String, Rig<'a>, Sensed)> {
    let mut out = Vec::new();
    for mach in MACHS {
        for alt in ALTS {
            for mass in MASSES {
                let Some(rig) = Rig::new(model, fcs, assist, mach, alt, mass) else {
                    continue;
                };
                let s = rig.sensed();
                if s.qbar_pa < 5000.0
                    || s.cas > fcs.overspeed_onset * model.limits.vmo_mps
                    || s.mach > fcs.overspeed_onset * model.limits.mmo
                {
                    continue;
                }
                out.push((format!("M{mach} h{alt} m{mass}"), rig, s));
            }
        }
    }
    out
}

#[test]
fn the_grid_covers_the_envelope() {
    let (model, fcs) = kestrel();
    let g = grid(&model, &fcs, Assist::Standard);
    assert!(g.len() >= 35, "only {} conditions", g.len());
    // Subsonic and supersonic, low and high.
    assert!(g.iter().any(|(n, _, _)| n.starts_with("M0.3")));
    assert!(g.iter().any(|(n, _, _)| n.starts_with("M1.6")));
    assert!(g.iter().any(|(n, _, _)| n.contains("h12000")));
}

#[test]
fn h1_the_load_factor_step_settles_without_overshoot() {
    let (model, fcs) = kestrel();
    let mut worst = (0.0f64, 0.0f64);
    let mut n = 0;
    for (name, mut rig, s0) in grid(&model, &fcs, Assist::Standard) {
        let mass = s0.tel.mass_kg;
        let n_avail = model
            .limits
            .n_max
            .min(n_at_alpha_limit(&model, s0.mach, s0.qbar_pa, mass));
        if n_avail < 1.6 {
            continue; // nothing to step to: excluded by the step size, not by the threshold
        }
        let target = 1.0 + 0.5 * (n_avail - 1.0);
        let stick_pos = (target - 1.0) / (model.limits.n_max - 1.0);
        let data = rig.run(4.0, |t, _| {
            if t >= 1.0 {
                stick(stick_pos, 0.0, 0.0)
            } else {
                stick(0.0, 0.0, 0.0)
            }
        });
        let series: Vec<f64> = data.iter().map(|d| d.n_w).collect();
        let (over, settle) = step_metrics(&series, 1.0, 1.0, target, 0.1);
        assert_eq!(rig.faults, 0, "{name}");
        assert!(over < 10.0, "{name}: overshoot {over:.1} % (n {target:.2})");
        assert!(settle < 1.5, "{name}: 90 % settling {settle:.2} s");
        worst = (worst.0.max(over), worst.1.max(settle));
        n += 1;
    }
    println!(
        "H1: {n} conditions, worst overshoot {:.1} %, worst settling {:.2} s",
        worst.0, worst.1
    );
    assert!(n >= 30);
}

#[test]
fn h2_the_roll_rate_step_settles_without_overshoot() {
    let (model, fcs) = kestrel();
    let mut worst = (0.0f64, 0.0f64);
    let mut n = 0;
    for (name, mut rig, s0) in grid(&model, &fcs, Assist::Standard) {
        // Half of the roll rate available: the schedule, held back where the rudder could not
        // answer the coupling of a roll with the pull (level here, so at the pitch-rate floor).
        let sense = model.sense(&rig.ac, &rig.world, 0.0);
        let limit = fcs::standard::roll_rate_limit(&fcs, &s0, true)
            .min(fcs::standard::coupling_rate_limit(&model, &sense, &s0, 0.0));
        let target = 0.5 * limit;
        let data = rig.run(4.0, |t, _| {
            if t >= 1.0 {
                stick(0.0, 0.5, 0.0)
            } else {
                stick(0.0, 0.0, 0.0)
            }
        });
        let series: Vec<f64> = data
            .iter()
            .map(|d| d.rates.x * dmath::cos(d.alpha) + d.rates.z * dmath::sin(d.alpha))
            .collect();
        let (over, settle) = step_metrics(&series, 1.0, 0.0, target, 0.1);
        assert_eq!(rig.faults, 0, "{name}");
        assert!(over < 10.0, "{name}: overshoot {over:.1} %");
        assert!(settle < 1.5, "{name}: settling {settle:.2} s");
        worst = (worst.0.max(over), worst.1.max(settle));
        n += 1;
    }
    println!(
        "H2: {n} conditions, worst overshoot {:.1} %, worst settling {:.2} s",
        worst.0, worst.1
    );
    assert!(n >= 30);
}

#[test]
fn h3_hands_off_after_the_steps_there_is_no_limit_cycle() {
    let (model, fcs) = kestrel();
    for (name, mut rig, s0) in grid(&model, &fcs, Assist::Standard) {
        let n_avail = model.limits.n_max.min(n_at_alpha_limit(
            &model,
            s0.mach,
            s0.qbar_pa,
            s0.tel.mass_kg,
        ));
        let pull = if n_avail > 1.6 {
            0.5 * (n_avail - 1.0) / (model.limits.n_max - 1.0)
        } else {
            0.0
        };
        // The steps of H1 and H2, one second of each, then the stick is released and the
        // aircraft left alone; the window starts three seconds after the release.
        rig.run(1.0, |_, _| stick(pull, 0.0, 0.0));
        rig.run(0.5, |_, _| stick(0.0, 0.5, 0.0));
        rig.run(3.0, |_, _| stick(0.0, 0.0, 0.0));
        let window = rig.run(10.0, |_, _| stick(0.0, 0.0, 0.0));
        let n_min = window.iter().map(|d| d.n_w).fold(f64::MAX, f64::min);
        let n_max = window.iter().map(|d| d.n_w).fold(f64::MIN, f64::max);
        // The commanded quantity is the stability-axis roll rate; a coordinated turn at an angle
        // of attack has a body roll rate of -r sin(alpha) with that at zero.
        let p_max = window
            .iter()
            .map(|d| (d.rates.x * dmath::cos(d.alpha) + d.rates.z * dmath::sin(d.alpha)).abs())
            .fold(0.0, f64::max);
        assert_eq!(rig.faults, 0, "{name}");
        assert!(
            n_max - n_min < 0.02,
            "{name}: n varies {:.3} g",
            n_max - n_min
        );
        assert!(
            p_max.to_degrees() < 0.5,
            "{name}: roll rate {:.2} deg/s",
            p_max.to_degrees()
        );
    }
}

#[test]
fn h4_a_full_rate_roll_keeps_the_sideslip_small() {
    let (model, fcs) = kestrel();
    let mut worst = 0.0f64;
    for (name, mut rig, _) in grid(&model, &fcs, Assist::Standard) {
        let mut angle = 0.0;
        let mut beta_max = 0.0f64;
        let mut t = 0.0;
        while angle < core::f64::consts::TAU && t < 14.0 {
            let d = rig.step(&stick(0.0, 1.0, 0.0));
            // The wind-axis roll rate integrates to the angle rolled through.
            angle += (d.rates.x * dmath::cos(d.alpha) + d.rates.z * dmath::sin(d.alpha)) * DT;
            beta_max = beta_max.max(d.beta.abs());
            t += DT;
        }
        assert_eq!(rig.faults, 0, "{name}");
        assert!(
            angle >= core::f64::consts::TAU,
            "{name}: only {:.0} deg in {t:.0} s",
            angle.to_degrees()
        );
        assert!(
            beta_max.to_degrees() < 5.0,
            "{name}: sideslip {:.2} deg",
            beta_max.to_degrees()
        );
        worst = worst.max(beta_max);
    }
    println!("H4: worst sideslip {:.2} deg", worst.to_degrees());
}

#[test]
fn h5_full_aft_stick_is_held_at_the_angle_of_attack_and_load_limits() {
    let (model, fcs) = kestrel();
    let mut worst = (0.0f64, 0.0f64);
    for (name, mut rig, _) in grid(&model, &fcs, Assist::Standard) {
        // Held for as long as the aircraft stays in the grid's envelope: over the top of a loop
        // the dynamic pressure falls and with it all control power (below 5 kPa, the floor of
        // the grid, the run is over).
        let data: Vec<_> = rig
            .run(8.0, |_, _| stick(1.0, 0.0, 0.0))
            .into_iter()
            .take_while(|d| d.qbar_pa >= 5000.0)
            .collect();
        assert!(
            data.len() > 30,
            "{name}: left the envelope after {} ticks",
            data.len()
        );
        let alpha = data
            .iter()
            .map(|d| d.alpha)
            .fold(f64::MIN, f64::max)
            .to_degrees();
        let n_max_g = data.iter().map(|d| d.n_w).fold(f64::MIN, f64::max);
        let limit = model.limits.alpha_limit_rad.to_degrees();
        assert_eq!(rig.faults, 0, "{name}");
        assert!(
            alpha <= limit + 2.0,
            "{name}: alpha reached {alpha:.1} deg (limit {limit})"
        );
        assert!(
            n_max_g <= model.limits.n_max + 0.3,
            "{name}: n reached {n_max_g:.2}"
        );
        worst = (worst.0.max(alpha), worst.1.max(n_max_g));
    }
    println!(
        "H5: worst alpha {:.1} deg, worst n {:.2} g",
        worst.0, worst.1
    );
}

#[test]
fn h5b_the_pull_over_the_vertical_keeps_the_angle_of_attack_limit() {
    // Past 90 degrees of pitch the Euler bank flips by half a turn; the loops must not notice.
    let (model, fcs) = kestrel();
    let mut rig = Rig::new(&model, &fcs, Assist::Standard, 0.5, 0.0, 12500.0).unwrap();
    let data: Vec<_> = rig
        .run(6.0, |_, _| stick(1.0, 0.0, 0.0))
        .into_iter()
        .take_while(|d| d.qbar_pa >= 5000.0)
        .collect();
    let flipped = data
        .iter()
        .filter(|d| d.theta.to_degrees() > 85.0 || d.phi.abs() > 90f64.to_radians());
    assert!(flipped.count() > 10, "the pull reached the vertical");
    assert!(
        data.iter().any(|d| d.phi.abs() > 90f64.to_radians()),
        "the Euler bank flipped"
    );
    assert!(
        data.iter().all(|d| d.bank_w.abs() < 10f64.to_radians()),
        "the bank of the lift vector stayed continuous"
    );
    let limit = model.limits.alpha_limit_rad.to_degrees();
    let alpha = data.iter().map(|d| d.alpha).fold(f64::MIN, f64::max);
    assert!(alpha.to_degrees() <= limit + 0.5, "{}", alpha.to_degrees());
    assert_eq!(rig.faults, 0);
}

#[test]
fn h6_an_afterburner_dive_stops_at_the_overspeed_wall() {
    let (model, fcs) = kestrel();
    let mut rig = Rig::new(&model, &fcs, Assist::Standard, 0.9, 3000.0, 10000.0).unwrap();
    rig.engine = flight::engine::EngineCommand::max_ab(model.ab_stages());
    rig.pitch_over(-30f64.to_radians());
    let data = rig.run(40.0, |_, _| stick(0.0, 0.0, 0.0));
    let cas = data.iter().map(|d| d.cas).fold(0.0, f64::max);
    let mach = data.iter().map(|d| d.mach).fold(0.0, f64::max);
    println!(
        "H6: peak CAS {cas:.1} m/s (vmo {}), peak Mach {mach:.3} (mmo {})",
        model.limits.vmo_mps, model.limits.mmo
    );
    assert_eq!(rig.faults, 0);
    assert!(cas <= 1.03 * model.limits.vmo_mps, "CAS {cas}");
    assert!(mach <= 1.03 * model.limits.mmo, "Mach {mach}");
    assert!(
        cas > 0.9 * model.limits.vmo_mps,
        "the dive really did reach the wall: {cas}"
    );
}

#[test]
fn h7_h8_h9_the_full_law_holds_the_path_banks_and_levels() {
    let (model, fcs) = kestrel();
    for (mach, alt) in [(0.4, 3000.0), (0.6, 5000.0), (0.9, 8000.0), (1.2, 10000.0)] {
        let name = format!("M{mach} h{alt}");
        // H7: stick centred for ten seconds from a 3 degree climb.
        let mut rig = Rig::new(&model, &fcs, Assist::Full, mach, alt, 10000.0).unwrap();
        rig.pitch_over(3f64.to_radians());
        rig.run(1.0, |_, _| stick(0.0, 0.0, 0.0));
        let g0 = rig.sensed().gamma;
        let data = rig.run(10.0, |_, _| stick(0.0, 0.0, 0.0));
        let drift = data
            .iter()
            .map(|d| (d.gamma - g0).abs())
            .fold(0.0, f64::max)
            .to_degrees();
        assert!(drift < 0.5, "H7 {name}: flight path drifted {drift:.2} deg");

        // H8: a 45 degree bank command (stick 0.75 of 60 deg) settles within 2 degrees.
        let mut rig = Rig::new(&model, &fcs, Assist::Full, mach, alt, 10000.0).unwrap();
        let data = rig.run(8.0, |_, _| stick(0.0, 0.75, 0.0));
        let bank = data.last().unwrap().phi.to_degrees();
        let tail = data[data.len() - 120..]
            .iter()
            .map(|d| (d.phi.to_degrees() - 45.0).abs())
            .fold(0.0, f64::max);
        assert!(
            (bank - 45.0).abs() < 2.0 && tail < 2.0,
            "H8 {name}: bank {bank:.2} deg"
        );

        // H9: released from 45 degrees, the wings are level within 2 degrees in 2 s.
        let data = rig.run(2.0, |_, _| stick(0.0, 0.0, 0.0));
        let bank = data.last().unwrap().phi.to_degrees();
        assert!(bank.abs() < 2.0, "H9 {name}: bank {bank:.2} deg after 2 s");
        assert_eq!(rig.faults, 0, "{name}");
    }
}

#[test]
fn h10_the_reduced_pitch_rate_step_settles_without_overshoot() {
    let (model, fcs) = kestrel();
    let mut worst = (0.0f64, 0.0f64);
    let mut n = 0;
    for (name, mut rig, s0) in grid(&model, &fcs, Assist::Reduced) {
        let target = 0.5 * fcs.pitch.q_max.eval(s0.qbar_kpa());
        let data = rig.run(4.0, |t, _| {
            if t >= 1.0 {
                stick(0.5, 0.0, 0.0)
            } else {
                stick(0.0, 0.0, 0.0)
            }
        });
        let series: Vec<f64> = data.iter().map(|d| d.rates.y).collect();
        let (over, settle) = step_metrics(&series, 1.0, 0.0, target, 0.1);
        assert_eq!(rig.faults, 0, "{name}");
        assert!(over < 10.0, "{name}: overshoot {over:.1} %");
        assert!(settle < 1.5, "{name}: settling {settle:.2} s");
        worst = (worst.0.max(over), worst.1.max(settle));
        n += 1;
    }
    println!(
        "H10: {n} conditions, worst overshoot {:.1} %, worst settling {:.2} s",
        worst.0, worst.1
    );
    assert!(n >= 30);
}
