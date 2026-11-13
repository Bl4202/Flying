//! `content-bake`: turns authoring JSON into a `.cpack` (GDD 62.2, 62.6).
//!
//! The authoring formats are the serde types in [`model`], which also generate the JSON Schemas in
//! `content/schemas/` ([`schema`]). [`check`] validates a file and reports each problem with its
//! file, JSON pointer and a fix; [`bake`] packs a whole directory; [`export`] reads a pack back.
//! The presentation files (HUD, cockpit, model, font, audio) are not part of the pack: [`present`]
//! validates them and writes them to a separate bundle, so they never move the state hash.
//! The world files (regions, data-source descriptors) are baked by [`world`] into a third bundle.

#![deny(missing_docs)]

pub mod bake;
pub mod check;
pub mod combat_check;
pub mod combat_model;
pub mod diag;
pub mod export;
pub mod gear;
pub mod model;
pub mod present;
pub mod present_check;
pub mod present_model;
pub mod schema;
pub mod world;
pub mod world_model;
