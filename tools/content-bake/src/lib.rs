//! `content-bake`: turns authoring JSON into a `.cpack` (GDD 62.2, 62.6).
//!
//! The authoring formats are the serde types in [`model`], which also generate the JSON Schemas in
//! `content/schemas/` ([`schema`]). [`check`] validates a file and reports each problem with its
//! file, JSON pointer and a fix; [`bake`] packs a whole directory; [`export`] reads a pack back.
#![deny(missing_docs)]

pub mod bake;
pub mod check;
pub mod diag;
pub mod export;
pub mod gear;
pub mod model;
pub mod schema;
