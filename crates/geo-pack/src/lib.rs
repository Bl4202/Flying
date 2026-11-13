//! `GeoPack` v1 (`.gpk`, ADR-54): the archive writer and the full reader with inflate.
//!
//! The byte layout and the fixed-size parsing are in [`geo::gpk`]; this crate adds what needs
//! DEFLATE and the header JSON:
//!
//! * [`tile`]: [`DecodedTile`] and the blob codecs (row deltas, raw DEFLATE, CRC-32C).
//! * [`writer`]: [`ArchiveWriter`], deterministic (directory and data in tile-id order).
//! * [`reader`]: [`Archive`], which checks every rule of the format before returning a tile.
//! * [`json`]: the header JSON, written with sorted keys and no whitespace so rebuilding from the
//!   same inputs gives the same bytes.
//!
//! Not a sim crate: the simulation never reads an archive. The pipeline, `fg` and the tests do.
#![deny(missing_docs)]

pub mod json;
pub mod reader;
pub mod tile;
pub mod writer;

pub use json::{HeaderJson, SourceFile, SourceSet};
pub use reader::Archive;
pub use tile::DecodedTile;
pub use writer::ArchiveWriter;
