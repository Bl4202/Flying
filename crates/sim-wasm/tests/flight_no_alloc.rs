//! A flight-model tick allocates nothing (GDD 61.4): the model owns its tables from load, and the
//! tick works on `Copy` state and fixed-size arrays. Faults and their rollback included.
//!
//! This tests the `flight` crate on its own; it lives here because the counting allocator needs
//! `unsafe`, which only this crate permits. `no_alloc.rs` covers the whole sim tick.
#![allow(unsafe_code)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::path::Path;

use content::Registry;
use flight::actuators::Surfaces;
use flight::aircraft::{AircraftModel, Commands};
use flight::body::RigidState;
use flight::controls::PilotControls;
use flight::engine::EngineCommand;
use flight::env::SphereSea;
use flight::frames::{Euler, attitude};
use flight::gear::WheelCommands;
use flight::physio::{GEffects, PhysioParams, PhysioState};
use flight::structure::StructureState;
use flight::trim::{TrimProblem, TrimSetup, guess_level, solve, state_of};
use simmath::dmath;
use simmath::math::DVec3;

thread_local! {
    static ALLOCATIONS: Cell<u64> = const { Cell::new(0) };
}

struct Counting;

// SAFETY: defers to `System` for every operation; the counter is a plain thread-local `Cell`
// with a const initialiser, so counting never allocates.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.with(|n| n.set(n.get() + 1));
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.with(|n| n.set(n.get() + 1));
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        ALLOCATIONS.with(|n| n.set(n.get() + 1));
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

fn allocations() -> u64 {
    ALLOCATIONS.with(Cell::get)
}

/// The Kestrel, from the baked core pack.
fn kestrel() -> AircraftModel {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content/core");
    let baked = content_bake::bake::bake_dir(&dir).expect("the core pack bakes");
    let registry = Registry::from_cpack(&baked.cpack).unwrap();
    let handle = registry.aircraft_handles().next().unwrap();
    AircraftModel::from_registry(&registry, handle, 1.0 / 60.0).unwrap()
}

#[test]
fn a_tick_allocates_nothing() {
    let model = kestrel();
    let world = SphereSea::default();
    let body = RigidState {
        pos_bf: DVec3::new(0.0, 0.0, 5000.0),
        vel_bf: DVec3::new(0.0, 200.0, 0.0),
        att: attitude(
            DVec3::Z,
            Euler {
                pitch_rad: 0.06,
                ..Euler::default()
            },
        ),
        omega_body: DVec3::ZERO,
    };
    let spawn = model.state(body, 2000.0, EngineCommand::from_lever(0.6, 4));
    let mut s = spawn;

    let before = allocations();
    for tick in 0..1000_u32 {
        let phase = f64::from(tick) * 0.05;
        let controls = PilotControls {
            pitch: dmath::sin(phase) * 0.6,
            roll: dmath::cos(phase) * 0.4,
            throttle: 0.6 + 0.4 * dmath::sin(phase * 0.1),
            trim_nose_up: tick % 200 < 50,
            ..PilotControls::default()
        };
        let mut cmd = model.manual_commands(&mut s, &controls);
        if tick == 500 {
            // A transient fault: rolled back and retried with neutral surfaces.
            cmd.surfaces.elevator_rad = f64::NAN;
        }
        model.step_tick(&mut s, &cmd, &world, f64::from(tick) / 60.0);
        let t = model.telemetry(&s, &world, f64::from(tick) / 60.0);
        assert!(t.air.tas_mps.is_finite());
    }
    // A persistent fault, then the in-place respawn a caller would do: trimmed level flight.
    s.body.omega_body = DVec3::new(99.0, 0.0, 0.0);
    let calm = Commands {
        surfaces: Surfaces::NEUTRAL,
        engine: EngineCommand::IDLE,
        wheels: WheelCommands::NONE,
    };
    assert_eq!(model.step_tick(&mut s, &calm, &world, 20.0).faults, 2);
    let setup = TrimSetup {
        pos_bf: spawn.body.pos_bf,
        fuel: spawn.fuel,
        config: Surfaces::NEUTRAL,
    };
    let guess = guess_level(&model, &setup, 200.0, 1.0, &world);
    let trim = solve(&model, &TrimProblem::level(), &setup, guess, &world);
    assert!(trim.converged);
    s = state_of(&model, &setup, &trim.point, &world);
    model.step_tick(&mut s, &calm, &world, 20.0);
    assert_eq!(allocations() - before, 0, "the tick allocated");
}

/// The ground model allocates nothing either: the static-pose solver, taxiing with brakes and
/// steering, every touchdown class (normal, hard, collapse, destroyed), a belly skid and the
/// tunnelling guard.
#[test]
fn the_ground_model_allocates_nothing() {
    let model = kestrel();
    let world = SphereSea::default();
    let fuel_kg = 1800.0;

    let before = allocations();

    // The static pose, solved on the real forces.
    let (rest, pose) = model
        .static_state(fuel_kg, 0.0, 0.3, DVec3::ZERO, &world, EngineCommand::IDLE)
        .expect("the Kestrel settles on its gear");
    assert!(pose.residual_n.abs() < 1.0);

    // Taxi: brakes and steering varying, the parking brake and a stop.
    let mut s = rest;
    s.ground.air_time_s = 0.0;
    let thrust = EngineCommand::from_lever(0.5, 4);
    for tick in 0..600_u32 {
        let phase = f64::from(tick) * 0.03;
        let wheels = WheelCommands {
            brake: [0.5 + 0.5 * dmath::sin(phase); flight::gear::MAX_GEAR],
            steer_rad: 0.5 * dmath::cos(phase),
        };
        let cmd = Commands {
            surfaces: Surfaces {
                gear: 1.0,
                ..Surfaces::NEUTRAL
            },
            engine: thrust,
            wheels,
        };
        let out = model.step_tick(&mut s, &cmd, &world, f64::from(tick) / 60.0);
        assert_eq!(out.faults, 0);
    }

    // Touchdowns at 2, 3.5, 5 and 8 m/s: normal, hard, strut collapse, destroyed.
    let settle = Commands {
        surfaces: Surfaces {
            gear: 1.0,
            ..Surfaces::NEUTRAL
        },
        engine: EngineCommand::IDLE,
        wheels: WheelCommands {
            brake: [1.0; flight::gear::MAX_GEAR],
            steer_rad: 0.0,
        },
    };
    let mut destroyed = 0;
    for sink in [2.0_f64, 3.5, 5.0, 8.0] {
        let mut s = rest;
        s.ground.air_time_s = 1.0e3;
        s.body.pos_bf.z += 0.14;
        s.body.vel_bf = DVec3::new(
            0.0,
            0.0,
            -(sink * sink - 2.0 * 9.80665 * 0.02).max(0.0).sqrt(),
        );
        for tick in 0..120_u32 {
            let out = model.step_tick(&mut s, &settle, &world, f64::from(tick) / 60.0);
            assert_eq!(out.faults, 0);
            if out.events.destroyed.is_some() {
                destroyed += 1;
                break;
            }
        }
    }
    assert_eq!(destroyed, 1, "only the 8 m/s drop is destroyed");

    // A belly skid at 25 m/s with the gear up.
    let mut s = rest;
    s.surfaces.gear = 0.0;
    s.ground.air_time_s = 1.0e3;
    s.body.att = attitude(DVec3::Z, Euler::default());
    s.body.pos_bf = DVec3::new(0.0, 0.0, 1.37);
    s.body.vel_bf = DVec3::Y * 25.0;
    let up = Commands {
        surfaces: Surfaces::NEUTRAL,
        engine: EngineCommand::IDLE,
        wheels: WheelCommands::NONE,
    };
    for tick in 0..30_u32 {
        model.step_tick(&mut s, &up, &world, f64::from(tick) / 60.0);
    }

    // The CG far below the ground: the guard, not a numerical fault.
    let mut s = rest;
    s.body.pos_bf.z = -9.0;
    let out = model.step_tick(&mut s, &settle, &world, 0.0);
    assert!(out.events.destroyed.is_some() || out.faults > 0);

    assert_eq!(allocations() - before, 0, "the ground model allocated");
}

/// The pilot's physiology, the structure model, the onboard control model and the departure
/// diagnostics are plain arithmetic on `Copy` state.
#[test]
fn physiology_structure_and_the_control_model_allocate_nothing() {
    let model = kestrel();
    let world = SphereSea::open();
    let body = RigidState {
        pos_bf: DVec3::new(0.0, 0.0, 5000.0),
        vel_bf: DVec3::new(0.0, 200.0, 0.0),
        att: attitude(DVec3::Z, Euler::default()),
        omega_body: DVec3::ZERO,
    };
    let mut s = model.state(body, 2000.0, EngineCommand::from_lever(0.6, 4));
    let mass = model.mass.props(&s.fuel);
    let mut physio = PhysioState::REST;
    let mut structure = StructureState::NEW;
    let params = PhysioParams::GDD;

    let before = allocations();
    let mut flags = 0_u32;
    for tick in 0..2000_u32 {
        let t = model.telemetry(&s, &world, f64::from(tick) / 60.0);
        // A long hard pull (G-LOC, recovery, G-LOC again) and some negative g.
        let nz = if tick < 1200 { 9.0 } else { -2.0 };
        let effects = physio.step(nz, 1.0 / 60.0, &params, GEffects::Full);
        flags += u32::from(effects.gloc) + u32::from(effects.blackout);
        let mut input = model.structure_input(&s, &t);
        input.nz_g = nz;
        let f = structure.step(&model.limits, &input, 1.0 / 60.0);
        flags += u32::from(f.overstress) + u32::from(f.gear_overspeed);
        let air = flight::aero::AeroInput {
            air: t.air,
            alpha_dot_rps: 0.0,
            surfaces: s.surfaces,
            height_agl_m: 5000.0,
            cg_body_m: mass.cg_body_m,
        };
        let cm = model.aero.control_model(&air);
        assert!(cm.dm_de.is_finite());
        let row = flight::departure::row(&model.aero, &mass, f64::from(tick % 90));
        assert!(row.cn_beta_dyn.is_finite());
        s.surfaces.elevator_rad = 1e-3 * f64::from(tick % 7);
    }
    assert!(flags > 0, "the effects were exercised");
    assert_eq!(
        allocations() - before,
        0,
        "physiology or structure allocated"
    );
}
