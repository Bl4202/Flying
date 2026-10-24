//! Random pilots (AC-11.6): whatever the controls do, under any assist, nothing goes non-finite
//! and nothing faults. Ten minutes per seed in release (eight seeds), the first two seeds in
//! debug; a runway fuzz exercises the ground law and the gear.

mod common;

use common::*;
use fcs::{Aim, Assist, PilotInput};
use flight::actuators::Surfaces;
use flight::controls::PilotControls;
use flight::env::SphereSea;

/// A small deterministic generator (the tests need no quality, only repeatability).
struct XorShift(u64);

impl XorShift {
    fn next(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }

    fn signed(&mut self) -> f64 {
        2.0 * self.next() - 1.0
    }

    /// Mostly a value in range, often the end stops.
    fn stick(&mut self) -> f64 {
        match self.next() {
            x if x < 0.15 => 1.0,
            x if x < 0.30 => -1.0,
            x if x < 0.40 => 0.0,
            _ => self.signed(),
        }
    }
}

fn seeds() -> &'static [u64] {
    if cfg!(debug_assertions) {
        &[1, 2]
    } else {
        &[1, 2, 3, 4, 5, 6, 7, 8]
    }
}

fn random_input(rng: &mut XorShift, assist: Assist) -> PilotInput {
    let mut c = PilotControls {
        pitch: rng.stick(),
        roll: rng.stick(),
        yaw: rng.stick(),
        throttle: rng.next(),
        trim_nose_up: rng.next() < 0.1,
        trim_nose_down: rng.next() < 0.1,
        flaps: if rng.next() < 0.1 { 1.0 } else { 0.0 },
        airbrake: if rng.next() < 0.2 { 1.0 } else { 0.0 },
        gear: if rng.next() < 0.1 { 1.0 } else { 0.0 },
        brake_l: rng.next(),
        brake_r: rng.next(),
    };
    // Now and then the input is garbage: the control system must shrug it off.
    if rng.next() < 0.02 {
        c.pitch = f64::NAN;
    }
    if rng.next() < 0.02 {
        c.roll = 1.0e30;
    }
    let aim =
        (matches!(assist, Assist::Full | Assist::Standard) && rng.next() < 0.2).then(|| Aim {
            azimuth_rad: core::f64::consts::PI * rng.signed(),
            elevation_rad: core::f64::consts::FRAC_PI_2 * rng.signed(),
        });
    PilotInput {
        controls: c,
        aim,
        recover: rng.next() < 0.05,
        gloc: rng.next() < 0.02,
        auto_gcas: false,
    }
}

/// Random controls for `seconds` under `assist`, respawning whenever the aircraft gets near the
/// sea. Returns the number of respawns.
fn fuzz(assist: Assist, seed: u64, seconds: u32) -> u32 {
    let (model, fcs) = kestrel();
    let (speed, alt) = if seed % 2 == 1 {
        (220.0, 6_000.0)
    } else {
        (443.0, 11_000.0)
    };
    let mut rig = Rig::trimmed(
        SphereSea::default(),
        &model,
        &fcs,
        assist,
        speed,
        alt,
        10_000.0,
        Surfaces::NEUTRAL,
    )
    .expect("the start trims");
    let (spawn_ac, spawn_fs) = (rig.ac, rig.fs);
    let mut rng = XorShift(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
    let mut input = PilotInput::default();
    let mut respawns = 0;
    for tick in 0..seconds * 60 {
        if tick % 30 == 0 {
            input = random_input(&mut rng, assist);
        }
        // The throttle lever is the pilot's; the engine follows it through the control system.
        rig.use_lever = true;
        rig.step(&input);
        let name = format!("{assist:?} seed {seed} tick {tick}");
        assert!(!rig.ac.numerical_fault, "{name}: numerical fault");
        assert!(rig.ac.body.is_finite(), "{name}: non-finite state");
        assert!(rig.fs.is_finite(), "{name}: non-finite control state");
        assert!(
            rig.fs.last_cmd.iter().all(|v| v.is_finite()),
            "{name}: non-finite command"
        );
        if rig.height() < 300.0 {
            rig.ac = spawn_ac;
            rig.fs = spawn_fs;
            respawns += 1;
        }
    }
    assert_eq!(rig.faults, 0, "{assist:?} seed {seed}: faults");
    respawns
}

#[test]
fn ac_11_6_random_pilots_cannot_make_any_assist_fault() {
    for assist in [
        Assist::Full,
        Assist::Standard,
        Assist::Reduced,
        Assist::Manual,
    ] {
        for &seed in seeds() {
            fuzz(assist, seed, 600);
        }
    }
}

#[test]
fn ac_11_6_random_pilots_cannot_make_the_ground_roll_fault() {
    // Two minutes from the runway with random controls: take-offs, runway excursions of the
    // virtual kind, bounces, collapsed legs and crashes (which restart the run on the runway).
    let (model, fcs) = kestrel();
    let (mut rolling, mut flying, mut crashes) = (0u32, 0u32, 0u32);
    for assist in [
        Assist::Full,
        Assist::Standard,
        Assist::Reduced,
        Assist::Manual,
    ] {
        for &seed in &seeds()[..2] {
            let mut rig = Rig::on_runway(&model, &fcs, assist, 10_000.0);
            let (spawn_ac, spawn_fs) = (rig.ac, rig.fs);
            let mut rng = XorShift(seed.wrapping_mul(0xD1B5_4A32_D192_ED03) | 1);
            let mut input = PilotInput::default();
            for tick in 0..(120 * 60) {
                if tick % 20 == 0 {
                    input = random_input(&mut rng, assist);
                    // Mostly roll straight: full power, wheels down.
                    input.controls.throttle = 0.5 + 0.5 * rng.next();
                    input.controls.gear = 1.0;
                    // A bias to pull, a light hand on the roll and the pedals, and mostly no
                    // brakes: enough to take off, bounce, skid and crash now and then.
                    input.controls.pitch = 0.3 + 0.3 * input.controls.pitch.clamp(-1.0, 1.0);
                    input.controls.roll *= 0.2;
                    input.controls.yaw *= 0.3;
                    if rng.next() > 0.15 {
                        input.controls.brake_l = 0.0;
                        input.controls.brake_r = 0.0;
                    }
                    input.gloc = false;
                }
                rig.step(&input);
                let name = format!("{assist:?} seed {seed} tick {tick}");
                assert!(!rig.ac.numerical_fault, "{name}: numerical fault");
                assert!(rig.ac.body.is_finite(), "{name}: non-finite state");
                assert!(rig.fs.is_finite(), "{name}: non-finite control state");
                if rig.ac.ground.wow {
                    rolling += 1;
                } else {
                    flying += 1;
                }
                if rig.destroyed.is_some() || rig.height() < -10.0 {
                    crashes += 1;
                    rig.ac = spawn_ac;
                    rig.fs = spawn_fs;
                    rig.destroyed = None;
                }
            }
            assert_eq!(rig.faults, 0, "{assist:?} seed {seed}: faults");
        }
    }
    println!("ground fuzz: {rolling} ticks on the wheels, {flying} in the air, {crashes} crashes");
    // The fuzz really did use the ground and the air.
    assert!(rolling > 5_000 && flying > 1_000, "{rolling} / {flying}");
}
