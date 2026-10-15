//! Content registry and the `.cpack` v2 binary format (GDD 62, 61.4.2, ADR-30).
//!
//! Authoring data is JSON; `content-bake` validates it and packs it into a `.cpack`. The sim
//! receives only the packed bytes and reads them through [`Registry`]. Content is accessed by
//! `u32` handles ([`ContentId`]), never by string at runtime, and is immutable once loaded.
//!
//! Entry kinds: aircraft ([`AircraftDef`]), engines ([`EngineDef`]) and aero sets ([`AeroDef`]).
//! An aircraft refers to its engines and its aero set by handle.
//!
//! This crate has no dependencies, so it builds for native and WebAssembly alike and the sim can
//! load a pack without a JSON parser.
#![deny(missing_docs)]

pub mod aero;
pub mod aircraft;
mod codec;
pub mod cpack;
pub mod engine;
mod enums;
mod registry;
pub mod table;

pub use aero::AeroDef;
pub use aircraft::AircraftDef;
pub use cpack::{CpackError, Entry};
pub use engine::EngineDef;
pub use enums::{Category, EngineKind, EnvelopeConfig, Era, Metric, Requirement};
pub use registry::{ContentId, Registry};
pub use table::{Curve, Grid};
