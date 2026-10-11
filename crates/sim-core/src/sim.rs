//! The simulation core: tick counter, random streams and the probe world.

use crate::hash::{StateHasher, SubsystemHashes};
use crate::probe::ProbeWorld;
use crate::rng::RngStreams;

/// Lifecycle state of the core (GDD 61.4.1). Only `Idle -> Running` is wired in v0.0a.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum CoreState {
    /// Created, not stepping.
    Idle = 0,
    /// A mission is loading.
    Loading = 1,
    /// Stepping.
    Running = 2,
    /// Paused.
    Paused = 3,
    /// Finished.
    Ended = 4,
}

/// The deterministic simulation.
#[derive(Clone, Debug)]
pub struct SimCore {
    tick: u64,
    state: CoreState,
    rng: RngStreams,
    probe: ProbeWorld,
}

impl SimCore {
    /// Creates an idle core from a master seed.
    #[must_use]
    pub fn new(seed: u64) -> Self {
        let rng = RngStreams::new(seed);
        let probe = ProbeWorld::new(
            rng.stream("probe/init"),
            rng.stream("probe/gust"),
            rng.stream("probe/reseed"),
        );
        Self {
            tick: 0,
            state: CoreState::Idle,
            rng,
            probe,
        }
    }

    /// Moves `Idle` to `Running`. Returns false (and changes nothing) from any other state.
    pub fn start(&mut self) -> bool {
        if self.state == CoreState::Idle {
            self.state = CoreState::Running;
            true
        } else {
            false
        }
    }

    /// Runs up to `max_ticks` ticks if `Running`; returns how many ran.
    pub fn step(&mut self, max_ticks: u32) -> u32 {
        if self.state != CoreState::Running {
            return 0;
        }
        for _ in 0..max_ticks {
            self.probe.step(self.tick);
            self.tick += 1;
        }
        max_ticks
    }

    /// Ticks executed so far.
    #[must_use]
    pub fn tick(&self) -> u64 {
        self.tick
    }

    /// Current lifecycle state.
    #[must_use]
    pub fn core_state(&self) -> CoreState {
        self.state
    }

    /// The master seed.
    #[must_use]
    pub fn seed(&self) -> u64 {
        self.rng.master()
    }

    /// Read access to the probe world.
    #[must_use]
    pub fn probe(&self) -> &ProbeWorld {
        &self.probe
    }

    /// Per-subsystem hashes and the combined hash.
    #[must_use]
    pub fn subsystem_hashes(&self) -> SubsystemHashes {
        let mut t = StateHasher::new();
        t.write_u64(self.tick);
        t.write_u64(u64::from(self.state as u8));
        t.write_u64(self.rng.master());
        SubsystemHashes::new(t.finish(), self.probe.hash_rng(), self.probe.hash_bodies())
    }

    /// The combined state hash (GDD 61.4.4 `state_hash`).
    #[must_use]
    pub fn state_hash(&self) -> u64 {
        self.subsystem_hashes().combined
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn running(seed: u64) -> SimCore {
        let mut c = SimCore::new(seed);
        assert!(c.start());
        c
    }

    #[test]
    fn idle_core_does_not_step() {
        let mut c = SimCore::new(1);
        assert_eq!(c.step(10), 0);
        assert_eq!(c.tick(), 0);
        assert!(c.start());
        assert!(!c.start());
        assert_eq!(c.step(10), 10);
    }

    #[test]
    fn same_seed_same_hash_different_seed_different_hash() {
        let (mut a, mut b, mut c) = (running(5), running(5), running(6));
        a.step(2_000);
        b.step(2_000);
        c.step(2_000);
        assert_eq!(a.state_hash(), b.state_hash());
        assert_ne!(a.state_hash(), c.state_hash());
    }

    #[test]
    fn stepping_in_pieces_equals_stepping_at_once() {
        let (mut a, mut b) = (running(9), running(9));
        a.step(1_500);
        b.step(400);
        b.step(1);
        b.step(1_099);
        assert_eq!(a.tick(), b.tick());
        assert_eq!(a.subsystem_hashes(), b.subsystem_hashes());
    }

    #[test]
    fn hash_changes_every_tick() {
        let mut c = running(3);
        let mut last = c.state_hash();
        for _ in 0..50 {
            c.step(1);
            let h = c.state_hash();
            assert_ne!(h, last);
            last = h;
        }
    }

    #[test]
    fn state_stays_finite_and_quaternions_normalised() {
        let mut c = running(11);
        // 80,000 ticks: well past many reseed events, so respawned bodies are covered too.
        for _ in 0..80 {
            c.step(1_000);
            assert!(c.probe().is_finite());
            assert!(c.probe().max_quat_norm_error() < 1e-12);
        }
    }

    #[test]
    fn bodies_stay_near_the_airspace() {
        let mut c = running(21);
        c.step(20_000);
        for i in 0..crate::probe::BODY_COUNT {
            let [x, y, z] = c.probe().pos_bf(i);
            assert!(
                x.abs() < 5.0e6 && y.abs() < 5.0e6,
                "body {i} left the area: {x} {y}"
            );
            assert!((-1.0e5..1.0e5).contains(&z), "body {i} altitude {z}");
        }
    }
}
