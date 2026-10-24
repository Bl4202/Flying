//! A step of the flight control system allocates nothing (GDD 61.4): every law, the limiters, the
//! ground law and its blends, mouse-aim, the recovery button, G-LOC, and switching the assist level
//! in flight or on the ground. The airframe's own tick is in `flight_no_alloc.rs`.
//!
//! It lives here because the counting allocator needs `unsafe`, which only this crate permits.
#![allow(unsafe_code)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::path::Path;

use content::Registry;
use fcs::{Aim, Assist, FcsModel, FcsState, PilotInput};
use flight::aircraft::AircraftModel;
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

/// The Kestrel and its control gains, from the baked core pack.
fn kestrel() -> (AircraftModel, FcsModel) {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content/core");
    let baked = content_bake::bake::bake_dir(&dir).expect("the core pack bakes");
    let registry = Registry::from_cpack(&baked.cpack).unwrap();
    let handle = registry.aircraft_handles().next().unwrap();
    let model = AircraftModel::from_registry(&registry, handle, 1.0 / 60.0).unwrap();
    let fcs = FcsModel::from_registry(&registry, handle, &model).unwrap();
    (model, fcs)
}

const ASSISTS: [Assist; 4] = [
    Assist::Full,
    Assist::Standard,
    Assist::Reduced,
    Assist::Manual,
];

#[test]
fn every_law_in_flight_allocates_nothing() {
    let (model, fcs) = kestrel();
    let world = SphereSea::open();
    let body = RigidState {
        pos_bf: DVec3::new(0.0, 0.0, 5000.0),
        vel_bf: DVec3::new(0.0, 200.0, 0.0),
        att: attitude(DVec3::Z, Euler::default()),
        omega_body: DVec3::ZERO,
    };

    let before = allocations();
    for (k, &start) in ASSISTS.iter().enumerate() {
        let mut ac = model.state(body, 2000.0, EngineCommand::from_lever(0.6, 4));
        let mut fs = FcsState::new(start, false);
        for tick in 0..900_u32 {
            let t = f64::from(tick) / 60.0;
            let phase = t * 1.3;
            let input = PilotInput {
                controls: PilotControls {
                    pitch: 0.7 * dmath::sin(phase),
                    roll: 0.8 * dmath::cos(phase * 0.7),
                    yaw: 0.3 * dmath::sin(phase * 0.3),
                    throttle: 0.6 + 0.4 * dmath::sin(phase * 0.1),
                    trim_nose_up: tick % 200 < 30,
                    airbrake: f64::from(u8::from(tick % 300 < 100)),
                    ..PilotControls::default()
                },
                // The cursor: a slow circle round the nose, in the middle of the run.
                aim: (300..500).contains(&tick).then(|| Aim {
                    azimuth_rad: 1.2 * dmath::sin(phase * 0.5),
                    elevation_rad: 0.6 * dmath::cos(phase * 0.5),
                }),
                // The recovery button for a while, then the pilot is out for a while.
                recover: (500..600).contains(&tick),
                gloc: (650..700).contains(&tick),
                auto_gcas: false,
            };
            if tick == 450 {
                // A bumpless switch in flight to the next level.
                fcs.switch_assist(&mut fs, ASSISTS[(k + 1) % 4]);
            }
            let cmd = fcs.step(&model, &mut ac, &mut fs, &input, &world, t);
            let out = model.step_tick(&mut ac, &cmd, &world, t);
            assert_eq!(out.faults, 0, "{start:?} tick {tick}");
        }
        assert!(fs.is_finite(), "{start:?}");
    }
    assert_eq!(allocations() - before, 0, "the control system allocated");
}

#[test]
fn the_ground_law_and_its_blends_allocate_nothing() {
    let (model, fcs) = kestrel();
    let world = SphereSea::default();

    let before = allocations();
    for &assist in &ASSISTS {
        let (mut ac, _) = model
            .static_state(1800.0, 0.0, 0.3, DVec3::ZERO, &world, EngineCommand::IDLE)
            .expect("the Kestrel settles on its gear");
        let mut fs = FcsState::new(assist, true);
        // Brakes and steering on the spot, then a take-off roll, a pull, the gear and flaps up,
        // and back down: the ground law, the blend to the flight law and the blend back.
        for tick in 0..2400_u32 {
            let t = f64::from(tick) / 60.0;
            let rolling = tick > 120;
            let input = PilotInput {
                controls: PilotControls {
                    pitch: if ac.ground.wow || tick < 900 {
                        if tick > 600 { 0.4 } else { 0.0 }
                    } else {
                        0.0
                    },
                    yaw: 0.3 * dmath::sin(t * 0.8),
                    throttle: if rolling && tick < 1500 { 1.0 } else { 0.0 },
                    gear: f64::from(u8::from(!(800..=1700).contains(&tick))),
                    flaps: f64::from(u8::from(!(1000..=1700).contains(&tick))),
                    brake_l: if (120..=1900).contains(&tick) {
                        0.0
                    } else {
                        1.0
                    },
                    brake_r: if (120..=1900).contains(&tick) {
                        0.0
                    } else {
                        1.0
                    },
                    ..PilotControls::default()
                },
                ..PilotInput::default()
            };
            let cmd = fcs.step(&model, &mut ac, &mut fs, &input, &world, t);
            let out = model.step_tick(&mut ac, &cmd, &world, t);
            assert_eq!(out.faults, 0, "{assist:?} tick {tick}");
            if out.events.destroyed.is_some() {
                break;
            }
        }
        assert!(fs.is_finite(), "{assist:?}");
    }
    assert_eq!(
        allocations() - before,
        0,
        "the ground law or its blends allocated"
    );
}
