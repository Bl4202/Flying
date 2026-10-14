//! Deterministic simulation core (GDD 61.4).
//!
//! No browser, renderer, audio or network dependency. Everything that can influence state is
//! reproducible bit-for-bit across native and WebAssembly builds (ADR-10):
//!
//! * transcendental functions come only from [`dmath`] (software `libm`),
//! * randomness comes only from [`rng`] (integer generators),
//! * no hash-ordered collections and no fast-math.
#![deny(missing_docs)]

pub mod frame_math;
pub mod hash;
pub mod input;
pub mod layout;
pub mod observer;
pub mod probe;
pub mod profile;
pub mod rng;
pub mod schedule;
pub mod seams;
pub mod services;
mod sim;
pub mod snapshot;
mod systems;
pub mod terrain;
pub mod time;
pub mod world;

/// Deterministic transcendental functions, re-exported from [`simmath`] (ADR-29).
pub use simmath::dmath;
/// Vector, quaternion and matrix types, re-exported from [`simmath`] (ADR-29).
pub use simmath::math;

/// Default master seed for tools and tests.
pub const DEFAULT_SEED: u64 = 0x1F1E_5EED_0000_0001;

pub use hash::SubsystemHashes;
pub use input::InputFrame;
pub use sim::{CoreState, LoadContentError, SimCore};
