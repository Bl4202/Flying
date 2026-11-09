//! Content registry and the `.cpack` v4 binary format (GDD 62, 61.4.2, ADR-30, ADR-34, ADR-46).
//!
//! Authoring data is JSON; `content-bake` validates it and packs it into a `.cpack`. The sim
//! receives only the packed bytes and reads them through [`Registry`]. Content is accessed by
//! `u32` handles ([`ContentId`]), never by string at runtime, and is immutable once loaded.
//!
//! Entry kinds: aircraft ([`AircraftDef`]), engines ([`EngineDef`]), aero sets ([`AeroDef`]),
//! flight-control gains ([`FcsDef`]), weapons ([`WeaponDef`]) and AI skills ([`SkillDef`]). An
//! aircraft refers to its engines, its aero set, its control gains, its gun and its rail stores by
//! handle.
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
pub mod fcs;
mod registry;
pub mod skill;
pub mod table;
pub mod weapon;

pub use aero::AeroDef;
pub use aircraft::AircraftDef;
pub use cpack::{CpackError, Entry};
pub use engine::EngineDef;
pub use enums::{
    Category, EngineKind, EnvelopeConfig, Era, FuzeKind, GuidanceLaw, Metric, Requirement,
    SeekerKind, WarheadKind, WeaponClass,
};
pub use fcs::FcsDef;
pub use registry::{ContentId, Registry};
pub use skill::SkillDef;
pub use table::{Curve, Grid};
pub use weapon::WeaponDef;
