//! The battlefield validator (GDD 8.8; ADR-53, v0.2a's checks).
//!
//! | # | Check | Result |
//! |---|---|---|
//! | V1 | the polygon is simple (authored polygons), its area ≥ 1 km², its extent (largest vertex-to-vertex distance) ≤ 1 000 km, ≤ 64 authored vertices | reject |
//! | V2 | the anchor's latitude within ±85° | reject |
//! | V3 | every listed cell meeting the playable area + 10 km holds z10 or finer | reject |
//! | V4 | every listed cell meeting the fine zone holds `spawn_level` | warning `LOW_DETAIL` |
//! | V5 | the spawn is inside (`sdf < 0`) | reject |
//! | V6 | an air spawn ≥ 150 m above the planned terrain (checked by the core once tiles are in) | raised to 300 m above it, warning `SPAWN_RAISED` |
//! | V7 | the entity budget (GDD 0.6.2) | passes: one aircraft in v0.2a |
//! | V8 | a base or an air spawn exists | passes: an air spawn |
//! | — | the plan holds at most 8 192 tiles | reject `TOO_LARGE` |

use crate::anchor::Placed;
use crate::config::CellCoverage;
use crate::plan::{
    FINE_ZONE_M, MARGIN_M, Region, SIM_PLAN_MAX_TILES, candidates, cell_max_level, tile_meets,
};
use crate::sdf::Sdf;
use crate::shape::{LatLonRad, MAX_AUTHORED_VERTICES, Shape};
use geo::angles::RAD;
use geo::tile::CELL_LEVEL;

/// The least area, m². // DATA: GDD 8.8
pub const MIN_AREA_M2: f64 = 1e6;
/// The largest extent, m. // DATA: GDD 8.8
pub const MAX_EXTENT_M: f64 = 1_000_000.0;
/// The highest anchor latitude, rad. // DATA: GDD 7.8, 85°
pub const MAX_ANCHOR_LAT_RAD: f64 = 85.0 * RAD;
/// The least clearance of an air spawn, m. // DATA: ADR-53
pub const MIN_SPAWN_CLEARANCE_M: f64 = 150.0;
/// The clearance an air spawn is raised to, m. // DATA: ADR-53
pub const RAISED_SPAWN_CLEARANCE_M: f64 = 300.0;
/// The least level of a cell meeting the area. // DATA: GDD 8.8, z10
pub const MIN_COVERAGE_LEVEL: u8 = 10;

/// Why a battlefield was rejected; [`Reject::code`] is ABI v6's `world_status`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reject {
    /// Two non-adjacent edges meet.
    SelfIntersecting,
    /// The area is below 1 km².
    TooSmall,
    /// The extent is above 1 000 km, or the plan above 8 192 tiles.
    TooLarge,
    /// More than 64 authored vertices.
    TooManyVertices,
    /// The anchor is beyond ±85°.
    PolarAnchor,
    /// A cell meeting the area lacks z10.
    NoCoverage,
    /// The spawn is outside the playable area.
    SpawnOutside,
}

impl Reject {
    /// The `world_status` code (Appendix A of the v0.2a plan).
    #[must_use]
    pub const fn code(self) -> u32 {
        match self {
            Self::SelfIntersecting => 4,
            Self::TooSmall => 5,
            Self::TooLarge => 6,
            Self::TooManyVertices => 7,
            Self::PolarAnchor => 8,
            Self::NoCoverage => 9,
            Self::SpawnOutside => 10,
        }
    }
}

/// Warnings that let the session proceed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct WarnFlags(pub u32);

impl WarnFlags {
    /// The fine zone lacks `spawn_level`.
    pub const LOW_DETAIL: u32 = 1;
    /// The air spawn was raised (V6).
    pub const SPAWN_RAISED: u32 = 2;

    /// Whether `flag` is set.
    #[must_use]
    pub const fn has(self, flag: u32) -> bool {
        self.0 & flag != 0
    }
}

/// The validator's verdict.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Validation {
    /// The first failed check, if any.
    pub reject: Option<Reject>,
    /// Warnings.
    pub warn: WarnFlags,
}

/// What the static checks (V1 to V5) read.
#[derive(Clone, Copy, Debug)]
pub struct ValidateInputs<'a> {
    /// The authored shape.
    pub shape: &'a Shape,
    /// The shape placed on the Earth.
    pub placed: &'a Placed,
    /// The playable area's field.
    pub sdf: &'a Sdf,
    /// The spawn.
    pub spawn: LatLonRad,
    /// The level wanted under the spawn.
    pub spawn_level: u8,
    /// The cells with data.
    pub coverage: &'a [CellCoverage],
}

/// Whether some listed cell meeting `region` fails `ok(max_level)`.
fn a_cell_fails(inp: &ValidateInputs<'_>, region: &Region<'_>, ok: impl Fn(u8) -> bool) -> bool {
    let f = &inp.placed.frame;
    let mut cand = Vec::new();
    candidates(f, region.bbox(), CELL_LEVEL, &mut cand);
    cand.iter().any(|&t| {
        cell_max_level(inp.coverage, t.x, t.y).is_some_and(|m| !ok(m)) && tile_meets(f, t, region)
    })
}

/// The static checks V1 to V5, in that order.
#[must_use]
pub fn validate(inp: &ValidateInputs<'_>) -> Validation {
    let mut v = Validation::default();
    let poly = &inp.placed.polygon;
    let reject = if inp.shape.authored_vertices() > MAX_AUTHORED_VERTICES {
        Some(Reject::TooManyVertices)
    } else if matches!(inp.shape, Shape::Polygon { .. }) && !poly.is_simple() {
        Some(Reject::SelfIntersecting)
    } else if inp.placed.area_m2 < MIN_AREA_M2 {
        Some(Reject::TooSmall)
    } else if poly.extent_m() > MAX_EXTENT_M {
        Some(Reject::TooLarge)
    } else if inp.placed.frame.anchor.lat_rad.abs() > MAX_ANCHOR_LAT_RAD {
        Some(Reject::PolarAnchor)
    } else if a_cell_fails(
        inp,
        &Region::Area {
            poly,
            margin_m: MARGIN_M,
        },
        |m| m >= MIN_COVERAGE_LEVEL,
    ) {
        Some(Reject::NoCoverage)
    } else {
        let s = inp.placed.frame.geo_to_bf(inp.spawn.ground());
        (inp.sdf.sample(s.x, s.y) >= 0.0).then_some(Reject::SpawnOutside)
    };
    v.reject = reject;
    if reject.is_none() {
        let s = inp.placed.frame.geo_to_bf(inp.spawn.ground());
        let fine = Region::Disc {
            x: s.x,
            y: s.y,
            r: FINE_ZONE_M,
        };
        if a_cell_fails(inp, &fine, |m| m >= inp.spawn_level) {
            v.warn.0 |= WarnFlags::LOW_DETAIL;
        }
    }
    v
}

/// The plan-size check.
#[must_use]
pub fn check_plan_size(tiles: usize) -> Option<Reject> {
    (tiles > SIM_PLAN_MAX_TILES).then_some(Reject::TooLarge)
}

/// V6: the air spawn's altitude given the planned terrain under it, and whether it was raised.
#[must_use]
pub fn spawn_clearance(alt_msl_m: f64, terrain_msl_m: f64) -> (f64, bool) {
    if alt_msl_m - terrain_msl_m < MIN_SPAWN_CLEARANCE_M {
        (terrain_msl_m + RAISED_SPAWN_CLEARANCE_M, true)
    } else {
        (alt_msl_m, false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v6_raises_a_low_spawn() {
        assert_eq!(spawn_clearance(1_000.0, 800.0), (1_000.0, false));
        assert_eq!(spawn_clearance(1_000.0, 900.0), (1_200.0, true));
        assert_eq!(check_plan_size(8_192), None);
        assert_eq!(check_plan_size(8_193), Some(Reject::TooLarge));
    }
}
