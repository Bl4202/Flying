//! Exported C ABI. See `UNSAFE.md` for the invariants that make the global state sound.
#![allow(unsafe_code)]

use core::cell::UnsafeCell;

use sim_core::SimCore;

/// Bumped on any incompatible change to the exports below.
pub const ABI_VERSION: u32 = 1;

struct Global(UnsafeCell<Option<SimCore>>);

// SAFETY: only the Sim Worker thread calls the exported functions (UNSAFE.md, "Single caller").
unsafe impl Sync for Global {}

static CORE: Global = Global(UnsafeCell::new(None));

fn with_core<R>(f: impl FnOnce(&mut Option<SimCore>) -> R) -> R {
    // SAFETY: single caller, non-re-entrant, no reference escapes (UNSAFE.md).
    unsafe { f(&mut *CORE.0.get()) }
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

fn subsystem(kind: u32) -> u64 {
    with_core(|c| {
        c.as_ref().map_or(0, |core| {
            let h = core.subsystem_hashes();
            match kind {
                0 => h.combined,
                1 => h.tick,
                2 => h.rng,
                3 => h.bodies,
                _ => 0,
            }
        })
    })
}

/// ABI version of this module.
#[unsafe(no_mangle)]
pub extern "C" fn fg_abi_version() -> u32 {
    ABI_VERSION
}

/// Creates (or replaces) the core from a 64-bit seed given as two halves, and starts it.
/// Returns 0 on success.
#[unsafe(no_mangle)]
pub extern "C" fn fg_init(seed_lo: u32, seed_hi: u32) -> i32 {
    with_core(|c| {
        let mut core = SimCore::new(join(seed_lo, seed_hi));
        core.start();
        *c = Some(core);
    });
    0
}

/// Runs up to `max_ticks` ticks; returns how many ran (0 if not initialised).
#[unsafe(no_mangle)]
pub extern "C" fn fg_step(max_ticks: u32) -> u32 {
    with_core(|c| c.as_mut().map_or(0, |core| core.step(max_ticks)))
}

/// Low 32 bits of the tick counter.
#[unsafe(no_mangle)]
pub extern "C" fn fg_tick_lo() -> u32 {
    with_core(|c| c.as_ref().map_or(0, |core| lo(core.tick())))
}

/// High 32 bits of the tick counter.
#[unsafe(no_mangle)]
pub extern "C" fn fg_tick_hi() -> u32 {
    with_core(|c| c.as_ref().map_or(0, |core| hi(core.tick())))
}

/// Low 32 bits of the combined state hash.
#[unsafe(no_mangle)]
pub extern "C" fn fg_state_hash_lo() -> u32 {
    lo(subsystem(0))
}

/// High 32 bits of the combined state hash.
#[unsafe(no_mangle)]
pub extern "C" fn fg_state_hash_hi() -> u32 {
    hi(subsystem(0))
}

/// Low 32 bits of a subsystem hash: 0 combined, 1 tick, 2 rng, 3 bodies.
#[unsafe(no_mangle)]
pub extern "C" fn fg_subsystem_hash_lo(kind: u32) -> u32 {
    lo(subsystem(kind))
}

/// High 32 bits of a subsystem hash: 0 combined, 1 tick, 2 rng, 3 bodies.
#[unsafe(no_mangle)]
pub extern "C" fn fg_subsystem_hash_hi(kind: u32) -> u32 {
    hi(subsystem(kind))
}

#[cfg(test)]
mod tests {
    use super::*;

    // One test only: the exports share a process-wide global.
    #[test]
    fn abi_matches_the_core() {
        assert_eq!(fg_abi_version(), ABI_VERSION);
        assert_eq!(fg_step(10), 0, "not initialised yet");
        assert_eq!(fg_state_hash_lo(), 0);

        let seed = 0x1234_5678_9ABC_DEF0_u64;
        assert_eq!(fg_init(lo(seed), hi(seed)), 0);
        assert_eq!(fg_step(1_000), 1_000);

        let mut reference = SimCore::new(seed);
        reference.start();
        reference.step(1_000);
        let h = reference.subsystem_hashes();

        assert_eq!(join(fg_tick_lo(), fg_tick_hi()), 1_000);
        assert_eq!(join(fg_state_hash_lo(), fg_state_hash_hi()), h.combined);
        assert_eq!(
            join(fg_subsystem_hash_lo(1), fg_subsystem_hash_hi(1)),
            h.tick
        );
        assert_eq!(
            join(fg_subsystem_hash_lo(2), fg_subsystem_hash_hi(2)),
            h.rng
        );
        assert_eq!(
            join(fg_subsystem_hash_lo(3), fg_subsystem_hash_hi(3)),
            h.bodies
        );
        assert_eq!(fg_subsystem_hash_lo(99), 0);
    }
}
