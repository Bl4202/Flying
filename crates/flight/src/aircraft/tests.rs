use super::*;
use crate::env::SphereSea;
use crate::frames::{Euler, attitude};
use crate::test_content::registry;

const TICK: f64 = 1.0 / 60.0;

fn kestrel() -> AircraftModel {
    let r = registry();
    let h = r.aircraft_handles().next().unwrap();
    AircraftModel::from_registry(r, h, TICK).unwrap()
}

/// Level, heading north at `alt_m`, `speed_mps` true airspeed, pitched to `alpha_deg`.
fn level(
    m: &AircraftModel,
    alt_m: f64,
    speed_mps: f64,
    alpha_deg: f64,
    fuel_kg: f64,
) -> AircraftState {
    let up = DVec3::Z;
    let att = attitude(
        up,
        Euler {
            pitch_rad: alpha_deg.to_radians(),
            ..Euler::default()
        },
    );
    let body = RigidState {
        pos_bf: DVec3::new(0.0, 0.0, alt_m),
        vel_bf: DVec3::new(0.0, speed_mps, 0.0),
        att,
        omega_body: DVec3::ZERO,
    };
    m.state(body, fuel_kg, EngineCommand::from_lever(0.6, m.ab_stages()))
}

fn hold(m: &AircraftModel) -> Commands {
    Commands {
        surfaces: Surfaces::NEUTRAL,
        engine: EngineCommand::from_lever(0.6, m.ab_stages()),
        wheels: WheelCommands::NONE,
    }
}

#[test]
fn the_kestrel_builds_from_the_core_pack() {
    let m = kestrel();
    assert_eq!(m.engines().len(), 1);
    assert_eq!(m.ab_stages(), 4);
    assert_eq!(m.mass.tanks().len(), 4);
    assert!((m.limits.alpha_limit_rad.to_degrees() - 26.0).abs() < 1e-5);
    assert_eq!(m.aero.wing_area_m2(), 28.0);
    let missing = AircraftModel::from_registry(registry(), ContentId(99), TICK);
    assert!(matches!(missing, Err(ModelError::Missing("aircraft"))));
    let not_aircraft = registry().find("core:eng/f129").unwrap();
    assert!(AircraftModel::from_registry(registry(), not_aircraft, TICK).is_err());
}

#[test]
fn telemetry_of_a_level_state() {
    let m = kestrel();
    let world = SphereSea::default();
    // 10 000 kg at 5 000 m, 200 m/s, 4 deg: lift close to weight.
    let s = level(&m, 5000.0, 200.0, 4.0, 1600.0);
    let t = m.telemetry(&s, &world, 0.0);
    assert!((t.mass_kg - 10_000.0).abs() < 1e-9);
    assert!((t.air.alpha_rad.to_degrees() - 4.0).abs() < 1e-9);
    assert!((t.height_msl_m - 5000.0).abs() < 1e-9);
    assert!((t.air.mach - 200.0 / 320.53).abs() < 1e-3, "{}", t.air.mach);
    assert!(t.cas_mps < 200.0 && t.cas_mps > 140.0, "{}", t.cas_mps);
    // q = 0.5 * 0.7361 * 200^2 = 14 722 Pa; C_L(4 deg) = 0.251: lift 103 kN, n_z ~ 1.05.
    assert!((t.nz_g - 1.05).abs() < 0.05, "{}", t.nz_g);
    assert!(t.thrust_n > 0.0 && t.fuel_flow_kgps > 0.0);
    assert_eq!((t.ab_stage, t.stall_warning, t.flameout), (0, false, false));
    assert!(
        (t.ref_point_bf - s.body.pos_bf).length() < 1e-8,
        "CG on the reference point"
    );
    // The stall warning sounds 3 degrees short of the peak (32 deg at low Mach).
    let near = level(&m, 5000.0, 120.0, 29.5, 1600.0);
    assert!(m.telemetry(&near, &world, 0.0).stall_warning);
}

#[test]
fn a_tick_is_two_rk4_sub_steps() {
    let m = kestrel();
    let world = SphereSea::default();
    let mut s = level(&m, 3000.0, 200.0, 3.0, 2000.0);
    s.mass_frozen = true;
    let cmd = hold(&m);
    let mut manual = s;
    m.step_tick(&mut s, &cmd, &world, 0.0);
    // The same by hand: actuators, engines, then RK4 at half the tick, twice.
    let mass = m.mass.props(&manual.fuel);
    let dt = TICK / 2.0;
    for k in 0..2 {
        let t = f64::from(k) * dt;
        manual.surfaces = m.actuators.step(&manual.surfaces, &cmd.surfaces);
        let (air, atmo, _, _) = m.air_at(&manual.body, &world, t);
        for (e, st) in m.engines().iter().zip(manual.engines.iter_mut()) {
            e.model
                .step(st, cmd.engine, &engine_conditions(&air, &atmo), true);
        }
        let held = manual;
        manual.body = step_rk4(&manual.body, dt, |st| {
            m.stage(st, &held, &mass, &world, t).deriv
        });
    }
    // The ground model also counts the time in the air, once per sub-step.
    manual.ground.air_time_s += TICK;
    assert_eq!(s, manual);
    // And a tick moves the aircraft by about V dt.
    let moved = (s.body.pos_bf - level(&m, 3000.0, 200.0, 3.0, 2000.0).body.pos_bf).length();
    assert!((moved - 200.0 * TICK).abs() < 0.05, "{moved}");
}

#[test]
fn fuel_burns_in_feed_order_unless_mass_is_frozen() {
    let m = kestrel();
    // Sea level but no ground: the bench holds the aircraft at the point, wheels in the water.
    let world = SphereSea::open();
    let mut s = level(&m, 0.0, 0.5, 0.0, 3100.0);
    let mil = Commands {
        surfaces: Surfaces::NEUTRAL,
        engine: EngineCommand::MILITARY,
        wheels: WheelCommands::NONE,
    };
    s.engines[0] = m.engines()[0].model.steady(EngineCommand::MILITARY);
    let before = m.mass.fuel_kg(&s.fuel);
    for i in 0..60 {
        // Held at the start point: this measures the engine on a bench.
        s.body = level(&m, 0.0, 0.5, 0.0, 0.0).body;
        m.step_tick(&mut s, &mil, &world, f64::from(i) * TICK);
    }
    let burned = before - m.mass.fuel_kg(&s.fuel);
    // C.9 ff_mil_sl: 1.566 kg/s.
    assert!((burned - 1.566).abs() < 0.03, "{burned}");
    assert!(s.fuel[2] < 550.0, "wing tanks first");
    assert_eq!(s.fuel[0], 900.0, "fuselage tanks untouched");
    let mut frozen = s;
    frozen.mass_frozen = true;
    let fuel = frozen.fuel;
    m.step_tick(&mut frozen, &mil, &world, 1.0);
    assert_eq!(frozen.fuel, fuel);
}

#[test]
fn a_transient_fault_is_rolled_back_and_retried() {
    let m = kestrel();
    let world = SphereSea::default();
    let mut s = level(&m, 3000.0, 200.0, 3.0, 2000.0);
    let before = s;
    let mut bad = hold(&m);
    bad.surfaces.elevator_rad = f64::NAN;
    let out = m.step_tick(&mut s, &bad, &world, 0.0);
    assert_eq!(out.faults, 1);
    assert!(!s.numerical_fault);
    assert!(s.body.is_finite() && s.surfaces.is_finite());
    // The retry flew with neutral surfaces.
    let mut calm = before;
    m.step_tick(&mut calm, &hold(&m), &world, 0.0);
    assert_eq!(s, calm);
}

#[test]
fn a_persistent_fault_restores_the_state_and_flags_it() {
    let m = kestrel();
    let world = SphereSea::default();
    let mut s = level(&m, 3000.0, 200.0, 3.0, 2000.0);
    s.body.omega_body = DVec3::new(60.0, 0.0, 0.0);
    let before = s;
    let out = m.step_tick(&mut s, &hold(&m), &world, 0.0);
    assert_eq!(out.faults, 2);
    assert!(s.numerical_fault);
    assert_eq!(s.body, before.body, "the last good state");
    // A faulted aircraft does not move until the caller respawns it.
    let frozen = s;
    assert_eq!(m.step_tick(&mut s, &hold(&m), &world, 0.0).faults, 0);
    assert_eq!(s, frozen);
}

/// A small deterministic generator for the fuzz inputs.
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
}

/// Random stick, pedals, throttle and trim under the Manual law for `seconds`, respawning
/// whenever the aircraft gets near the sea. The state must stay finite and never fault. Odd seeds
/// start subsonic at 6 km (they reach about 85 deg of alpha and 14 rad/s), even seeds at
/// Mach 1.5 at 11 km.
fn fuzz(seed: u64, seconds: u32) -> u32 {
    let m = kestrel();
    let world = SphereSea::default();
    let mut rng = XorShift(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
    let spawn = if seed % 2 == 1 {
        level(&m, 6000.0, 220.0, 3.0, 2000.0)
    } else {
        level(&m, 11_000.0, 443.0, 2.0, 2000.0)
    };
    let mut s = spawn;
    let mut controls = PilotControls::default();
    let mut faults = 0;
    let mut respawns = 0;
    for tick in 0..seconds * 60 {
        if tick % 30 == 0 {
            controls = PilotControls {
                pitch: rng.signed(),
                roll: rng.signed(),
                yaw: rng.signed(),
                throttle: rng.next(),
                trim_nose_up: rng.next() < 0.1,
                trim_nose_down: rng.next() < 0.1,
                flaps: if rng.next() < 0.1 { 1.0 } else { 0.0 },
                airbrake: if rng.next() < 0.2 { 1.0 } else { 0.0 },
                gear: if rng.next() < 0.1 { 1.0 } else { 0.0 },
                brake_l: rng.next(),
                brake_r: rng.next(),
            };
        }
        let cmd = m.manual_commands(&mut s, &controls);
        faults += m
            .step_tick(&mut s, &cmd, &world, f64::from(tick) * TICK)
            .faults;
        assert!(
            !s.numerical_fault,
            "seed {seed}: numerical fault at tick {tick}"
        );
        assert!(s.body.is_finite(), "seed {seed}: non-finite at tick {tick}");
        if world.height_msl_m(s.body.pos_bf) < 300.0 {
            s = spawn;
            respawns += 1;
        }
    }
    assert!(faults == 0, "seed {seed}: {faults} faults");
    respawns
}

#[test]
fn manual_flight_under_random_inputs_stays_finite() {
    // Ten minutes per seed for eight seeds (the plan's NaN fuzz); in debug builds the first two
    // seeds, the rest under `--release` (`cargo test --release -p flight`).
    let seeds: &[u64] = if cfg!(debug_assertions) {
        &[1, 2]
    } else {
        &[1, 2, 3, 4, 5, 6, 7, 8]
    };
    for &seed in seeds {
        fuzz(seed, 600);
    }
}

#[test]
fn the_flap_fraction_and_the_structure_input_come_from_the_state() {
    let m = kestrel();
    let world = SphereSea::open();
    let mut s = level(&m, 3000.0, 150.0, 3.0, 2000.0);
    assert_eq!(m.flap_fraction(&s.surfaces), 0.0);
    s.surfaces.tef_rad = m.actuators.tef.max * 0.5;
    s.surfaces.gear = 1.0;
    assert!((m.flap_fraction(&s.surfaces) - 0.5).abs() < 1e-12);
    let t = m.telemetry(&s, &world, 0.0);
    let i = m.structure_input(&s, &t);
    assert_eq!((i.gear_pos, i.flap_fraction), (1.0, 0.5));
    assert_eq!((i.nz_g, i.cas_mps, i.mach), (t.nz_g, t.cas_mps, t.air.mach));
    // The limits for that configuration are the lowest of the two.
    assert_eq!(m.limits.n_max_for(i.gear_pos, i.flap_fraction), 4.0);
    // Over the travel is capped.
    s.surfaces.tef_rad = 10.0;
    assert_eq!(m.flap_fraction(&s.surfaces), 1.0);
}
