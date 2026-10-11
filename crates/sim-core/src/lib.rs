//! Deterministic simulation core (GDD 61.4).
//!
//! No browser, renderer, audio or network dependency. Everything that can influence state is
//! reproducible bit-for-bit across native and WebAssembly builds (ADR-10):
//!
//! * transcendental functions come only from [`dmath`] (software `libm`),
//! * randomness comes only from [`rng`] (integer generators),
//! * no hash-ordered collections and no fast-math.
#![deny(missing_docs)]

pub mod dmath;
pub mod hash;
pub mod probe;
pub mod rng;
mod sim;
pub mod time;

pub use hash::SubsystemHashes;
pub use sim::{CoreState, SimCore};
