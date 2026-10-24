//! The mutable state the schedule's systems operate on (GDD 61.4.2).

use crate::input::InputState;
use crate::ownship::Ownship;
use crate::probe::ProbeWorld;
use crate::seams::replay::ReplaySink;
use crate::snapshot::SnapshotBuffer;

/// Diagnostic counters kept by systems other than input (input keeps its own).
///
/// Counters are observations, not state: they are not part of the state hash.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counters {
    /// Integrator faults of the aircraft (each one a rolled-back tick, GDD 11.10).
    pub fdm_faults: u32,
    /// Times the aircraft hit the ground.
    pub crashes: u32,
    /// Respawns after a crash, a fault or a reset.
    pub respawns: u32,
    /// Spawns where the trim solver failed and the fallback was used.
    pub trim_failures: u32,
    /// Snapshots published.
    pub snapshots: u32,
    /// First wheel contacts after being airborne.
    pub touchdowns: u32,
    /// Touchdowns at a sink rate of 3.0 m/s or more.
    pub hard_landings: u32,
    /// Times the pilot was incapacitated by sustained g.
    pub gloc_events: u32,
    /// Changes of the assist level.
    pub assist_changes: u32,
    /// Spectator only: times the observer was restored after a non-finite value.
    pub observer_resets: u32,
    /// Spectator only: times the observer was lifted back above the ground.
    pub ground_clamps: u32,
    /// Spectator only: times the observer was held inside the playable square.
    pub boundary_clamps: u32,
}

/// Everything the tick pipeline reads and writes.
pub struct World {
    /// What the player flies: the aircraft, or the spectator without aircraft content.
    pub ownship: Ownship,
    /// The determinism probe: many bodies exercising every transcendental.
    pub probe: ProbeWorld,
    /// Validated controls and the queue of frames waiting for the next tick.
    pub input: InputState,
    /// Diagnostic counters.
    pub counters: Counters,
    /// The newest published snapshot.
    pub snapshot: SnapshotBuffer,
    /// Receives every snapshot published by a tick.
    pub replay: Box<dyn ReplaySink>,
}
