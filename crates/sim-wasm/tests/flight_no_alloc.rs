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

#[test]
fn a_tick_allocates_nothing() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content/core");
    let baked = content_bake::bake::bake_dir(&dir).expect("the core pack bakes");
    let registry = Registry::from_cpack(&baked.cpack).unwrap();
    let handle = registry.aircraft_handles().next().unwrap();
    let model = AircraftModel::from_registry(&registry, handle, 1.0 / 60.0).unwrap();
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
    // A persistent fault, then the in-place respawn a caller would do.
    s.body.omega_body = DVec3::new(99.0, 0.0, 0.0);
    let calm = Commands {
        surfaces: Surfaces::NEUTRAL,
        engine: EngineCommand::IDLE,
    };
    assert_eq!(model.step_tick(&mut s, &calm, &world, 20.0).faults, 2);
    s = spawn;
    model.step_tick(&mut s, &calm, &world, 20.0);
    assert_eq!(allocations() - before, 0, "the tick allocated");
}
