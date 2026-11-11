//! NN-2 (ADR-45 G1-c): the flight-control system commands the airframe; it never moves it.
//!
//! - G1-c1, the structural audit: from 2 000 random aircraft states under each assist, with random
//!   pilot inputs, `FcsModel::step` changes nothing in the aircraft's state but the pitch trim (the
//!   trim switches). The body, the engines, the fuel, the ground and the surfaces move only in the
//!   airframe's own tick.
//! - G1-c2, the behavioural test: under Full, full right stick gives a turn whose rate depends on
//!   the speed, and changes as the speed changes during the turn. A control system that turned the
//!   aircraft at a fixed rate would give neither. (ADR-45 expected the turn at M 0.85 to bleed
//!   speed; at military power the Kestrel gains it, about 292 to 323 m/s, and the rate falls.)

mod common;

use common::*;
use fcs::{Aim, Assist, FcsState, PilotInput};
use flight::actuators::Surfaces;
use flight::aircraft::{AircraftModel, AircraftState};
use flight::controls::PilotControls;
use flight::env::SphereSea;
use simmath::math::{DQuat, DVec3};

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

    fn between(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.next()
    }
}

const ASSISTS: [Assist; 4] = [
    Assist::Full,
    Assist::Standard,
    Assist::Reduced,
    Assist::Manual,
];

/// Random states per assist.
const STATES: u32 = 2_000;
/// Calls of the control system on each state, each with a new random input.
const CALLS: u32 = 3;

/// Any attitude.
fn random_att(rng: &mut XorShift) -> DQuat {
    use core::f64::consts::PI;
    (DQuat::from_axis_angle(DVec3::Z, PI * rng.signed())
        * DQuat::from_axis_angle(DVec3::Y, 0.5 * PI * rng.signed())
        * DQuat::from_axis_angle(DVec3::X, PI * rng.signed()))
    .normalized()
}

/// Surfaces anywhere in their travel.
fn random_surfaces(model: &AircraftModel, rng: &mut XorShift) -> Surfaces {
    let a = &model.actuators;
    Surfaces {
        elevator_rad: rng.between(a.elevator.min, a.elevator.max),
        aileron_rad: rng.between(a.aileron.min, a.aileron.max),
        rudder_rad: rng.between(a.rudder.min, a.rudder.max),
        lef_rad: rng.between(a.lef.min, a.lef.max),
        tef_rad: rng.between(a.tef.min, a.tef.max),
        airbrake: rng.next(),
        gear: if rng.next() < 0.3 { 1.0 } else { 0.0 },
    }
}

/// An aircraft somewhere in the air: any attitude, a velocity within 50° of the nose (sometimes
/// anywhere), any rates, surfaces, trim and fuel.
fn random_air_state(
    base: &AircraftState,
    model: &AircraftModel,
    rng: &mut XorShift,
) -> AircraftState {
    let mut ac = *base;
    let att = random_att(rng);
    let speed = rng.between(40.0, 460.0);
    let dir_body = if rng.next() < 0.9 {
        DVec3::new(1.0, 1.2 * rng.signed(), 1.2 * rng.signed())
    } else {
        DVec3::new(rng.signed(), rng.signed(), rng.signed())
    };
    ac.body.att = att;
    ac.body.vel_bf = att.rotate(dir_body.normalized_or(DVec3::X)) * speed;
    ac.body.pos_bf = DVec3::new(
        20_000.0 * rng.signed(),
        20_000.0 * rng.signed(),
        rng.between(300.0, 14_000.0),
    );
    ac.body.omega_body = DVec3::new(3.0 * rng.signed(), 1.5 * rng.signed(), rng.signed());
    ac.surfaces = random_surfaces(model, rng);
    ac.fuel = model.mass.load_for(rng.between(0.0, 3_500.0));
    let e = &model.actuators.elevator;
    ac.trim_pitch_rad = 0.5 * rng.between(e.min, e.max);
    ac
}

/// An aircraft on the runway, standing or rolling along it, with any surfaces and trim.
fn random_ground_state(
    base: &AircraftState,
    model: &AircraftModel,
    rng: &mut XorShift,
) -> AircraftState {
    let mut ac = *base;
    let nose = ac.body.att.rotate(DVec3::X);
    ac.body.vel_bf = nose * rng.between(0.0, 90.0);
    ac.surfaces = random_surfaces(model, rng);
    ac.surfaces.gear = 1.0;
    let e = &model.actuators.elevator;
    ac.trim_pitch_rad = 0.5 * rng.between(e.min, e.max);
    ac
}

/// Any pilot input, levers and buttons included.
fn random_input(rng: &mut XorShift, assist: Assist) -> PilotInput {
    let stick = |rng: &mut XorShift| match rng.next() {
        x if x < 0.15 => 1.0,
        x if x < 0.30 => -1.0,
        _ => rng.signed(),
    };
    let controls = PilotControls {
        pitch: stick(rng),
        roll: stick(rng),
        yaw: stick(rng),
        throttle: rng.next(),
        trim_nose_up: rng.next() < 0.25,
        trim_nose_down: rng.next() < 0.25,
        flaps: if rng.next() < 0.2 { 1.0 } else { 0.0 },
        airbrake: if rng.next() < 0.2 { 1.0 } else { 0.0 },
        gear: if rng.next() < 0.2 { 1.0 } else { 0.0 },
        brake_l: rng.next(),
        brake_r: rng.next(),
    };
    let aim =
        (matches!(assist, Assist::Full | Assist::Standard) && rng.next() < 0.3).then(|| Aim {
            azimuth_rad: core::f64::consts::PI * rng.signed(),
            elevation_rad: core::f64::consts::FRAC_PI_2 * rng.signed(),
        });
    PilotInput {
        controls,
        aim,
        recover: rng.next() < 0.1,
        gloc: rng.next() < 0.05,
        auto_gcas: false,
    }
}

#[test]
fn g1_c1_the_control_system_never_writes_the_aircraft_state_but_its_trim() {
    let (model, fcs) = kestrel();
    let air = Rig::new(&model, &fcs, Assist::Standard, 0.6, 5_000.0, 10_000.0)
        .expect("the start trims")
        .ac;
    // On the runway, settled on its wheels by a second of the airframe's own ticks.
    let mut runway = Rig::on_runway(&model, &fcs, Assist::Standard, 10_000.0);
    for _ in 0..60 {
        runway.step(&PilotInput::default());
    }
    assert!(runway.ac.ground.wow);
    let ground = runway.ac;
    let mut rng = XorShift(0x9E37_79B9_7F4A_7C15);
    let (mut calls, mut trims, mut on_ground) = (0u32, 0u32, 0u32);
    for assist in ASSISTS {
        for i in 0..STATES {
            // One state in ten on the runway (the ground law), the rest anywhere in the air.
            let wheels = i % 10 == 0;
            let (mut ac, world) = if wheels {
                (
                    random_ground_state(&ground, &model, &mut rng),
                    SphereSea::default(),
                )
            } else {
                (random_air_state(&air, &model, &mut rng), SphereSea::open())
            };
            on_ground += u32::from(ac.ground.wow);
            let mut fs = FcsState::new(assist, wheels);
            let t = rng.between(0.0, 600.0);
            for k in 0..CALLS {
                let input = random_input(&mut rng, assist);
                let before = ac;
                let _ = fcs.step(
                    &model,
                    &mut ac,
                    &mut fs,
                    &input,
                    &world,
                    t + f64::from(k) * DT,
                );
                let mut rest = ac;
                rest.trim_pitch_rad = before.trim_pitch_rad;
                assert_eq!(
                    rest, before,
                    "{assist:?} state {i} call {k}: the control system wrote the aircraft's state"
                );
                calls += 1;
                trims += u32::from(ac.trim_pitch_rad.to_bits() != before.trim_pitch_rad.to_bits());
            }
        }
    }
    println!("NN-2 audit: {calls} calls, {trims} trim changes, {on_ground} states on the wheels");
    assert_eq!(calls, 4 * STATES * CALLS);
    // The trim switches were exercised, and the ground states were on the wheels.
    assert!(trims > 0);
    assert!(on_ground >= 4 * STATES / 10, "{on_ground}");
}

/// The track's heading, rad (east of north).
fn heading(v: DVec3) -> f64 {
    simmath::dmath::atan2(v.x, v.y)
}

/// Full assist, full right stick from level and trimmed at `mach` and 3 000 m, military power:
/// the mean heading rate over seconds 4 to 8 and over seconds 20 to 24, rad/s, and the true
/// airspeed at 6 s and at 22 s, m/s.
fn full_right_turn(mach: f64) -> (f64, f64, f64, f64) {
    let (model, fcs) = kestrel();
    let mut rig = Rig::new(&model, &fcs, Assist::Full, mach, 3_000.0, 10_000.0).expect("trims");
    rig.use_lever = true;
    let input = PilotInput {
        controls: PilotControls {
            roll: 1.0,
            // The lever's military detent (full dry thrust, no afterburner).
            throttle: 0.8,
            ..PilotControls::default()
        },
        ..PilotInput::default()
    };
    // The unwrapped heading at the end of each tick.
    let mut psi = Vec::with_capacity(24 * 60 + 1);
    psi.push(heading(rig.ac.body.vel_bf));
    let mut tas = [0.0; 2];
    for tick in 0..24 * 60 {
        rig.step(&input);
        if tick + 1 == 6 * 60 {
            tas[0] = rig.ac.body.vel_bf.length();
        }
        if tick + 1 == 22 * 60 {
            tas[1] = rig.ac.body.vel_bf.length();
        }
        let last = *psi.last().unwrap();
        let now = heading(rig.ac.body.vel_bf);
        let d = (now - last + core::f64::consts::PI).rem_euclid(core::f64::consts::TAU)
            - core::f64::consts::PI;
        psi.push(last + d);
    }
    assert_eq!(rig.faults, 0);
    let at = |s: usize| psi[s * 60];
    (
        (at(8) - at(4)) / 4.0,
        (at(24) - at(20)) / 4.0,
        tas[0],
        tas[1],
    )
}

#[test]
fn g1_c2_a_full_assist_turn_depends_on_the_speed_and_bleeds() {
    let (slow, _, slow_v, _) = full_right_turn(0.5);
    let (fast, fast_late, fast_v, fast_late_v) = full_right_turn(0.85);
    println!(
        "NN-2 turn: M 0.5 {:.2}°/s at {slow_v:.0} m/s; M 0.85 {:.2}°/s at {fast_v:.0} m/s, then \
         {:.2}°/s at {fast_late_v:.0} m/s (20 to 24 s)",
        slow.to_degrees(),
        fast.to_degrees(),
        fast_late.to_degrees()
    );
    // Turning right, at every speed.
    assert!(slow > 0.0 && fast > 0.0 && fast_late > 0.0);
    // The same stick, a different rate at a different speed (by at least 10 %).
    assert!((slow / fast - 1.0).abs() >= 0.10, "{slow} {fast}");
    // And at M 0.85 the rate changes by at least 5 % as the speed changes in the turn.
    assert!((fast_late / fast - 1.0).abs() >= 0.05, "{fast} {fast_late}");
}
