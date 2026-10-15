//! The tick path allocates nothing once warmed up (GDD 61.4, ADR-26).
//!
//! A counting global allocator tallies allocations made by the test thread. After a warm-up that
//! lets every lazily grown buffer reach its working size, 1 000 ticks with a fresh input frame
//! pushed every tick must allocate zero times, with profiling off and on, and while the host reads
//! the snapshot and profile pointers the way the Sim Worker does. Then the same with the core
//! pack's aircraft flying: manoeuvres, a reset (a trim solve), a crash and the respawn.
//!
//! One test only: the ABI shares a process-wide global.
#![allow(unsafe_code)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use sim_core::InputFrame;
use sim_core::dmath;
use sim_core::layout::{axis, button, content_status, counter, input_frame, input_script};
use sim_wasm::abi::{
    fg_content_reserve, fg_init, fg_load_content, fg_profile_ptr, fg_push_inputs,
    fg_set_input_script, fg_set_profiling, fg_snapshot_ptr, fg_start, fg_state_hash_lo, fg_step,
    with_content_buffer, with_input_buffer, with_profile,
};

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

/// Stages one frame (sequence number `seq`, slowly varying axes) and pushes it.
fn push_frame(seq: u32) {
    let mut f = InputFrame::neutral(seq);
    let phase = f64::from(seq) * 0.05;
    f.axes[axis::PITCH] = (dmath::sin(phase) * 0.7) as f32;
    f.axes[axis::ROLL] = (dmath::cos(phase) * 0.5) as f32;
    f.axes[axis::THROTTLE] = 0.5;
    f.t_ms = f64::from(seq) * (1000.0 / 60.0);
    with_input_buffer(|b| f.encode((&mut b[..input_frame::SIZE]).try_into().unwrap()));
    assert_eq!(fg_push_inputs(1), 1);
}

/// Runs `ticks` ticks the way the Sim Worker does and returns the allocations they made.
fn measure(first_seq: u32, ticks: u32) -> u64 {
    let before = allocations();
    for i in 0..ticks {
        push_frame(first_seq + i);
        assert_eq!(fg_step(1), 1);
        // The host reads the snapshot every tick and the profile block at about 4 Hz.
        assert_ne!(fg_snapshot_ptr(), 0);
        if i % 15 == 0 {
            assert_ne!(fg_profile_ptr(), 0);
            let _ = fg_state_hash_lo();
        }
    }
    allocations() - before
}

#[test]
fn the_tick_path_allocates_nothing_after_warm_up() {
    // Control: the counter really counts, so the zeros below mean something.
    let before = allocations();
    drop(std::hint::black_box(Vec::<u8>::with_capacity(64)));
    assert!(
        allocations() > before,
        "the counting allocator is not counting"
    );

    assert_eq!(fg_init(0x1F1E_5EED, 0), 0);
    assert_eq!(fg_set_input_script(input_script::NONE as u32), 1);
    assert_eq!(fg_start(), 1);

    // Warm-up: first use of each queue, buffer and lazily initialised table.
    measure(1, 300);

    let steady = measure(301, 1_000);
    assert_eq!(
        steady, 0,
        "{steady} allocations in 1 000 steady-state ticks"
    );

    fg_set_profiling(1);
    measure(1_301, 300);
    let profiled = measure(1_601, 1_000);
    assert_eq!(
        profiled, 0,
        "{profiled} allocations in 1 000 profiled ticks"
    );

    // The scripted pilot goes through the same decode path and must not allocate either.
    fg_set_profiling(0);
    assert_eq!(fg_set_input_script(input_script::ERRATIC as u32), 1);
    for _ in 0..300 {
        fg_step(1);
    }
    let before = allocations();
    for _ in 0..1_000 {
        assert_eq!(fg_step(1), 1);
    }
    assert_eq!(allocations() - before, 0, "scripted pilot allocated");

    // The aircraft. Loading content and starting allocate (the model, the slot); flying does not.
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content/core");
    let pack = content_bake::bake::bake_dir(&dir)
        .expect("the core pack bakes")
        .cpack;
    assert_eq!(fg_init(0x1F1E_5EED, 0), 0);
    let len = u32::try_from(pack.len()).unwrap();
    assert_ne!(fg_content_reserve(len), 0);
    with_content_buffer(|b| b.copy_from_slice(&pack));
    assert_eq!(fg_load_content(len), content_status::OK as u32);
    assert_eq!(fg_set_input_script(input_script::NONE as u32), 1);
    assert_eq!(fg_start(), 1);
    measure(1, 300);
    let flying = measure(301, 1_000);
    assert_eq!(flying, 0, "{flying} allocations in 1 000 ticks of flight");

    // A reset (the respawn trims inside the tick), then a dive into the ground, the crash and the
    // respawn two seconds later.
    let before = allocations();
    push_frame_with(1_301, |f| f.buttons = 1 << button::RESET);
    assert_eq!(fg_step(1), 1);
    for seq in 1_302..4_302 {
        push_frame_with(seq, |f| {
            f.axes[axis::PITCH] = -1.0;
            f.axes[axis::THROTTLE] = 1.0;
        });
        assert_eq!(fg_step(1), 1);
    }
    assert_eq!(
        allocations() - before,
        0,
        "reset, crash or respawn allocated"
    );
    let read = |i: usize| {
        with_profile(|b| {
            let at = sim_core::layout::profile::COUNTERS + 4 * i;
            u32::from_le_bytes(b[at..at + 4].try_into().unwrap())
        })
        .unwrap()
    };
    assert!(read(counter::CRASHES) >= 1, "the dive reached the ground");
    assert!(
        read(counter::RESPAWNS) >= 2,
        "the reset and the post-crash respawn"
    );
    assert_eq!(read(counter::FDM_FAULTS), 0);
}

/// Stages one frame built by `f` from neutral (throttle 0.5) and pushes it.
fn push_frame_with(seq: u32, f: impl FnOnce(&mut InputFrame)) {
    let mut fr = InputFrame::neutral(seq);
    fr.axes[axis::THROTTLE] = 0.5;
    fr.t_ms = f64::from(seq) * (1000.0 / 60.0);
    f(&mut fr);
    with_input_buffer(|b| fr.encode((&mut b[..input_frame::SIZE]).try_into().unwrap()));
    assert_eq!(fg_push_inputs(1), 1);
}
