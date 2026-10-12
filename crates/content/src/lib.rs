//! Content registry and the `.cpack` v1 binary format (GDD 62, 61.4.2).
//!
//! Authoring data is JSON; `content-bake` validates it and packs it into a `.cpack`. The sim
//! receives only the packed bytes and reads them through [`Registry`]. Content is accessed by
//! `u32` handles ([`ContentId`]), never by string at runtime, and is immutable once loaded.
//!
//! This crate has no dependencies, so it builds for native and WebAssembly alike and the sim can
//! load a pack without a JSON parser.
#![deny(missing_docs)]

pub mod aircraft;
pub mod cpack;
mod registry;

pub use aircraft::{AircraftDef, Category, Era};
pub use cpack::CpackError;
pub use registry::{ContentId, Registry};
