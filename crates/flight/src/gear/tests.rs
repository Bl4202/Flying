use super::*;
use crate::actuators::Surfaces;
use crate::aircraft::{AircraftModel, AircraftState, Commands};
use crate::engine::{EngineCommand, EngineModel};
use crate::env::{FlightEnv, SphereSea};
use crate::frames::{Euler, euler};
use crate::physical::G0;
use crate::test_content::registry;
use simmath::dmath;

const TICK: f64 = 1.0 / 60.0;

fn kestrel() -> AircraftModel {
    let r = registry();
    let h = r.aircraft_handles().next().unwrap();
    AircraftModel::from_registry(r, h, TICK).unwrap()
}

/// The Kestrel at rest on a paved, dry sea-level pad with 60 % fuel, engine idling.
fn at_rest(m: &AircraftModel) -> (AircraftState, StaticPose) {
    let (mut s, pose) = m
        .static_state(
            3100.0 * 0.6,
            0.0,
            0.0,
            DVec3::ZERO,
            &SphereSea::default(),
            EngineCommand::IDLE,
        )
        .expect("the Kestrel settles on its gear");
    s.ground.air_time_s = 0.0;
    (s, pose)
}

fn cmd(wheels: WheelCommands, throttle: EngineCommand, gear: f64) -> Commands {
    Commands {
        surfaces: Surfaces {
            gear,
            ..Surfaces::NEUTRAL
        },
        engine: throttle,
        wheels,
    }
}

fn all_brakes(b: f64) -> WheelCommands {
    WheelCommands {
        brake: [b; MAX_GEAR],
        steer_rad: 0.0,
    }
}

/// Runs `ticks` ticks, returning the events they raised.
fn run(
    m: &AircraftModel,
    s: &mut AircraftState,
    c: &Commands,
    world: &SphereSea,
    ticks: usize,
) -> GroundEvents {
    let mut ev = GroundEvents::NONE;
    for i in 0..ticks {
        let out = m.step_tick(s, c, world, i as f64 * TICK);
        assert_eq!(out.faults, 0, "tick {i}");
        ev.merge(&out.events);
    }
    ev
}

fn pitch_deg(s: &AircraftState, world: &SphereSea) -> f64 {
    euler(world.up_bf(s.body.pos_bf), s.body.att)
        .pitch_rad
        .to_degrees()
}

// ---------------------------------------------------------------------------------------------
// The model and the static pose.

#[test]
fn the_kestrel_has_three_legs_and_five_contact_points() {
    let m = kestrel();
    assert_eq!(m.gear.legs().len(), 3);
    assert_eq!(m.gear.contacts().len(), 5);
    let nose = &m.gear.legs()[0];
    assert!(nose.braked && !nose.main);
    assert!((nose.steer_max_rad - 60f64.to_radians()).abs() < 1e-12);
    assert!(m.gear.legs()[1].main && m.gear.legs()[2].main);
    assert!(m.gear.anti_skid, "anti-skid is on by default");
    assert!(m.gear.reach_m() > 7.0 && m.gear.reach_m() < 12.0);
    assert!(GearModel::none().is_empty());
}

#[test]
fn the_static_pose_carries_the_weight_nose_up_with_the_designed_compressions() {
    let m = kestrel();
    let (s, pose) = at_rest(&m);
    let mass = m.mass.props(&s.fuel);
    // Weight is on the struts, the residual is a newton at most.
    assert!(pose.residual_n.abs() < 1.0, "{}", pose.residual_n);
    // ADR-35: about 3 degrees nose up on the gear at the 60 % fuel weight.
    assert!(
        (pose.pitch_rad.to_degrees() - 3.2).abs() < 0.3,
        "{}",
        pose.pitch_rad.to_degrees()
    );
    assert!((pitch_deg(&s, &SphereSea::default()) - pose.pitch_rad.to_degrees()).abs() < 1e-9);
    let [nose, main_l, main_r, none] = pose.compression_m;
    assert!((nose - 0.071).abs() < 0.01, "nose {nose}");
    assert!((main_l - 0.120).abs() < 0.01, "main {main_l}");
    assert_eq!(main_l, main_r, "symmetric");
    assert_eq!(none, 0.0);
    // The nose carries about a tenth of the weight.
    let f = forces(
        &m.gear,
        &s.body,
        &mass,
        &GearHeld::DOWN,
        &SphereSea::default(),
        0.0,
    );
    let share = f.n_leg_n[0] / f.n_total_n;
    assert!((0.08..0.14).contains(&share), "nose share {share}");
    assert!((f.n_total_n - mass.mass_kg * G0).abs() < 1.0);
    assert_eq!(f.n_main_n, f.n_leg_n[1] + f.n_leg_n[2]);
}

#[test]
fn the_pose_follows_the_lift_and_refuses_what_cannot_balance() {
    let m = kestrel();
    let world = SphereSea::default();
    let (s0, p0) = at_rest(&m);
    let weight = m.mass.props(&s0.fuel).mass_kg * G0;
    // With 40 % of the weight on the wing the struts are lighter and the aircraft sits higher.
    let (s1, p1) = m
        .static_state(
            3100.0 * 0.6,
            0.4 * weight,
            0.0,
            DVec3::ZERO,
            &world,
            EngineCommand::IDLE,
        )
        .unwrap();
    assert!(s1.body.pos_bf.z > s0.body.pos_bf.z);
    assert!(p1.compression_m[1] < p0.compression_m[1]);
    // No weight on the wheels at all, or none to carry: no pose.
    assert!(
        m.static_state(
            1000.0,
            weight * 2.0,
            0.0,
            DVec3::ZERO,
            &world,
            EngineCommand::IDLE
        )
        .is_none()
    );
    let mass = m.mass.props(&s0.fuel);
    assert!(static_pose(&GearModel::none(), &mass, 1e5, 0.0, DVec3::ZERO, &world).is_none());
}

#[test]
fn far_above_the_ground_nothing_is_evaluated() {
    let m = kestrel();
    let (s, _) = at_rest(&m);
    let mass = m.mass.props(&s.fuel);
    let world = SphereSea::default();
    let f = forces(&m.gear, &s.body, &mass, &GearHeld::DOWN, &world, 5000.0);
    assert_eq!(f, GroundForces::ZERO);
    let f = forces(
        &GearModel::none(),
        &s.body,
        &mass,
        &GearHeld::DOWN,
        &world,
        0.0,
    );
    assert_eq!(f, GroundForces::ZERO);
    // NaN height is treated as out of reach, not as a reason to compute.
    let f = forces(&m.gear, &s.body, &mass, &GearHeld::DOWN, &world, f64::NAN);
    assert_eq!(f, GroundForces::ZERO);
    // With the gear up the legs push nothing even though the airframe is still above the points.
    let up = GearHeld {
        gear_pos: 0.0,
        ..GearHeld::DOWN
    };
    let f = forces(&m.gear, &s.body, &mass, &up, &world, 0.0);
    assert_eq!(
        f.n_total_n, 0.0,
        "belly and tail are clear at the static height"
    );
}

#[test]
fn the_ground_forces_hold_the_aircraft_in_the_derivative_and_the_accelerometer() {
    let m = kestrel();
    let world = SphereSea::default();
    let (mut s, _) = at_rest(&m);
    let (d, _) = m.derivative(&s, &world, 0.0);
    // Vertically the struts hold the weight; the idle thrust pushes it forward (4.7 kN / 10 t).
    assert!(
        d.vel.z.abs() < 0.05,
        "the struts hold the weight: {:?}",
        d.vel
    );
    assert!(d.vel.y > 0.3 && d.vel.y < 0.6, "{:?}", d.vel);
    // Raise the gear: nothing holds it and it falls at g.
    s.surfaces.gear = 0.0;
    let (d, _) = m.derivative(&s, &world, 0.0);
    assert!((d.vel.z + G0).abs() < 0.05, "{:?}", d.vel);
    // One tick on the gear: the accelerometer reads 1 g and weight is on the wheels.
    let (mut s, _) = at_rest(&m);
    let out = m.step_tick(
        &mut s,
        &cmd(all_brakes(1.0), EngineCommand::IDLE, 1.0),
        &world,
        0.0,
    );
    assert!((out.nz_g - 1.0).abs() < 0.02, "{}", out.nz_g);
    assert!(s.ground.wow);
    let t = m.telemetry(&s, &world, 0.0);
    assert!(t.wow && t.agl_m < 0.0 && t.agl_m > -0.3, "{}", t.agl_m);
    assert!((t.nz_g - 1.0).abs() < 0.02);
    assert!(t.ground_speed_mps < 0.05 && t.vs_mps.abs() < 0.05);
}

// ---------------------------------------------------------------------------------------------
// Staying put, rolling, braking and steering.

#[test]
fn the_parking_brake_holds_the_aircraft_at_idle() {
    let m = kestrel();
    let world = SphereSea::default();
    let (mut s, pose) = at_rest(&m);
    let start = s.body.pos_bf;
    let c = cmd(all_brakes(1.0), EngineCommand::IDLE, 1.0);
    // The engine spools to idle and pushes with its idle thrust the whole minute.
    run(&m, &mut s, &c, &world, 60 * 60);
    let drift = (s.body.pos_bf - start).length();
    assert!(drift < 0.5, "drifted {drift} m in 60 s");
    let pitch = pitch_deg(&s, &world);
    assert!((pitch - pose.pitch_rad.to_degrees()).abs() < 0.2, "{pitch}");
    let t = m.telemetry(&s, &world, 0.0);
    assert!(t.thrust_n > 3000.0, "idle thrust is real: {}", t.thrust_n);
    assert!(s.ground.wow && !s.ground.skid);
    assert_eq!(s.ground.collapsed, [false; MAX_GEAR]);
}

#[test]
fn without_brakes_idle_thrust_rolls_the_aircraft_forward() {
    let m = kestrel();
    let world = SphereSea::default();
    let (mut s, _) = at_rest(&m);
    let c = cmd(WheelCommands::NONE, EngineCommand::IDLE, 1.0);
    run(&m, &mut s, &c, &world, 10 * 60);
    let t = m.telemetry(&s, &world, 0.0);
    // 4.7 kN against the rolling resistance of 0.02 W (2 kN): a = 0.27 m/s^2.
    assert!(
        t.ground_speed_mps > 1.0 && t.ground_speed_mps < 5.0,
        "{}",
        t.ground_speed_mps
    );
    assert!(s.ground.wow);
}

/// Total ground force along the aircraft's direction of travel over the total normal force.
fn friction_ratio(m: &AircraftModel, s: &AircraftState, held: &GearHeld, world: &SphereSea) -> f64 {
    let mass = m.mass.props(&s.fuel);
    let f = forces(&m.gear, &s.body, &mass, held, world, 0.0);
    let f_bf = s.body.att.rotate(f.force_body_n);
    let up = world.up_bf(s.body.pos_bf);
    let along = s.body.vel_bf.normalized_or(DVec3::Y);
    -(f_bf - up * f_bf.dot(up)).dot(along) / f.n_total_n
}

#[test]
fn braking_gives_mu_roll_plus_point_nine_mu_brake_of_the_weight() {
    let m = kestrel();
    let world = SphereSea::default();
    let (mut s, _) = at_rest(&m);
    // Rolling straight ahead at 30 m/s on the pose: every wheel is braked (the nose too).
    s.body.vel_bf = s.body.att.rotate(DVec3::X) * 30.0;
    s.body.vel_bf = s.body.vel_bf - DVec3::Z * s.body.vel_bf.z;
    let mut held = GearHeld::DOWN;
    held.wheels = all_brakes(1.0);
    let ratio = friction_ratio(&m, &s, &held, &world);
    // GDD 10.4 / 11.13.2: mu_r 0.02 + 0.9 x 0.70.
    assert!((ratio - 0.65).abs() < 0.005, "{ratio}");
    // Half the brake is about half the braking part; no brake leaves only the rolling part.
    held.wheels = all_brakes(0.5);
    assert!((friction_ratio(&m, &s, &held, &world) - (0.02 + 0.315)).abs() < 0.005);
    held.wheels = WheelCommands::NONE;
    assert!((friction_ratio(&m, &s, &held, &world) - 0.02).abs() < 0.001);
}

#[test]
fn a_braked_aircraft_stops_in_the_distance_the_friction_gives() {
    let m = kestrel();
    let world = SphereSea::default();
    let (mut s, _) = at_rest(&m);
    s.body.vel_bf = DVec3::Y * 40.0;
    s.engines[0] = m.engines()[0].model.steady(EngineCommand::IDLE);
    let start = s.body.pos_bf;
    let c = cmd(all_brakes(1.0), EngineCommand::IDLE, 1.0);
    let mut t = 0.0;
    while m.telemetry(&s, &world, 0.0).ground_speed_mps > 0.5 && t < 30.0 {
        run(&m, &mut s, &c, &world, 1);
        t += TICK;
    }
    let dist = (s.body.pos_bf - start).length();
    // a = 0.65 g less idle thrust (0.46 m/s^2) and drag, a little lift relief: about 6 m/s^2.
    let expected = 40.0 * 40.0 / (2.0 * 6.0);
    assert!(
        (dist - expected).abs() < 0.1 * expected,
        "{dist} m (expected {expected})"
    );
    assert!(t < 10.0);
    assert!(s.ground.wow && !s.ground.skid);
}

#[test]
fn the_steering_angle_sets_the_turn_radius() {
    let m = kestrel();
    let world = SphereSea::default();
    let (mut s, _) = at_rest(&m);
    s.body.vel_bf = DVec3::Y * 8.0;
    let steer = 10f64.to_radians();
    let c = cmd(
        WheelCommands {
            brake: [0.0; MAX_GEAR],
            steer_rad: steer,
        },
        EngineCommand::IDLE,
        1.0,
    );
    run(&m, &mut s, &c, &world, 20 * 60);
    let t = m.telemetry(&s, &world, 0.0);
    // Curvature r / V of a tricycle: tan(delta) / wheelbase (4.58 + 0.62 = 5.2 m), to a few
    // percent for the CG sitting ahead of the axle.
    let curvature = s.body.omega_body.z / t.ground_speed_mps;
    let expected = dmath::tan(steer) / 5.2;
    assert!(
        (curvature - expected).abs() < 0.06 * expected,
        "{curvature} vs {expected}"
    );
    // Positive steering is a right turn: the yaw rate is positive (clockwise from above).
    assert!(s.body.omega_body.z > 0.1, "{:?}", s.body.omega_body);
    // A command past the leg's range is clamped, and the applied angle is recorded.
    let big = cmd(
        WheelCommands {
            steer_rad: 2.0,
            ..WheelCommands::NONE
        },
        EngineCommand::IDLE,
        1.0,
    );
    m.step_tick(&mut s, &big, &world, 0.0);
    assert!((s.ground.steer_rad - 60f64.to_radians()).abs() < 1e-12);
}

#[test]
fn sideways_friction_is_smooth_through_zero_and_does_not_chatter() {
    let m = kestrel();
    let world = SphereSea::default();
    let (mut s, _) = at_rest(&m);
    // A sideways slide of 0.4 m/s with the parking brake on stops in a third of a second and
    // the aircraft rocks on its struts for a couple of seconds more, with falling amplitude.
    s.body.vel_bf = DVec3::X * 0.4;
    let c = cmd(all_brakes(1.0), EngineCommand::IDLE, 1.0);
    let mut peak_after_2s = 0.0_f64;
    let mut peak_after_4s = 0.0_f64;
    for i in 0..(20 * 60) {
        let t = f64::from(i) * TICK;
        let out = m.step_tick(&mut s, &c, &world, t);
        assert_eq!(out.faults, 0);
        let v = s.body.vel_bf.x;
        if t >= 2.0 {
            peak_after_2s = peak_after_2s.max(v.abs());
        }
        if t >= 4.0 {
            peak_after_4s = peak_after_4s.max(v.abs());
        }
    }
    // The rock dies away: chatter would hold its amplitude instead.
    assert!(peak_after_2s < 0.02, "{peak_after_2s}");
    assert!(peak_after_4s < 1e-3, "{peak_after_4s}");
}

// ---------------------------------------------------------------------------------------------
// The friction law itself.

fn wheel(n_force: f64, v: DVec3, brake: f64, anti_skid: bool) -> Wheel {
    Wheel {
        n_force,
        n: DVec3::Z,
        e_long: DVec3::Y,
        v_bf: v,
        friction: friction(crate::surface::SurfaceKind::Paved, 0.0),
        brake,
        anti_skid,
    }
}

#[test]
fn friction_opposes_the_motion_with_the_gdd_coefficients() {
    // 100 kN on dry asphalt moving along the wheel at 20 m/s.
    let on = |brake: f64, anti_skid: bool, v: f64| {
        let f = wheel_friction(&wheel(1e5, DVec3::Y * v, brake, anti_skid));
        -f.y / 1e5
    };
    assert!((on(0.0, true, 20.0) - 0.02).abs() < 1e-9, "rolling only");
    assert!(
        (on(1.0, true, 20.0) - (0.02 + 0.63)).abs() < 1e-9,
        "anti-skid: 0.9 x 0.7"
    );
    assert!(
        (on(0.5, false, 20.0) - (0.02 + 0.35)).abs() < 1e-9,
        "no anti-skid, half brake"
    );
    // Hard braking fast without anti-skid locks the wheel: 0.6 of the grip.
    assert!(
        (on(1.0, false, 20.0) - (0.02 + 0.42)).abs() < 1e-9,
        "lock-up"
    );
    // Below 5 m/s the wheel does not lock: the full 0.7, then the circle caps it at mu.
    assert!(
        (on(1.0, false, 3.0) - 0.7).abs() < 1e-9,
        "no lock-up when slow, capped by the circle"
    );
    // Backwards the force reverses, and at rest it is zero (the tanh is odd).
    assert!(wheel_friction(&wheel(1e5, DVec3::Y * -20.0, 1.0, true)).y > 0.0);
    assert_eq!(
        wheel_friction(&wheel(1e5, DVec3::ZERO, 1.0, true)),
        DVec3::ZERO
    );
}

#[test]
fn the_friction_circle_limits_braking_and_cornering_together() {
    let n = 1e5;
    // Braking hard while sliding sideways: the sum cannot exceed mu N.
    for side in [0.1, 0.5, 1.0, 5.0, 30.0] {
        let f = wheel_friction(&wheel(n, DVec3::new(side, 20.0, 0.0), 1.0, true));
        let mag = f.length() / n;
        assert!(mag <= 0.7 + 1e-12, "side {side}: {mag}");
        assert!(f.x < 0.0 && f.y < 0.0, "both oppose the motion");
    }
    // A pure sideways slide at 5 m/s is a full smooth Coulomb: tanh(10) of mu.
    let f = wheel_friction(&wheel(n, DVec3::new(5.0, 0.0, 0.0), 0.0, true));
    assert!((-f.x / n - 0.7).abs() < 1e-6);
    // At v_s the sideways force is tanh(1) of its limit (the GDD law).
    let f = wheel_friction(&wheel(n, DVec3::new(0.5, 0.0, 0.0), 0.0, true));
    assert!((-f.x / n - 0.7 * dmath::tanh(1.0)).abs() < 1e-9);
    // Nothing on the ground normal: the force lies in the ground plane.
    assert_eq!(
        wheel_friction(&wheel(n, DVec3::new(1.0, 1.0, 3.0), 1.0, true)).z,
        0.0
    );
}

#[test]
fn the_steering_angle_turns_the_wheels_axes() {
    let m = kestrel();
    let (s, _) = at_rest(&m);
    let n = DVec3::Z;
    let straight = rolling_direction(&s.body, 0.0, n);
    let right = rolling_direction(&s.body, 30f64.to_radians(), n);
    // The body forward axis is north (+y) here; 30 degrees right turns it towards east.
    assert!(straight.y > 0.99);
    assert!(
        right.x > 0.45 && right.x < 0.52 && right.y > 0.8,
        "{right:?}"
    );
    assert!((right.length() - 1.0).abs() < 1e-12);
    assert!(right.dot(n).abs() < 1e-12, "in the ground plane");
}

// ---------------------------------------------------------------------------------------------
// Touchdown criteria (GDD 11.13.3).

/// Drops the aircraft level on its gear from just above the pad at `sink` m/s, with no forward
/// speed, and returns what the ground did and the strut compressions.
fn drop(m: &AircraftModel, sink: f64) -> (GroundEvents, AircraftState, f64) {
    let world = SphereSea::default();
    let (mut s, _) = at_rest(m);
    s.ground.air_time_s = 1.0e3;
    // 0.14 m up: the mains (compressed 0.12 m) are 2 cm clear, so they arrive at `sink`.
    s.body.pos_bf.z += 0.14;
    s.body.vel_bf = DVec3::new(0.0, 0.0, -(sink * sink - 2.0 * G0 * 0.02).max(0.0).sqrt());
    let c = cmd(all_brakes(1.0), EngineCommand::IDLE, 1.0);
    let mut ev = GroundEvents::NONE;
    let mut deepest = 0.0_f64;
    let mass = m.mass.props(&s.fuel);
    for i in 0..(4 * 60) {
        let out = m.step_tick(&mut s, &c, &world, f64::from(i) * TICK);
        assert_eq!(out.faults, 0);
        ev.merge(&out.events);
        for leg in m.gear.legs() {
            let p = probe(&s.body, leg.pos_ext_body_m - mass.cg_body_m, &world);
            deepest = deepest.max(p.depth / leg.stroke_m);
        }
        if ev.destroyed.is_some() {
            break;
        }
    }
    (ev, s, deepest)
}

#[test]
fn a_gentle_touchdown_is_normal() {
    let m = kestrel();
    let (ev, s, deepest) = drop(&m, 1.0);
    assert_eq!(ev.touchdowns, 1);
    assert_eq!((ev.hard_landings, ev.collapses, ev.destroyed), (0, 0, None));
    assert!(
        (ev.touchdown_vs_mps - 1.0).abs() < 0.2,
        "{}",
        ev.touchdown_vs_mps
    );
    assert!(deepest < 0.75, "the struts do not bottom: {deepest}");
    assert!(s.ground.wow && s.ground.hard_landing_s == 0.0);
}

#[test]
fn three_metres_per_second_is_still_normal_and_does_not_bottom_the_struts() {
    let m = kestrel();
    let (ev, _, deepest) = drop(&m, 2.9);
    assert_eq!(ev.touchdowns, 1);
    assert_eq!((ev.hard_landings, ev.collapses, ev.destroyed), (0, 0, None));
    assert!(
        deepest < 1.0,
        "a normal landing stays inside the stroke: {deepest}"
    );
}

#[test]
fn three_and_a_half_metres_per_second_is_a_hard_landing() {
    let m = kestrel();
    let (ev, s, _) = drop(&m, 3.5);
    assert_eq!(ev.touchdowns, 1);
    assert_eq!(ev.hard_landings, 1);
    assert_eq!((ev.collapses, ev.destroyed), (0, None));
    assert!(
        (ev.touchdown_vs_mps - 3.5).abs() < 0.3,
        "{}",
        ev.touchdown_vs_mps
    );
    assert!((s.ground.touchdown_vs_mps - ev.touchdown_vs_mps).abs() < 1e-12);
    // The flag lasts three seconds from the touchdown and then clears.
    assert!(
        s.ground.hard_landing_s == 0.0,
        "four seconds on, it has cleared"
    );
}

#[test]
fn five_metres_per_second_collapses_the_struts_but_not_the_aircraft_when_slow() {
    let m = kestrel();
    let (ev, s, _) = drop(&m, 5.0);
    assert!(ev.collapses >= 2, "the mains give way: {}", ev.collapses);
    assert_eq!(
        ev.destroyed, None,
        "no forward speed: a belly landing, not a wreck"
    );
    assert!(s.ground.collapsed.iter().any(|&c| c));
    assert!(s.ground.skid, "it is on its belly");
}

#[test]
fn eight_metres_per_second_destroys_the_aircraft() {
    let m = kestrel();
    let (ev, _, _) = drop(&m, 8.0);
    assert_eq!(ev.destroyed, Some(CrashCause::SinkRate));
    assert_eq!(CrashCause::SinkRate.code(), 2);
}

#[test]
fn a_collapsed_strut_at_speed_destroys_the_aircraft() {
    let m = kestrel();
    let world = SphereSea::default();
    let (mut s, _) = at_rest(&m);
    s.ground.air_time_s = 1.0e3;
    s.body.pos_bf.z += 0.14;
    s.body.vel_bf = DVec3::new(0.0, 60.0, -5.4);
    let c = cmd(WheelCommands::NONE, EngineCommand::IDLE, 1.0);
    let mut ev = GroundEvents::NONE;
    for i in 0..120 {
        let out = m.step_tick(&mut s, &c, &world, f64::from(i) * TICK);
        ev.merge(&out.events);
        if ev.destroyed.is_some() {
            break;
        }
    }
    assert_eq!(ev.destroyed, Some(CrashCause::GearCollapse));
}

#[test]
fn airframe_contact_at_25_mps_is_a_skid_and_at_35_mps_it_is_fatal() {
    let m = kestrel();
    let world = SphereSea::default();
    let belly_on = |speed: f64| {
        let (mut s, _) = at_rest(&m);
        // Gear up, wings level, the belly point a few centimetres into the pad.
        s.surfaces.gear = 0.0;
        s.ground = GroundState::AIRBORNE;
        s.body.att = crate::frames::attitude(DVec3::Z, Euler::default());
        s.body.pos_bf = DVec3::new(0.0, 0.0, 1.37);
        s.body.vel_bf = DVec3::Y * speed;
        let c = cmd(WheelCommands::NONE, EngineCommand::IDLE, 0.0);
        let mut ev = GroundEvents::NONE;
        for i in 0..30 {
            let out = m.step_tick(&mut s, &c, &world, f64::from(i) * TICK);
            ev.merge(&out.events);
            if ev.destroyed.is_some() {
                break;
            }
        }
        (ev, s)
    };
    let (ev, s) = belly_on(25.0);
    assert_eq!(ev.destroyed, None);
    assert!(s.ground.skid);
    assert!(
        s.body.vel_bf.y < 25.0,
        "the skid slows it down: {}",
        s.body.vel_bf.y
    );
    let (ev, _) = belly_on(35.0);
    assert_eq!(ev.destroyed, Some(CrashCause::AirframeImpact));
}

#[test]
fn a_leg_meeting_the_ground_in_transit_collapses() {
    let m = kestrel();
    let world = SphereSea::default();
    let (mut s, _) = at_rest(&m);
    // The gear is a hair short of locked down, with the wheels in the ground.
    s.surfaces.gear = 0.985;
    let ev = run(
        &m,
        &mut s,
        &cmd(WheelCommands::NONE, EngineCommand::IDLE, 0.985),
        &world,
        1,
    );
    assert_eq!(ev.collapses, 3);
    assert_eq!(&s.ground.collapsed[..3], &[true; 3]);
}

#[test]
fn the_cg_through_the_ground_is_tunnelling_not_a_crash() {
    let m = kestrel();
    let world = SphereSea::default();
    let (mut s, _) = at_rest(&m);
    s.body.pos_bf.z = -6.0;
    // The guard fires from the state alone; a tick this deep would be a numerical fault first.
    let mass = m.mass.props(&s.fuel);
    let mut ground = s.ground;
    let ev = events(
        &m.gear,
        &s.body,
        &s.body,
        &mass,
        &GearHeld::DOWN,
        &mut ground,
        &world,
        1.0 / 120.0,
    );
    assert_eq!(ev.destroyed, Some(CrashCause::Tunnelling));
    assert_eq!(CrashCause::Tunnelling.code(), 4);
}

#[test]
fn a_bounce_needs_half_a_second_in_the_air_to_count_as_a_new_touchdown() {
    let m = kestrel();
    let world = SphereSea::default();
    let (mut s, _) = at_rest(&m);
    s.ground.air_time_s = 1.0e3;
    s.body.pos_bf.z += 0.3;
    s.body.vel_bf = DVec3::Z * -1.5;
    let ev = run(
        &m,
        &mut s,
        &cmd(WheelCommands::NONE, EngineCommand::IDLE, 1.0),
        &world,
        6 * 60,
    );
    assert_eq!(ev.touchdowns, 1, "settling on the struts is one touchdown");
}

// ---------------------------------------------------------------------------------------------
// Energy, stability and the events' bookkeeping.

fn strut_energy(model: &GearModel, st: &RigidState, mass: &MassProps, world: &SphereSea) -> f64 {
    let mut e = 0.0;
    for leg in model.legs() {
        let x = probe(st, leg.pos_ext_body_m - mass.cg_body_m, world)
            .depth
            .max(0.0);
        let u = x / leg.stroke_m;
        e += leg.k_npm * (0.5 * x * x + 0.25 * x * x * u * u);
    }
    e
}

fn total_energy(m: &AircraftModel, s: &AircraftState, world: &SphereSea) -> f64 {
    let mass = m.mass.props(&s.fuel);
    let rot = 0.5
        * s.body
            .omega_body
            .dot(mass.inertia_kgm2.mul_vec(s.body.omega_body));
    0.5 * mass.mass_kg * s.body.vel_bf.length_squared()
        + mass.mass_kg * G0 * s.body.pos_bf.z
        + rot
        + strut_energy(&m.gear, &s.body, &mass, world)
}

#[test]
fn an_undamped_bounce_conserves_energy_and_a_damped_one_loses_it_steadily() {
    let world = SphereSea::default();
    let drop_energy = 10_260.0 * G0 * 0.15;
    // The Kestrel dropped 15 cm onto its struts, engine out so its thrust adds no energy, with
    // the damping of every strut scaled by `damping`.
    let bounce = |damping: f64, seconds: usize| {
        let mut m = kestrel();
        for leg in &mut m.gear.legs[..m.gear.n_legs] {
            leg.c_comp_nspm *= damping;
            leg.c_ext_nspm *= damping;
        }
        let (mut s, _) = at_rest(&m);
        let rest = total_energy(&m, &s, &world);
        s.body.pos_bf.z += 0.15;
        s.ground.air_time_s = 1.0e3;
        s.engines[0].running = false;
        s.engines[0].core = 0.0;
        let c = cmd(WheelCommands::NONE, EngineCommand::IDLE, 1.0);
        let e0 = total_energy(&m, &s, &world);
        let mass = m.mass.props(&s.fuel);
        let mut series = Vec::new();
        for i in 0..(seconds * 60) {
            let out = m.step_tick(&mut s, &c, &world, i as f64 * TICK);
            assert_eq!(out.faults, 0);
            let free = strut_energy(&m.gear, &s.body, &mass, &world) == 0.0;
            series.push((total_energy(&m, &s, &world), free));
        }
        (rest, e0, series)
    };
    // No damper at all: when the aircraft first leaves the ground again it has the energy it
    // fell with, to half a percent of the drop (the rolling resistance of the rocking wheels
    // takes a little), and it never gains any.
    let (_, e0, series) = bounce(0.0, 3);
    let first_lift = series
        .iter()
        .enumerate()
        .skip(10)
        .find(|(_, (_, free))| *free)
        .map(|(i, _)| i)
        .expect("it bounces clear of the ground");
    let dev = (series[first_lift].0 - e0).abs() / drop_energy;
    assert!(
        dev < 0.005,
        "first bounce changed the energy by {} %",
        100.0 * dev
    );
    let gain = series
        .iter()
        .map(|(e, _)| e - e0)
        .fold(f64::NEG_INFINITY, f64::max);
    assert!(gain < 0.005 * drop_energy, "gained {gain} J");
    // With the real dampers the energy only falls, and ten seconds on the aircraft sits still
    // with the energy of the static pose.
    let (rest, e0, series) = bounce(1.0, 10);
    let mut last = e0;
    for (e, _) in &series {
        assert!(
            *e <= last + 1e-4 * drop_energy,
            "energy rose from {last} to {e}"
        );
        last = *e;
    }
    assert!(
        (last - rest).abs() < 0.02 * drop_energy,
        "{last} vs the static {rest}"
    );
}

#[test]
fn ten_minutes_of_random_taxiing_stays_finite_and_on_the_ground() {
    let m = kestrel();
    let world = SphereSea::default();
    let (mut s, _) = at_rest(&m);
    let mut rng = 0x2545_F491_4F6C_DD1D_u64;
    let mut next = || {
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        (rng >> 11) as f64 / (1u64 << 53) as f64
    };
    let mut c = cmd(WheelCommands::NONE, EngineCommand::IDLE, 1.0);
    let mut worst_pitch = 0.0_f64;
    let mut worst_speed = 0.0_f64;
    for i in 0..(600 * 60) {
        if i % 30 == 0 {
            c.wheels = WheelCommands {
                brake: [next(), next(), next(), 0.0],
                steer_rad: (next() * 2.0 - 1.0) * 60f64.to_radians(),
            };
            c.engine = EngineCommand::from_lever(next() * 0.35, m.ab_stages());
        }
        let out = m.step_tick(&mut s, &c, &world, f64::from(i) * TICK);
        assert_eq!(out.faults, 0, "tick {i}");
        assert_eq!(out.events.destroyed, None, "tick {i}");
        assert!(s.body.is_finite());
        let t = m.telemetry(&s, &world, 0.0);
        worst_pitch = worst_pitch.max(pitch_deg(&s, &world).abs());
        worst_speed = worst_speed.max(t.ground_speed_mps);
        assert!(
            s.body.pos_bf.z.abs() < 5.0,
            "tick {i}: z {}",
            s.body.pos_bf.z
        );
    }
    assert!(worst_pitch < 10.0, "{worst_pitch}");
    assert!(worst_speed < 60.0, "{worst_speed}");
    assert_eq!(s.ground.collapsed, [false; MAX_GEAR]);
}

#[test]
fn wheel_commands_are_sanitised() {
    let w = WheelCommands {
        brake: [f64::NAN, 2.0, -1.0, 0.5],
        steer_rad: f64::NAN,
    }
    .sanitized();
    assert_eq!(w.brake, [0.0, 1.0, 0.0, 0.5]);
    assert_eq!(w.steer_rad, 0.0);
}

#[test]
fn events_merge_by_adding_and_keeping_the_first_cause() {
    let mut a = GroundEvents {
        touchdowns: 1,
        destroyed: Some(CrashCause::SinkRate),
        touchdown_vs_mps: 2.0,
        ..GroundEvents::NONE
    };
    a.merge(&GroundEvents {
        touchdowns: 1,
        hard_landings: 1,
        collapses: 2,
        destroyed: Some(CrashCause::AirframeImpact),
        touchdown_vs_mps: 5.0,
    });
    assert_eq!((a.touchdowns, a.hard_landings, a.collapses), (2, 1, 2));
    assert_eq!(a.destroyed, Some(CrashCause::SinkRate));
    assert_eq!(a.touchdown_vs_mps, 5.0);
    assert_eq!(CrashCause::AirframeImpact.code(), 1);
    assert_eq!(CrashCause::GearCollapse.code(), 3);
}

#[test]
fn the_gear_comes_from_content_and_is_refused_when_it_does_not_fit() {
    let r = registry();
    let h = r.aircraft_handles().next().unwrap();
    let mut ac = r.aircraft(h).unwrap().clone();
    assert!(GearModel::from_def(&ac).is_ok());
    let leg = ac.geometry.gear[0].clone();
    ac.geometry.gear = vec![leg.clone(); MAX_GEAR + 1];
    assert!(matches!(
        GearModel::from_def(&ac),
        Err(ModelError::TooMany {
            what: "gear legs",
            ..
        })
    ));
    ac.geometry.gear = vec![content::aircraft::GearLeg {
        stroke_m: 0.0,
        ..leg
    }];
    assert!(matches!(
        GearModel::from_def(&ac),
        Err(ModelError::Invalid(_))
    ));
    let mut ac = r.aircraft(h).unwrap().clone();
    let c = ac.geometry.contacts[0].clone();
    ac.geometry.contacts = vec![c; MAX_CONTACTS + 1];
    assert!(matches!(
        GearModel::from_def(&ac),
        Err(ModelError::TooMany {
            what: "contact points",
            ..
        })
    ));
}

#[test]
fn the_lowest_point_is_the_radar_altitude_of_the_feet() {
    let m = kestrel();
    let world = SphereSea::default();
    let (mut s, _) = at_rest(&m);
    let mass = m.mass.props(&s.fuel);
    let on = lowest_point_agl_m(&m.gear, &s.body, &mass, &world);
    assert!((-0.2..0.0).contains(&on), "{on}");
    s.body.pos_bf.z += 100.0;
    let up = lowest_point_agl_m(&m.gear, &s.body, &mass, &world);
    assert!((up - (on + 100.0)).abs() < 1e-6);
    let none = lowest_point_agl_m(&GearModel::none(), &s.body, &mass, &world);
    assert!(
        (none - s.body.pos_bf.z).abs() < 1e-9,
        "no gear: the CG height"
    );
}
