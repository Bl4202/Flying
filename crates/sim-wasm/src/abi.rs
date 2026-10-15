//! Exported C ABI, version 2. See `UNSAFE.md` for the invariants that make the global state sound
//! and for the rules about the pointers handed out here.
//!
//! The host (the Sim Worker) drives the core through plain functions and exchanges bytes through
//! buffers that live in WebAssembly linear memory:
//!
//! | Buffer | Direction | Obtained with | Valid until |
//! |---|---|---|---|
//! | content | host writes | `fg_content_reserve(len)` | next `fg_content_reserve`, `fg_init`, or a successful `fg_load_content` |
//! | input staging | host writes | `fg_input_ptr()` | forever |
//! | snapshot | host reads | `fg_snapshot_ptr()` | next `fg_init` |
//! | profile | host reads | `fg_profile_ptr()` (refreshes it) | next `fg_init` |
//! | terrain patch | host reads | `fg_terrain_ptr()` after `fg_terrain_fill` | next `fg_init` or `fg_terrain_fill` |
//!
//! Any call that allocates (`fg_init`, `fg_content_reserve`, `fg_load_content`, the first
//! `fg_terrain_fill`) may grow memory, which replaces the `ArrayBuffer` behind every JavaScript
//! view: re-derive views after those calls. `fg_step` and `fg_push_inputs` never allocate
//! (`tests/no_alloc.rs`).
//!
//! Layouts and status codes are in `sim_core::layout`, generated into TypeScript by
//! `cargo xtask gen-abi`.
#![allow(unsafe_code)]

use core::cell::UnsafeCell;

use sim_core::layout::{abi, content_status, core_state, hash_kind, input_frame};
use sim_core::{CoreState, LoadContentError, SimCore};

/// Bumped on any incompatible change to the exports below.
pub const ABI_VERSION: u32 = abi::VERSION as u32;

/// Bytes the input staging buffer holds: [`abi::INPUT_BATCH`] frames.
const INPUT_STAGING_BYTES: usize = abi::INPUT_BATCH * input_frame::SIZE;
/// Refuse to reserve more than this for a content pack.
const MAX_CONTENT_BYTES: usize = 64 * 1024 * 1024;

/// The wall clock the profiler reads, supplied by the host. Never feeds simulation state.
#[cfg(target_arch = "wasm32")]
mod host {
    #[link(wasm_import_module = "env")]
    unsafe extern "C" {
        #[link_name = "fg_now"]
        pub safe fn now() -> f64;
    }
}

/// Native builds (tests, the CLI) have no host clock; profiling reads zero.
#[cfg(not(target_arch = "wasm32"))]
mod host {
    pub fn now() -> f64 {
        0.0
    }
}

struct State {
    core: Option<SimCore>,
    content: Vec<u8>,
    input: [u8; INPUT_STAGING_BYTES],
}

struct Global(UnsafeCell<State>);

// SAFETY: only the Sim Worker thread calls the exported functions (UNSAFE.md, "Single caller").
unsafe impl Sync for Global {}

static STATE: Global = Global(UnsafeCell::new(State {
    core: None,
    content: Vec::new(),
    input: [0; INPUT_STAGING_BYTES],
}));

fn with_state<R>(f: impl FnOnce(&mut State) -> R) -> R {
    // SAFETY: single caller, non-re-entrant, no reference escapes (UNSAFE.md).
    unsafe { f(&mut *STATE.0.get()) }
}

/// Runs `f` on the core, or returns `None` before `fg_init`.
fn with_core<R>(f: impl FnOnce(&mut SimCore) -> R) -> Option<R> {
    with_state(|s| s.core.as_mut().map(f))
}

/// Runs `f` on the input staging buffer. This is how a native host (tests, tools) fills the
/// buffer that a WebAssembly host reaches through [`fg_input_ptr`].
#[doc(hidden)]
pub fn with_input_buffer<R>(f: impl FnOnce(&mut [u8]) -> R) -> R {
    with_state(|s| f(&mut s.input))
}

/// Runs `f` on the reserved content buffer (empty without a reservation): how a native host
/// fills what a WebAssembly host writes through the address [`fg_content_reserve`] returns.
#[doc(hidden)]
pub fn with_content_buffer<R>(f: impl FnOnce(&mut [u8]) -> R) -> R {
    with_state(|s| f(&mut s.content))
}

/// Runs `f` on the profile block (counters always, timings while profiling), or returns `None`
/// before `fg_init`: the native view of what [`fg_profile_ptr`] points at.
#[doc(hidden)]
pub fn with_profile<R>(f: impl FnOnce(&[u8]) -> R) -> Option<R> {
    with_core(|c| f(c.profile_bytes()))
}

fn join(lo: u32, hi: u32) -> u64 {
    u64::from(lo) | (u64::from(hi) << 32)
}

fn lo(v: u64) -> u32 {
    (v & 0xFFFF_FFFF) as u32
}

fn hi(v: u64) -> u32 {
    (v >> 32) as u32
}

/// An address in linear memory (all addresses fit 32 bits on `wasm32`).
fn addr(p: *const u8) -> u32 {
    p as usize as u32
}

fn clock() -> f64 {
    host::now()
}

fn subsystem(kind: u32) -> u64 {
    with_core(|core| {
        let h = core.subsystem_hashes();
        match kind as usize {
            hash_kind::COMBINED => h.combined,
            hash_kind::TICK => h.tick,
            hash_kind::RNG => h.rng,
            hash_kind::BODIES => h.bodies,
            hash_kind::OWNSHIP => h.ownship,
            hash_kind::INPUT => h.input,
            hash_kind::CONTENT => h.content,
            _ => 0,
        }
    })
    .unwrap_or(0)
}

/// ABI version of this module.
#[unsafe(no_mangle)]
pub extern "C" fn fg_abi_version() -> u32 {
    ABI_VERSION
}

/// Creates (or replaces) the core from a 64-bit seed given as two halves. The core is `Idle`:
/// load content, then call `fg_start`. Invalidates every pointer except `fg_input_ptr`. Returns 0.
#[unsafe(no_mangle)]
pub extern "C" fn fg_init(seed_lo: u32, seed_hi: u32) -> i32 {
    with_state(|s| {
        s.core = Some(SimCore::new(join(seed_lo, seed_hi)));
        s.content = Vec::new();
        s.input = [0; INPUT_STAGING_BYTES];
    });
    0
}

/// The lifecycle state (`core_state`), or `core_state::NONE` before `fg_init`.
#[unsafe(no_mangle)]
pub extern "C" fn fg_core_state() -> u32 {
    with_core(|c| c.core_state().code()).unwrap_or(core_state::NONE as u32)
}

/// Reserves `len` bytes for a content pack and returns their address, or 0 if `len` is zero or
/// above 64 MiB. Replaces any earlier reservation.
#[unsafe(no_mangle)]
pub extern "C" fn fg_content_reserve(len: u32) -> u32 {
    let len = len as usize;
    if len == 0 || len > MAX_CONTENT_BYTES {
        return 0;
    }
    with_state(|s| {
        s.content = vec![0; len];
        addr(s.content.as_ptr())
    })
}

/// Loads the first `len` bytes of the reserved buffer as a content pack. Returns a
/// `content_status`. A successful load releases the buffer.
#[unsafe(no_mangle)]
pub extern "C" fn fg_load_content(len: u32) -> u32 {
    with_state(|s| {
        let Some(core) = s.core.as_mut() else {
            return content_status::NO_CORE as u32;
        };
        let len = len as usize;
        if len == 0 || len > s.content.len() {
            return content_status::NO_BUFFER as u32;
        }
        match core.load_content(&s.content[..len]) {
            Ok(()) => {
                s.content = Vec::new();
                content_status::OK as u32
            }
            Err(LoadContentError::NotIdle) => content_status::NOT_IDLE as u32,
            Err(LoadContentError::Invalid(_) | LoadContentError::Model(_)) => {
                content_status::INVALID as u32
            }
        }
    })
}

/// `Idle -> Running`. Returns 1 if the transition happened, else 0.
#[unsafe(no_mangle)]
pub extern "C" fn fg_start() -> u32 {
    with_core(|c| u32::from(c.start())).unwrap_or(0)
}

/// `Running -> Paused`. Returns 1 if the transition happened, else 0.
#[unsafe(no_mangle)]
pub extern "C" fn fg_pause() -> u32 {
    with_core(|c| u32::from(c.pause())).unwrap_or(0)
}

/// `Paused -> Running`. Returns 1 if the transition happened, else 0.
#[unsafe(no_mangle)]
pub extern "C" fn fg_resume() -> u32 {
    with_core(|c| u32::from(c.resume())).unwrap_or(0)
}

/// Selects the scripted pilot (`input_script`). Returns 1 if accepted, else 0.
#[unsafe(no_mangle)]
pub extern "C" fn fg_set_input_script(kind: u32) -> u32 {
    with_core(|c| u32::from(c.set_input_script(kind))).unwrap_or(0)
}

/// Address of the input staging buffer: [`fg_input_capacity`] frames of 96 bytes. Constant for
/// the life of the module.
#[unsafe(no_mangle)]
pub extern "C" fn fg_input_ptr() -> u32 {
    with_state(|s| addr(s.input.as_ptr()))
}

/// Frames the staging buffer holds.
#[unsafe(no_mangle)]
pub extern "C" fn fg_input_capacity() -> u32 {
    abi::INPUT_BATCH as u32
}

/// Hands the first `count` staged frames (at most the capacity) to the core for the next tick.
/// Returns how many were accepted.
#[unsafe(no_mangle)]
pub extern "C" fn fg_push_inputs(count: u32) -> u32 {
    with_state(|s| {
        let Some(core) = s.core.as_mut() else {
            return 0;
        };
        let frames = (count as usize).min(abi::INPUT_BATCH);
        core.push_inputs(&s.input[..frames * input_frame::SIZE])
    })
}

/// Turns per-stage timing on (non-zero) or off. The clock is the imported `env.fg_now`.
#[unsafe(no_mangle)]
pub extern "C" fn fg_set_profiling(on: u32) {
    with_core(|c| c.set_profiler((on != 0).then_some(clock as fn() -> f64)));
}

/// Runs up to `max_ticks` ticks if the core is `Running`; returns how many ran.
#[unsafe(no_mangle)]
pub extern "C" fn fg_step(max_ticks: u32) -> u32 {
    with_core(|c| c.step(max_ticks)).unwrap_or(0)
}

/// Address of the newest snapshot (`fg_snapshot_len` bytes, layout `snapshot`), or 0 before
/// `fg_init`. The core rewrites it in place on every tick and every lifecycle change.
#[unsafe(no_mangle)]
pub extern "C" fn fg_snapshot_ptr() -> u32 {
    with_core(|c| addr(c.snapshot_bytes().as_ptr())).unwrap_or(0)
}

/// Size of the snapshot, bytes.
#[unsafe(no_mangle)]
pub extern "C" fn fg_snapshot_len() -> u32 {
    with_core(|c| c.snapshot_bytes().len() as u32).unwrap_or(0)
}

/// Refreshes the profile block and returns its address (`fg_profile_len` bytes, layout
/// `profile`), or 0 before `fg_init`. Timings are zero while profiling is off.
#[unsafe(no_mangle)]
pub extern "C" fn fg_profile_ptr() -> u32 {
    with_core(|c| addr(c.profile_bytes().as_ptr())).unwrap_or(0)
}

/// Size of the profile block, bytes.
#[unsafe(no_mangle)]
pub extern "C" fn fg_profile_len() -> u32 {
    with_core(|c| c.profile_bytes().len() as u32).unwrap_or(0)
}

/// Generates the terrain patch centred on BF `(centre_east_m, centre_north_m)` with the given
/// post spacing, and returns its size in bytes (0 if the arguments are unusable). May allocate.
#[unsafe(no_mangle)]
pub extern "C" fn fg_terrain_fill(centre_east_m: f64, centre_north_m: f64, spacing_m: f64) -> u32 {
    with_core(|c| c.fill_terrain(centre_east_m, centre_north_m, spacing_m) as u32).unwrap_or(0)
}

/// Address of the terrain patch from the last `fg_terrain_fill`, or 0 if there is none.
#[unsafe(no_mangle)]
pub extern "C" fn fg_terrain_ptr() -> u32 {
    with_core(|c| {
        let bytes = c.terrain_bytes();
        if bytes.is_empty() {
            0
        } else {
            addr(bytes.as_ptr())
        }
    })
    .unwrap_or(0)
}

/// Low 32 bits of the tick counter.
#[unsafe(no_mangle)]
pub extern "C" fn fg_tick_lo() -> u32 {
    with_core(|c| lo(c.tick())).unwrap_or(0)
}

/// High 32 bits of the tick counter.
#[unsafe(no_mangle)]
pub extern "C" fn fg_tick_hi() -> u32 {
    with_core(|c| hi(c.tick())).unwrap_or(0)
}

/// Low 32 bits of the combined state hash.
#[unsafe(no_mangle)]
pub extern "C" fn fg_state_hash_lo() -> u32 {
    lo(subsystem(hash_kind::COMBINED as u32))
}

/// High 32 bits of the combined state hash.
#[unsafe(no_mangle)]
pub extern "C" fn fg_state_hash_hi() -> u32 {
    hi(subsystem(hash_kind::COMBINED as u32))
}

/// Low 32 bits of a subsystem hash (`hash_kind`; unknown kinds hash to 0).
#[unsafe(no_mangle)]
pub extern "C" fn fg_subsystem_hash_lo(kind: u32) -> u32 {
    lo(subsystem(kind))
}

/// High 32 bits of a subsystem hash (`hash_kind`; unknown kinds hash to 0).
#[unsafe(no_mangle)]
pub extern "C" fn fg_subsystem_hash_hi(kind: u32) -> u32 {
    hi(subsystem(kind))
}

// `CoreState` is what `fg_core_state` reports; keep the two in step.
const _: () = {
    assert!(CoreState::Idle as usize == core_state::IDLE);
    assert!(CoreState::Ended as usize == core_state::ENDED);
};

#[cfg(test)]
mod tests {
    use super::*;
    use sim_core::InputFrame;
    use sim_core::layout::{axis, input_script, snapshot as snap};

    fn word(b: &[u8], at: usize) -> u32 {
        u32::from_le_bytes(b[at..at + 4].try_into().unwrap())
    }

    /// Copies the snapshot out of the core, as the host would from linear memory.
    fn snapshot() -> Vec<u8> {
        with_core(|c| c.snapshot_bytes().to_vec()).unwrap()
    }

    // One test only: the exports share a process-wide global.
    #[test]
    fn the_abi_drives_the_core_end_to_end() {
        // Before init everything is inert.
        assert_eq!(fg_abi_version(), 2);
        assert_eq!(fg_core_state(), core_state::NONE as u32);
        assert_eq!(
            (
                fg_step(10),
                fg_start(),
                fg_snapshot_ptr(),
                fg_state_hash_lo()
            ),
            (0, 0, 0, 0)
        );
        assert_eq!(fg_load_content(10), content_status::NO_CORE as u32);
        assert_eq!(fg_push_inputs(1), 0);
        assert_eq!(fg_terrain_fill(0.0, 0.0, 100.0), 0);

        // Init leaves the core idle; stepping does nothing until start.
        let seed = 0x1234_5678_9ABC_DEF0_u64;
        assert_eq!(fg_init(lo(seed), hi(seed)), 0);
        assert_eq!(fg_core_state(), core_state::IDLE as u32);
        assert_eq!(fg_step(5), 0);
        assert_eq!(fg_snapshot_len() as usize, snap::SIZE);
        assert_ne!(fg_snapshot_ptr(), 0);
        assert_eq!(fg_profile_len() as usize, sim_core::layout::profile::SIZE);
        assert_eq!(fg_input_capacity(), 8);
        assert_ne!(fg_input_ptr(), 0);

        // Content: needs a reservation, rejects junk, and a refused pack changes nothing.
        assert_eq!(fg_load_content(4), content_status::NO_BUFFER as u32);
        assert_eq!(fg_content_reserve(0), 0);
        assert_eq!(fg_content_reserve(u32::MAX), 0);
        assert_ne!(fg_content_reserve(64), 0);
        assert_eq!(fg_load_content(65), content_status::NO_BUFFER as u32);
        assert_eq!(fg_load_content(64), content_status::INVALID as u32);
        assert_eq!(fg_subsystem_hash_lo(hash_kind::CONTENT as u32), {
            let fresh = SimCore::new(seed);
            lo(fresh.subsystem_hashes().content)
        });

        // Lifecycle through the ABI.
        assert_eq!(fg_start(), 1);
        assert_eq!(fg_start(), 0, "already running");
        assert_eq!(fg_core_state(), core_state::RUNNING as u32);
        assert_eq!(fg_load_content(1), content_status::NOT_IDLE as u32);
        assert_eq!(fg_set_input_script(99), 0);
        assert_eq!(fg_set_input_script(input_script::NONE as u32), 1);

        // Host input: one frame through the staging buffer reaches the core.
        let mut frame = InputFrame::neutral(1);
        frame.axes[axis::PITCH] = 0.5;
        with_input_buffer(|b| frame.encode((&mut b[..input_frame::SIZE]).try_into().unwrap()));
        assert_eq!(fg_push_inputs(1), 1);
        assert_eq!(fg_step(3), 3);
        assert_eq!(word(&snapshot(), snap::H_LAST_INPUT_SEQ), 1);
        assert_eq!(
            fg_push_inputs(1000),
            8,
            "a count above the batch is clamped to it"
        );
        assert_eq!(fg_push_inputs(0), 0);

        // Pause and resume.
        assert_eq!(fg_pause(), 1);
        assert_eq!(fg_step(5), 0, "paused cores do not step");
        assert_eq!(fg_resume(), 1);

        // Profiling flows through the clock import (zero on native, but the block is produced).
        fg_set_profiling(1);
        assert_eq!(fg_step(2), 2);
        assert_ne!(fg_profile_ptr(), 0);
        fg_set_profiling(0);

        // Terrain.
        assert_eq!(fg_terrain_ptr(), 0);
        assert_eq!(
            fg_terrain_fill(0.0, 0.0, 100.0) as usize,
            sim_core::layout::terrain::SIZE
        );
        assert_ne!(fg_terrain_ptr(), 0);
        assert_eq!(fg_terrain_fill(f64::NAN, 0.0, 100.0), 0);

        // The ABI and the core agree on every hash after identical work.
        assert_eq!(fg_init(lo(seed), hi(seed)), 0);
        assert_eq!(fg_start(), 1);
        assert_eq!(fg_step(1_000), 1_000);
        let mut reference = SimCore::new(seed);
        reference.start();
        reference.step(1_000);
        let h = reference.subsystem_hashes();
        assert_eq!(join(fg_tick_lo(), fg_tick_hi()), 1_000);
        assert_eq!(join(fg_state_hash_lo(), fg_state_hash_hi()), h.combined);
        for (kind, expect) in [
            (hash_kind::TICK, h.tick),
            (hash_kind::RNG, h.rng),
            (hash_kind::BODIES, h.bodies),
            (hash_kind::OWNSHIP, h.ownship),
            (hash_kind::INPUT, h.input),
            (hash_kind::CONTENT, h.content),
        ] {
            let kind = kind as u32;
            assert_eq!(
                join(fg_subsystem_hash_lo(kind), fg_subsystem_hash_hi(kind)),
                expect
            );
        }
        assert_eq!(fg_subsystem_hash_lo(99), 0);
    }
}
