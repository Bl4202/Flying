//! The mutable state the schedule's systems operate on (GDD 61.4.2).

use crate::input::InputState;
use crate::observer::Observer;
use crate::probe::ProbeWorld;
use crate::snapshot::SnapshotBuffer;

/// Diagnostic counters kept by systems other than input (input keeps its own).
///
/// Counters are observations, not state: they are not part of the state hash.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counters {
    /// Times the observer was restored after a non-finite value.
    pub observer_resets: u32,
    /// Times the observer was lifted back above the ground.
    pub ground_clamps: u32,
    /// Times the observer was held inside the playable square.
    pub boundary_clamps: u32,
    /// Snapshots published.
    pub snapshots: u32,
}

/// Everything the tick pipeline reads and writes.
#[derive(Debug)]
pub struct World {
    /// The player's stand-in body.
    pub observer: Observer,
    /// The determinism probe: many bodies exercising every transcendental.
    pub probe: ProbeWorld,
    /// Validated controls and the queue of frames waiting for the next tick.
    pub input: InputState,
    /// Diagnostic counters.
    pub counters: Counters,
    /// The newest published snapshot.
    pub snapshot: SnapshotBuffer,
}
