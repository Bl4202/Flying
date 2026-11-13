//! WGS84 geodesy, the battlefield frame and the world's tile maths (GDD 7, 44.3, 45; ADR-52, ADR-54).
//!
//! * [`wgs84`]: the ellipsoid's constants and radii of curvature.
//! * [`geodetic`]: geodetic ↔ ECEF (GDD 7.2.1, 7.2.2).
//! * [`frame`]: the curved battlefield frame on the ellipsoid (GDD 7.2.3, 7.2.4).
//! * [`angles`]: longitude normalisation and degree constants (GDD 7.8).
//! * [`distance`]: haversine, initial bearing, the Vincenty inverse, metres per degree, the horizon
//!   (GDD 7.3, 7.4.2).
//! * [`tile`]: tile identity, bounds, parents, children and cells (GDD 44.3, ADR-54).
//! * [`codec`]: the 16-bit height quantisation and the row-delta coding of `GeoPack` heights.
//! * [`sample`]: the bicubic (Catmull–Rom) height sampler over a tile's posts (GDD 44.6).
//! * [`crc32c`]: CRC-32C (Castagnoli), the checksum of every `GeoPack` blob.
//! * [`gpk`]: the `GeoPack` v1 byte layout and its header and directory parsing (no decompression).
//! * `testkit` (feature `testkit` or tests): the deterministic random points the gate tests use.
//!
//! Conventions (GDD 0.3.2): angles in radians, lengths in metres. ECEF has +Z through the north pole
//! and +X through (0°, 0°). BF is East-North-Up at the anchor and curved. Tile rows count from the
//! north edge and columns from the west edge. Every transcendental goes through [`simmath::dmath`]
//! (ADR-10), so the results are bit-identical natively and in WebAssembly.
#![deny(missing_docs)]

pub mod angles;
pub mod codec;
pub mod crc32c;
pub mod distance;
pub mod frame;
pub mod geodetic;
pub mod gpk;
pub mod sample;
#[cfg(any(test, feature = "testkit"))]
pub mod testkit;
pub mod tile;
pub mod wgs84;

pub use distance::{Accuracy, Geodesic};
pub use frame::Wgs84Frame;
pub use geodetic::{Geodetic, ecef_to_geodetic, geodetic_to_ecef};
pub use tile::{TileBounds, TileId};
