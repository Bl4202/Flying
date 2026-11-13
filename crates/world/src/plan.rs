//! The sim terrain plan (GDD 44.6, ADR-53): every tile a world session holds before its first tick.
//!
//! 1. The playable area expanded by 10 km; the **fine zone** is the disc of 40 km around the spawn.
//! 2. A cell's level is 12 if its archive holds z12, else 10 (a cell not listed is ocean: 10).
//! 3. Small, Medium, Large: each z10 tile meeting the expanded area. In a z12 cell its z12
//!    descendants that meet the expanded area are planned instead of it.
//! 4. Huge, Extreme: each z10 tile meeting the expanded area. In a z12 cell its z12 descendants
//!    that also meet the fine zone are planned beside it.
//! 5. Every z5 tile meeting the playable area expanded by 200 km (the fallback level).
//! 6. Sorted ascending, without duplicates. More than 8 192 tiles is `TOO_LARGE`.
//!
//! "Meets" is conservative: any of the tile's nine sample points (corners, edge midpoints,
//! centre, at h = 0) lies in the region, or a region vertex lies within the tile's bounds.
//!
//! A height lookup tries the plan's finest level, then 10, then 5 (`fine_level`, `rest_level`).
//! `fine_level` is 12 when the plan holds any z12 tile. The v0.2a plan said "when the spawn's cell
//! has z12"; that would skip the z12 tiles of a battlefield that also reaches into another cell
//! (ADR-53).

use simmath::math::DVec3;

use crate::config::CellCoverage;
use crate::polygon::BfPolygon;
use crate::preset::SizeClass;
use crate::shape::LatLonRad;
use geo::angles::delta_lon;
use geo::tile::{CELL_LEVEL, TileId, delta_rad, tile_for};
use geo::{Geodetic, Wgs84Frame};

/// The margin around the playable area, m. // DATA: ADR-53
pub const MARGIN_M: f64 = 10_000.0;
/// The fine zone's radius around the spawn, m. // DATA: ADR-53
pub const FINE_ZONE_M: f64 = 40_000.0;
/// The fallback level's margin, m. // DATA: ADR-53
pub const FALLBACK_MARGIN_M: f64 = 200_000.0;
/// The fallback level. // DATA: ADR-53
pub const FALLBACK_LEVEL: u8 = CELL_LEVEL;
/// The level everywhere the cell has no z12. // DATA: ADR-51 D1
pub const REST_LEVEL: u8 = 10;
/// The fine level. // DATA: ADR-51 D1
pub const FINE_LEVEL: u8 = 12;
/// The most tiles a plan may hold (≈ 306 MB of sim terrain). // DATA: ADR-53
pub const SIM_PLAN_MAX_TILES: usize = 8_192;

/// A region of the horizontal BF plane.
#[derive(Clone, Copy, Debug)]
pub enum Region<'a> {
    /// The polygon expanded by `margin_m`.
    Area {
        /// The polygon.
        poly: &'a BfPolygon,
        /// The expansion, m.
        margin_m: f64,
    },
    /// The disc of radius `r` around (`x`, `y`).
    Disc {
        /// Centre x, m.
        x: f64,
        /// Centre y, m.
        y: f64,
        /// Radius, m.
        r: f64,
    },
}

impl Region<'_> {
    /// Whether `p` is in the region.
    #[must_use]
    pub fn contains(&self, p: [f64; 2]) -> bool {
        match *self {
            Self::Area { poly, margin_m } => {
                poly.contains(p) || poly.boundary_distance_m(p) <= margin_m
            }
            Self::Disc { x, y, r } => (p[0] - x) * (p[0] - x) + (p[1] - y) * (p[1] - y) <= r * r,
        }
    }

    /// The region's bounding box `(x0, y0, x1, y1)`.
    #[must_use]
    pub fn bbox(&self) -> (f64, f64, f64, f64) {
        match *self {
            Self::Area { poly, margin_m } => {
                let (a, b, c, d) = poly.bbox();
                (a - margin_m, b - margin_m, c + margin_m, d + margin_m)
            }
            Self::Disc { x, y, r } => (x - r, y - r, x + r, y + r),
        }
    }

    fn any_vertex_in(&self, frame: &Wgs84Frame, t: TileId) -> bool {
        let b = t.bounds();
        let inside = |x: f64, y: f64| {
            let g = frame.bf_to_geo(DVec3::new(x, y, 0.0));
            let dl = delta_lon(b.lon_west, g.lon_rad);
            g.lat_rad <= b.lat_north
                && g.lat_rad >= b.lat_south
                && dl >= 0.0
                && dl <= delta_rad(t.z)
        };
        match *self {
            Self::Area { poly, .. } => poly.points().iter().any(|p| inside(p[0], p[1])),
            Self::Disc { x, y, .. } => inside(x, y),
        }
    }
}

/// The nine sample points of a tile at h = 0, in BF (x, y).
fn samples(frame: &Wgs84Frame, t: TileId) -> [[f64; 2]; 9] {
    let mut out = [[0.0; 2]; 9];
    let mut k = 0;
    for row in [0, 64, 128] {
        for col in [0, 64, 128] {
            let (lat, lon) = t.post_latlon(row, col);
            let p = frame.geo_to_bf(Geodetic::new(lat, lon, 0.0));
            out[k] = [p.x, p.y];
            k += 1;
        }
    }
    out
}

/// Whether tile `t` meets `region` (conservatively; module docs).
#[must_use]
pub fn tile_meets(frame: &Wgs84Frame, t: TileId, region: &Region<'_>) -> bool {
    samples(frame, t).iter().any(|p| region.contains(*p)) || region.any_vertex_in(frame, t)
}

/// Every tile of level `z` that may meet the BF box `(x0, y0, x1, y1)`: the lat/lon range of the
/// box's densified outline (and of the poles if the box holds one), padded by one tile.
pub fn candidates(frame: &Wgs84Frame, bbox: (f64, f64, f64, f64), z: u8, out: &mut Vec<TileId>) {
    const STEPS: usize = 64; // ALGO: outline samples per side; the one-tile pad covers the gaps
    let (x0, y0, x1, y1) = bbox;
    let lon0 = frame.anchor.lon_rad;
    let (mut lat_lo, mut lat_hi) = (f64::INFINITY, f64::NEG_INFINITY);
    let (mut dl_lo, mut dl_hi) = (f64::INFINITY, f64::NEG_INFINITY);
    let mut visit = |x: f64, y: f64| {
        let g = frame.bf_to_geo(DVec3::new(x, y, 0.0));
        lat_lo = lat_lo.min(g.lat_rad);
        lat_hi = lat_hi.max(g.lat_rad);
        let dl = delta_lon(lon0, g.lon_rad);
        dl_lo = dl_lo.min(dl);
        dl_hi = dl_hi.max(dl);
    };
    for k in 0..=STEPS {
        let t = k as f64 / STEPS as f64;
        visit(x0 + t * (x1 - x0), y0);
        visit(x0 + t * (x1 - x0), y1);
        visit(x0, y0 + t * (y1 - y0));
        visit(x1, y0 + t * (y1 - y0));
    }
    let mut all_lon = false;
    for pole in [core::f64::consts::FRAC_PI_2, -core::f64::consts::FRAC_PI_2] {
        let p = frame.geo_to_bf(Geodetic::new(pole, 0.0, 0.0));
        if p.x >= x0 && p.x <= x1 && p.y >= y0 && p.y <= y1 {
            all_lon = true;
            if pole > 0.0 {
                lat_hi = pole;
            } else {
                lat_lo = pole;
            }
        }
    }
    let d = delta_rad(z);
    let rows = 1u32 << z;
    let cols = 2u32 << z;
    let y_top = tile_for(lat_hi, lon0, z).y.saturating_sub(1);
    let y_bot = (tile_for(lat_lo, lon0, z).y + 1).min(rows - 1);
    let (c_start, c_count) = if all_lon || dl_hi - dl_lo + 2.0 * d >= core::f64::consts::TAU {
        (0, cols)
    } else {
        let first = tile_for(0.0, lon0 + dl_lo, z).x;
        let last = tile_for(0.0, lon0 + dl_hi, z).x;
        let span = (last + cols - first) % cols;
        ((first + cols - 1) % cols, (span + 3).min(cols))
    };
    for y in y_top..=y_bot {
        for k in 0..c_count {
            out.push(TileId {
                z,
                x: (c_start + k) % cols,
                y,
            });
        }
    }
}

/// What the plan is built from.
#[derive(Clone, Copy, Debug)]
pub struct PlanInputs<'a> {
    /// The battlefield frame.
    pub frame: &'a Wgs84Frame,
    /// The playable area.
    pub polygon: &'a BfPolygon,
    /// The battlefield's size class.
    pub class: SizeClass,
    /// The spawn.
    pub spawn: LatLonRad,
    /// The cells with data (a cell not listed is ocean, complete at every level).
    pub coverage: &'a [CellCoverage],
}

/// The finest level the archive of cell (`x`, `y`) holds; `None` for an ocean cell.
#[must_use]
pub fn cell_max_level(coverage: &[CellCoverage], x: u32, y: u32) -> Option<u8> {
    coverage
        .iter()
        .find(|c| u32::from(c.x) == x && u32::from(c.y) == y)
        .map(|c| c.max_level)
}

/// The sim terrain plan.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SimTerrainPlan {
    /// Tile ids, ascending, unique.
    pub ids: Vec<u64>,
    /// The finest level in the plan (12 or 10).
    pub fine_level: u8,
    /// The level below it (10).
    pub rest_level: u8,
}

impl SimTerrainPlan {
    /// Builds the plan (module docs).
    #[must_use]
    pub fn build(inp: &PlanInputs<'_>) -> Self {
        let f = inp.frame;
        let area = Region::Area {
            poly: inp.polygon,
            margin_m: MARGIN_M,
        };
        let s = f.geo_to_bf(inp.spawn.ground());
        let fine = Region::Disc {
            x: s.x,
            y: s.y,
            r: FINE_ZONE_M,
        };
        let big = matches!(inp.class, SizeClass::Huge | SizeClass::Extreme);
        let mut ids = Vec::new();
        let mut cand = Vec::new();
        candidates(f, area.bbox(), REST_LEVEL, &mut cand);
        for &t in &cand {
            if !tile_meets(f, t, &area) {
                continue;
            }
            let (cx, cy) = t.cell().unwrap_or((0, 0));
            let z12 = cell_max_level(inp.coverage, cx, cy).is_some_and(|m| m >= FINE_LEVEL);
            if !z12 {
                ids.push(t.id());
                continue;
            }
            if big {
                ids.push(t.id());
            }
            for c in t.children() {
                for g in c.children() {
                    if tile_meets(f, g, &area) && (!big || tile_meets(f, g, &fine)) {
                        ids.push(g.id());
                    }
                }
            }
        }
        let fallback = Region::Area {
            poly: inp.polygon,
            margin_m: FALLBACK_MARGIN_M,
        };
        cand.clear();
        candidates(f, fallback.bbox(), FALLBACK_LEVEL, &mut cand);
        for &t in &cand {
            if tile_meets(f, t, &fallback) {
                ids.push(t.id());
            }
        }
        ids.sort_unstable();
        ids.dedup();
        let fine_level = if ids
            .iter()
            .any(|&id| TileId::from_id(id).is_some_and(|t| t.z == FINE_LEVEL))
        {
            FINE_LEVEL
        } else {
            REST_LEVEL
        };
        Self {
            ids,
            fine_level,
            rest_level: REST_LEVEL,
        }
    }

    /// The levels a height lookup tries, finest first, without duplicates.
    #[must_use]
    pub fn lookup_levels(&self) -> ([u8; 3], usize) {
        if self.fine_level == self.rest_level {
            ([self.rest_level, FALLBACK_LEVEL, 0], 2)
        } else {
            ([self.fine_level, self.rest_level, FALLBACK_LEVEL], 3)
        }
    }

    /// The number of tiles of level `z`.
    #[must_use]
    pub fn count_at(&self, z: u8) -> usize {
        self.ids
            .iter()
            .filter(|&&id| TileId::from_id(id).is_some_and(|t| t.z == z))
            .count()
    }
}
