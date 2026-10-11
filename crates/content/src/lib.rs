//! Content registry (stub until the baker lands in v0.0b).
//!
//! Content is accessed by `u32` handles from the baked registry, never by string at runtime
//! (GDD 61.4.2).
#![deny(missing_docs)]

/// Handle of a baked content entry (aircraft, weapon, faction, ...).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ContentId(pub u32);
