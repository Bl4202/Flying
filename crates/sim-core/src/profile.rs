//! Per-stage timing (GDD 63.4 "stage profiler", 60.2).
//!
//! Off by default; when on, every system run is bracketed by two clock reads. The clock is
//! injected (`fn() -> f64` returning milliseconds) because the core has no clock of its own: a
//! WebAssembly module imports one from the host, a native binary uses `Instant`. Timings are
//! observations and never feed state or hashes.

use crate::layout::{counter, profile as p};
use crate::schedule::Stage;

/// Monotonic milliseconds from the host.
pub type ClockFn = fn() -> f64;

/// Weight of the newest sample in the moving averages.
const EWMA_ALPHA: f64 = 0.05;

/// Collects stage timings across ticks.
#[derive(Debug)]
pub struct Profiler {
    clock: ClockFn,
    stage_ms: [f64; Stage::COUNT],
    tick_start_ms: f64,
    stage_last: [f32; Stage::COUNT],
    stage_ewma: [f32; Stage::COUNT],
    tick_last: f32,
    tick_ewma: f32,
    ticks: u64,
}

impl Profiler {
    /// A profiler reading time from `clock`.
    #[must_use]
    pub fn new(clock: ClockFn) -> Self {
        Self {
            clock,
            stage_ms: [0.0; Stage::COUNT],
            tick_start_ms: 0.0,
            stage_last: [0.0; Stage::COUNT],
            stage_ewma: [0.0; Stage::COUNT],
            tick_last: 0.0,
            tick_ewma: 0.0,
            ticks: 0,
        }
    }

    /// Reads the clock.
    #[must_use]
    pub fn now(&self) -> f64 {
        (self.clock)()
    }

    /// Start of a tick.
    pub fn begin_tick(&mut self) {
        self.stage_ms = [0.0; Stage::COUNT];
        self.tick_start_ms = self.now();
    }

    /// Adds `ms` to a stage's total for the current tick.
    pub fn add_stage_ms(&mut self, stage: Stage, ms: f64) {
        self.stage_ms[stage.index()] += ms;
    }

    /// End of a tick: records the stage and whole-tick times and updates the averages.
    pub fn end_tick(&mut self) {
        let total = (self.now() - self.tick_start_ms).max(0.0);
        let first = self.ticks == 0;
        let blend = |old: f32, new: f64| -> f32 {
            if first {
                new as f32
            } else {
                (f64::from(old) + EWMA_ALPHA * (new - f64::from(old))) as f32
            }
        };
        for i in 0..Stage::COUNT {
            let ms = self.stage_ms[i].max(0.0);
            self.stage_last[i] = ms as f32;
            self.stage_ewma[i] = blend(self.stage_ewma[i], ms);
        }
        self.tick_last = total as f32;
        self.tick_ewma = blend(self.tick_ewma, total);
        self.ticks += 1;
    }

    /// Ticks measured so far.
    #[must_use]
    pub fn ticks(&self) -> u64 {
        self.ticks
    }

    /// Writes the profile block (layout in [`crate::layout::profile`]).
    pub fn write_block(&self, out: &mut [u8; p::SIZE], counters: &[u32; p::COUNTER_COUNT]) {
        out.fill(0);
        out[p::TICKS..p::TICKS + 8].copy_from_slice(&self.ticks.to_le_bytes());
        out[p::TICK_MS_LAST..p::TICK_MS_LAST + 4].copy_from_slice(&self.tick_last.to_le_bytes());
        out[p::TICK_MS_EWMA..p::TICK_MS_EWMA + 4].copy_from_slice(&self.tick_ewma.to_le_bytes());
        for i in 0..Stage::COUNT {
            let last = p::STAGE_MS_LAST + i * 4;
            let avg = p::STAGE_MS_EWMA + i * 4;
            out[last..last + 4].copy_from_slice(&self.stage_last[i].to_le_bytes());
            out[avg..avg + 4].copy_from_slice(&self.stage_ewma[i].to_le_bytes());
        }
        write_counters(out, counters);
    }
}

/// Writes only the counters into a profile block (used while profiling is off).
pub fn write_counters(out: &mut [u8; p::SIZE], counters: &[u32; p::COUNTER_COUNT]) {
    for (i, c) in counters.iter().enumerate() {
        let at = p::COUNTERS + i * 4;
        out[at..at + 4].copy_from_slice(&c.to_le_bytes());
    }
}

/// Collects the counters in the order of [`crate::layout::counter`].
#[must_use]
pub fn counter_array(
    input: crate::input::InputCounters,
    world: crate::world::Counters,
) -> [u32; p::COUNTER_COUNT] {
    let mut c = [0_u32; p::COUNTER_COUNT];
    c[counter::INPUT_APPLIED] = input.applied;
    c[counter::INPUT_NONFINITE] = input.nonfinite;
    c[counter::INPUT_STALE] = input.stale;
    c[counter::INPUT_OVERFLOW] = input.overflow;
    c[counter::OBSERVER_RESETS] = world.observer_resets;
    c[counter::GROUND_CLAMPS] = world.ground_clamps;
    c[counter::BOUNDARY_CLAMPS] = world.boundary_clamps;
    c[counter::SNAPSHOTS] = world.snapshots;
    c
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::sync::atomic::{AtomicU64, Ordering};

    static FAKE_US: AtomicU64 = AtomicU64::new(0);

    fn fake_clock() -> f64 {
        FAKE_US.load(Ordering::SeqCst) as f64 / 1000.0
    }

    fn f32_at(b: &[u8], o: usize) -> f32 {
        f32::from_le_bytes(b[o..o + 4].try_into().unwrap())
    }

    fn advance_us(us: u64) {
        FAKE_US.fetch_add(us, Ordering::SeqCst);
    }

    // The clock is a global, so everything that touches it lives in one test.
    #[test]
    fn stage_times_accumulate_and_average() {
        FAKE_US.store(0, Ordering::SeqCst);
        let mut pr = Profiler::new(fake_clock);

        pr.begin_tick();
        advance_us(250);
        pr.add_stage_ms(Stage::Dynamics, 0.25);
        advance_us(100);
        pr.add_stage_ms(Stage::Publish, 0.1);
        pr.add_stage_ms(Stage::Dynamics, 0.05);
        pr.end_tick();
        assert_eq!(pr.ticks(), 1);

        let mut block = [0_u8; p::SIZE];
        pr.write_block(&mut block, &[0; p::COUNTER_COUNT]);
        assert_eq!(u64::from_le_bytes(block[0..8].try_into().unwrap()), 1);
        assert!((f32_at(&block, p::TICK_MS_LAST) - 0.35).abs() < 1e-6);
        assert!(
            (f32_at(&block, p::STAGE_MS_LAST + 4 * Stage::Dynamics.index()) - 0.30).abs() < 1e-6
        );
        assert!(
            (f32_at(&block, p::STAGE_MS_LAST + 4 * Stage::Publish.index()) - 0.10).abs() < 1e-6
        );
        assert_eq!(
            f32_at(&block, p::STAGE_MS_LAST + 4 * Stage::Ingest.index()),
            0.0
        );
        // The first sample seeds the average.
        assert!((f32_at(&block, p::TICK_MS_EWMA) - 0.35).abs() < 1e-6);

        // A second, slower tick moves the average a twentieth of the way.
        pr.begin_tick();
        advance_us(1_350);
        pr.end_tick();
        pr.write_block(&mut block, &[0; p::COUNTER_COUNT]);
        let expected = 0.35 + 0.05 * (1.35 - 0.35);
        assert!((f32_at(&block, p::TICK_MS_EWMA) - expected as f32).abs() < 1e-5);
    }

    #[test]
    fn counters_land_at_their_documented_slots() {
        let input = crate::input::InputCounters {
            applied: 1,
            nonfinite: 2,
            stale: 3,
            overflow: 4,
        };
        let world = crate::world::Counters {
            observer_resets: 5,
            ground_clamps: 6,
            boundary_clamps: 7,
            snapshots: 8,
        };
        let arr = counter_array(input, world);
        assert_eq!(arr, [1, 2, 3, 4, 5, 6, 7, 8]);
        let mut block = [0xFF_u8; p::SIZE];
        write_counters(&mut block, &arr);
        for (i, want) in arr.iter().enumerate() {
            let at = p::COUNTERS + i * 4;
            assert_eq!(
                u32::from_le_bytes(block[at..at + 4].try_into().unwrap()),
                *want
            );
        }
    }
}
