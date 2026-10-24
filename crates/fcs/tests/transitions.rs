//! Ground <-> flight blends and assist switching (ADR-33: H11, H12).

mod common;

use common::*;
use fcs::{Assist, PilotInput};
use flight::actuators::Surfaces;
use flight::controls::PilotControls;
use flight::engine::EngineCommand;
use flight::env::SphereSea;

/// No surface command may move more than this in a tick, rad (2 degrees, H11).
const MAX_STEP_RAD: f64 = 2.0 * core::f64::consts::PI / 180.0;

fn controls(f: impl FnOnce(&mut PilotControls)) -> PilotInput {
    let mut c = PilotControls {
        gear: 1.0,
        ..PilotControls::default()
    };
    f(&mut c);
    PilotInput {
        controls: c,
        ..PilotInput::default()
    }
}

/// The largest one-tick change of the three surface commands after `rig.step`, rad.
fn step_of(prev: &mut [f64; 3], now: [f64; 3]) -> f64 {
    let d = (0..3).map(|i| (now[i] - prev[i]).abs()).fold(0.0, f64::max);
    *prev = now;
    d
}

#[test]
fn h11a_the_take_off_blends_from_the_ground_law_without_a_surface_step() {
    let (model, fcs) = kestrel();
    for assist in [Assist::Full, Assist::Standard, Assist::Reduced] {
        let mut rig = Rig::on_runway(&model, &fcs, assist, 10_500.0);
        let mut prev = rig.fs.last_cmd;
        let (mut worst, mut stick, mut airborne_s) = (0.0f64, 0.0f64, 0.0);
        for i in 0..(35 * 60) {
            let s = rig.sensed();
            let t = f64::from(i) * DT;
            if s.tel.ground_speed_mps > 70.0 && s.theta.to_degrees() < 9.0 {
                stick = (stick + 0.5 * DT).min(0.5);
            } else if s.theta.to_degrees() >= 9.0 {
                stick = (stick - 0.1 * DT).max(0.0);
            }
            let input = controls(|c| {
                c.throttle = if t > 1.0 { 1.0 } else { 0.0 };
                c.flaps = 0.5;
                c.pitch = stick;
            });
            rig.step(&input);
            if i > 1 {
                let st = step_of(&mut prev, rig.fs.last_cmd);
                worst = worst.max(st);
            }
            if !rig.ac.ground.wow {
                airborne_s += DT;
            }
        }
        println!(
            "H11a {assist:?}: worst step {:.2} deg, airborne for {airborne_s:.1} s",
            worst.to_degrees()
        );
        assert_eq!(rig.faults, 0, "{assist:?}");
        assert!(rig.destroyed.is_none(), "{assist:?}: {:?}", rig.destroyed);
        assert!(airborne_s > 10.0, "{assist:?}: it took off ({airborne_s})");
        assert!(
            worst <= MAX_STEP_RAD,
            "{assist:?}: step {}",
            worst.to_degrees()
        );
    }
}

#[test]
fn h11b_the_touchdown_blends_into_the_ground_law_without_a_surface_step() {
    let (model, fcs) = kestrel();
    let landing = Surfaces {
        tef_rad: model.actuators.tef.max,
        gear: 1.0,
        ..Surfaces::NEUTRAL
    };
    for assist in [Assist::Full, Assist::Standard, Assist::Reduced] {
        let mut rig = Rig::trimmed(
            SphereSea::default(),
            &model,
            &fcs,
            assist,
            75.0,
            30.0,
            9000.0,
            landing,
        )
        .expect("the approach trims");
        rig.pitch_over(-2f64.to_radians());
        let mut prev = rig.fs.last_cmd;
        let mut worst = 0.0f64;
        let mut down_s = 0.0;
        for i in 0..(25 * 60) {
            let s = rig.sensed();
            if s.tel.height_msl_m < 6.0 {
                rig.engine = EngineCommand::IDLE;
            }
            let on = rig.ac.ground.wow;
            let input = controls(|c| {
                c.flaps = 1.0;
                if on {
                    c.brake_l = 0.5;
                    c.brake_r = 0.5;
                }
            });
            rig.step(&input);
            if i > 1 {
                let st = step_of(&mut prev, rig.fs.last_cmd);
                worst = worst.max(st);
            }
            if rig.ac.ground.wow {
                down_s += DT;
            }
        }
        println!(
            "H11b {assist:?}: worst step {:.2} deg, on the ground {down_s:.1} s, sink {:.2}",
            worst.to_degrees(),
            rig.ac.ground.touchdown_vs_mps
        );
        assert_eq!(rig.faults, 0, "{assist:?}");
        assert!(rig.destroyed.is_none(), "{assist:?}: {:?}", rig.destroyed);
        assert!(rig.touchdowns >= 1, "{assist:?}: it landed");
        assert!(down_s > 5.0, "{assist:?}: it stayed down ({down_s})");
        assert!(
            worst <= MAX_STEP_RAD,
            "{assist:?}: step {}",
            worst.to_degrees()
        );
    }
}

/// Level flight conditions for the assist-switch test: (Mach, altitude m, mass kg).
const SWITCH_CONDITIONS: [(f64, f64, f64); 4] = [
    (0.45, 1_000.0, 10_000.0),
    (0.6, 5_000.0, 10_000.0),
    (0.9, 3_000.0, 9_000.0),
    (1.2, 9_000.0, 12_500.0),
];
const ASSISTS: [Assist; 4] = [
    Assist::Full,
    Assist::Standard,
    Assist::Reduced,
    Assist::Manual,
];

#[test]
fn h12_an_assist_switch_in_flight_is_bumpless() {
    let (model, fcs) = kestrel();
    let (mut worst_step, mut worst_n, mut cases) = (0.0f64, 0.0f64, 0);
    for (mach, alt, mass) in SWITCH_CONDITIONS {
        for from in ASSISTS {
            for to in ASSISTS {
                if from == to {
                    continue;
                }
                let mut rig = Rig::new(&model, &fcs, from, mach, alt, mass)
                    .unwrap_or_else(|| panic!("M{mach} {alt} m does not trim"));
                let idle = stick(0.0, 0.0, 0.0);
                for _ in 0..(3.0 / DT) as usize {
                    rig.step(&idle);
                }
                let n_before = rig.sensed().n_w;
                let mut prev = rig.fs.last_cmd;
                fcs.switch_assist(&mut rig.fs, to);
                let mut step = 0.0f64;
                let mut n_dev = 0.0f64;
                for _ in 0..(1.5 / DT) as usize {
                    let s = rig.step(&idle);
                    step = step.max(step_of(&mut prev, rig.fs.last_cmd));
                    n_dev = n_dev.max((s.n_w - n_before).abs());
                }
                let name = format!("M{mach} {alt} m: {from:?} -> {to:?}");
                assert_eq!(rig.faults, 0, "{name}");
                assert!(
                    step < 1f64.to_radians(),
                    "{name}: surface step {}",
                    step.to_degrees()
                );
                assert!(n_dev < 0.3, "{name}: load factor moved {n_dev}");
                worst_step = worst_step.max(step);
                worst_n = worst_n.max(n_dev);
                cases += 1;
            }
        }
    }
    println!(
        "H12: {cases} switches, worst surface step {:.2} deg, worst n transient {worst_n:.3} g",
        worst_step.to_degrees()
    );
}

#[test]
fn h12b_a_switch_with_the_stick_held_moves_no_surface_by_a_step() {
    // The meaning of the stick changes with the law, so the aircraft will move; the surfaces
    // must not jump.
    let (model, fcs) = kestrel();
    let (mut worst, mut cases) = (0.0f64, 0);
    for (mach, alt, mass) in SWITCH_CONDITIONS {
        for from in ASSISTS {
            for to in ASSISTS {
                if from == to {
                    continue;
                }
                let mut rig = Rig::new(&model, &fcs, from, mach, alt, mass).unwrap();
                let held = stick(0.15, 0.0, 0.0);
                for _ in 0..(3.0 / DT) as usize {
                    rig.step(&held);
                }
                let mut prev = rig.fs.last_cmd;
                fcs.switch_assist(&mut rig.fs, to);
                let mut step = 0.0f64;
                for _ in 0..(1.0 / DT) as usize {
                    rig.step(&held);
                    step = step.max(step_of(&mut prev, rig.fs.last_cmd));
                }
                let name = format!("M{mach} {alt} m: {from:?} -> {to:?}");
                assert_eq!(rig.faults, 0, "{name}");
                assert!(
                    step < 1f64.to_radians(),
                    "{name}: surface step {}",
                    step.to_degrees()
                );
                worst = worst.max(step);
                cases += 1;
            }
        }
    }
    println!(
        "H12b: {cases} switches, worst surface step {:.2} deg",
        worst.to_degrees()
    );
}
