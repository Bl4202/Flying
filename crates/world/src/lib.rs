//! The battlefield system (GDD 8, ADR-53): what the playable area is, where its frame is anchored,
//! how far any point is from its edge, what happens to an aircraft that leaves it, and which terrain
//! tiles a world session must hold before its first tick.
//!
//! * [`shape`]: the authored shapes (rect, circle, ellipse, capsule, polygon) and their boundary
//!   polygons in a provisional frame.
//! * [`anchor`]: the anchor (the ECEF mean of the densified boundary, GDD 8.7) and the playable
//!   area's polygon in the final battlefield frame.
//! * [`polygon`]: the BF polygon: area, extent, simplicity, winding, distance.
//! * [`sdf`]: the signed distance field of the playable area (GDD 8.2).
//! * [`preset`]: the five size classes (GDD 0.6.4) and airport-corridor sizing (GDD 8.6).
//! * [`validate`]: the battlefield validator (GDD 8.8, v0.2a's checks V1 to V8).
//! * [`boundary`]: the four boundary stages (GDD 8.5).
//! * [`plan`]: the sim terrain plan, the tiles a world session loads before it starts (GDD 44.6).
//! * [`config`]: the world configuration block the host writes (ABI v6).
//!
//! A sim crate (ADR-10): it depends only on `geo` and `simmath`, takes no randomness, iterates in
//! sorted order, and allocates only when a world is applied, never per tick.
#![deny(missing_docs)]

pub mod anchor;
pub mod boundary;
pub mod config;
pub mod plan;
pub mod polygon;
pub mod preset;
pub mod sdf;
pub mod shape;
pub mod validate;

pub use anchor::{Placed, place};
pub use boundary::{BoundaryEvent, BoundaryParams, BoundaryState, Policy, Stage};
pub use config::{CellCoverage, ConfigError, WorldConfig};
pub use plan::{PlanInputs, SimTerrainPlan};
pub use polygon::BfPolygon;
pub use preset::{Corridor, SizeClass};
pub use sdf::Sdf;
pub use shape::{LatLonRad, Shape};
pub use validate::{Reject, Validation, WarnFlags};
