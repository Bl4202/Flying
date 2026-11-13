//! The world formats (ADR-51 D4, ADR-53, ADR-54): `region/1`, a pilot region with its battlefield
//! and air spawn, and `geo_source/1`, a data source's licence terms (GDD 45.9). Strict serde types
//! like [`crate::model`], and the source of their JSON Schemas.
//!
//! They are **not** part of the sim pack. [`crate::world`] bakes them into `world.json`
//! (`world-bundle/1`), and a world session's numbers reach the core through the world
//! configuration block (ABI v6). Angles are authored in degrees here and sent in radians.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Tag of the region format.
pub const REGION_SCHEMA: &str = "region/1";
/// Tag of the source descriptor format.
pub const GEO_SOURCE_SCHEMA: &str = "geo_source/1";
/// Tag of the bundle the baker writes.
pub const WORLD_BUNDLE_SCHEMA: &str = "world-bundle/1";

/// `region/1`: one pilot region (plan section 9.4).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RegionFile {
    /// Format tag, always `region/1`.
    #[schemars(extend("const" = "region/1"))]
    pub schema: String,
    /// The region's id (a slug, the file's name).
    pub id: String,
    /// Its place in the main menu's Arena list, after the test arenas (1 first).
    #[schemars(range(min = 1, max = 99))]
    pub menu_order: u32,
    /// Locale key of the region's name.
    pub name_key: String,
    /// Locale key of its one-line preview.
    pub preview_key: String,
    /// The z5 cells (x, y) whose archives hold the region at z12.
    #[schemars(length(min = 1, max = 64))]
    pub cells: Vec<[u32; 2]>,
    /// Its terrain character (the reference regions of `AC-46.7`).
    pub character: Character,
    /// The default battlefield.
    pub battlefield: BattlefieldFile,
    /// The air spawn.
    pub spawn: SpawnFile,
}

/// A region's terrain character.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Character {
    /// High mountains.
    Alpine,
    /// Ranges and basins.
    Rugged,
    /// Plains.
    Flat,
    /// Hills and farmland.
    Rolling,
    /// Coast and sea.
    Coast,
    /// A dense city.
    City,
}

/// What happens when the boundary countdown runs out (GDD 8.5).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PolicyName {
    /// An autopilot turns back.
    AssistTurn,
    /// The aircraft is lost.
    MissionFail,
    /// Lost, and counted as a loss in a campaign.
    Deserter,
}

/// A battlefield (GDD 8.1, ADR-53).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BattlefieldFile {
    /// The playable area.
    pub shape: ShapeFile,
    /// The hard ceiling (GDD 8.4).
    #[schemars(extend("x-unit" = "m"), range(min = 1_000.0, max = 30_000.0))]
    pub ceiling_m: f64,
    /// The warning band; 0 means `max(2 km, 3 % of the shorter edge)` (GDD 8.5).
    #[schemars(extend("x-unit" = "m"), range(min = 0.0, max = 100_000.0))]
    pub warn_width_m: f64,
    /// The countdown.
    #[schemars(extend("x-unit" = "s"), range(min = 5.0, max = 300.0))]
    pub countdown_s: f64,
    /// The enforcement policy.
    pub policy: PolicyName,
    /// The level wanted under the spawn: 12 (regions) or 10.
    #[schemars(range(min = 10, max = 12))]
    pub spawn_level: u8,
}

/// A battlefield shape (GDD 8.2). Points are `[lat, lon]` in degrees; rotations are clockwise
/// from north.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ShapeFile {
    /// A rectangle.
    Rect {
        /// Centre `[lat, lon]`.
        #[schemars(extend("x-unit" = "deg"))]
        center_deg: [f64; 2],
        /// East–west size before rotation.
        #[schemars(extend("x-unit" = "m"), range(min = 1_000.0, max = 1_000_000.0))]
        width_m: f64,
        /// North–south size before rotation.
        #[schemars(extend("x-unit" = "m"), range(min = 1_000.0, max = 1_000_000.0))]
        height_m: f64,
        /// Clockwise rotation.
        #[schemars(extend("x-unit" = "deg"), range(min = -180.0, max = 180.0))]
        rotation_deg: f64,
    },
    /// A circle.
    Circle {
        /// Centre `[lat, lon]`.
        #[schemars(extend("x-unit" = "deg"))]
        center_deg: [f64; 2],
        /// Radius.
        #[schemars(extend("x-unit" = "m"), range(min = 600.0, max = 500_000.0))]
        radius_m: f64,
    },
    /// An ellipse.
    Ellipse {
        /// Centre `[lat, lon]`.
        #[schemars(extend("x-unit" = "deg"))]
        center_deg: [f64; 2],
        /// East–west half-axis before rotation.
        #[schemars(extend("x-unit" = "m"), range(min = 600.0, max = 500_000.0))]
        a_m: f64,
        /// North–south half-axis before rotation.
        #[schemars(extend("x-unit" = "m"), range(min = 600.0, max = 500_000.0))]
        b_m: f64,
        /// Clockwise rotation.
        #[schemars(extend("x-unit" = "deg"), range(min = -180.0, max = 180.0))]
        rotation_deg: f64,
    },
    /// A capsule (corridor).
    Capsule {
        /// One end `[lat, lon]`.
        #[schemars(extend("x-unit" = "deg"))]
        a_deg: [f64; 2],
        /// The other end `[lat, lon]`.
        #[schemars(extend("x-unit" = "deg"))]
        b_deg: [f64; 2],
        /// Half-width.
        #[schemars(extend("x-unit" = "m"), range(min = 1_000.0, max = 500_000.0))]
        radius_m: f64,
    },
    /// A simple polygon of 3 to 64 vertices `[lat, lon]`.
    Polygon {
        /// The vertices.
        #[schemars(extend("x-unit" = "deg"), length(min = 3, max = 64))]
        vertices_deg: Vec<[f64; 2]>,
    },
}

/// The air spawn.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SpawnFile {
    /// Latitude.
    #[schemars(extend("x-unit" = "deg"), range(min = -85.0, max = 85.0))]
    pub lat_deg: f64,
    /// Longitude.
    #[schemars(extend("x-unit" = "deg"), range(min = -180.0, max = 180.0))]
    pub lon_deg: f64,
    /// Altitude above mean sea level.
    #[schemars(extend("x-unit" = "m"), range(min = 0.0, max = 30_000.0))]
    pub alt_msl_m: f64,
    /// True heading, clockwise from north.
    #[schemars(extend("x-unit" = "deg"), range(min = 0.0, max = 360.0))]
    pub hdg_deg: f64,
    /// True airspeed.
    #[schemars(extend("x-unit" = "mps"), range(min = 60.0, max = 400.0))]
    pub tas_mps: f64,
}

/// `geo_source/1`: one data source's licence terms (GDD 45.9). The pipeline's licence audit reads
/// the same files (`tools/geo-pipeline/src/descriptors.rs`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[allow(clippy::struct_excessive_bools)] // DATA: GDD 45.9's descriptor fields, one flag per term
pub struct GeoSourceFile {
    /// Format tag, always `geo_source/1`.
    #[schemars(extend("const" = "geo_source/1"))]
    pub schema: String,
    /// The id archives name in their `licences` (a slug).
    pub id: String,
    /// The source's name.
    pub name: String,
    /// The licence's name.
    pub licence: String,
    /// The short attribution line.
    pub attribution: String,
    /// Where the terms are (https).
    pub url: String,
    /// The terms allow commercial use.
    pub commercial_ok: bool,
    /// The terms allow caching.
    pub cache_allowed: bool,
    /// The terms allow redistributing derived data.
    pub redistribution_allowed: bool,
    /// The terms require share-alike.
    pub share_alike: bool,
    /// When the terms were last read, `YYYY-MM-DD`.
    pub terms_checked: String,
    /// The notices the terms require, word for word; the Credits & data screen shows each.
    pub required_notices: Vec<String>,
    /// The file-name prefixes the descriptor covers.
    #[serde(default)]
    pub products: Vec<String>,
    /// For a share-alike database: where its public dump is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dump: Option<String>,
}
