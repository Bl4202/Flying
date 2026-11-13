//! Tile identity and maths of the geodetic quadtree (GDD 44.3, ADR-06, ADR-54).
//!
//! Level `z` has `2^(z+1)` columns and `2^z` rows of tiles of `Δ = 180° / 2^z` on a side; column 0
//! starts at 180° W and row 0 at the north pole. A tile holds 129 x 129 height posts (128
//! intervals, edges shared with the neighbours) and 64 x 64 landcover cells.
//!
//! **Tile id** (one `u64` per tile across all levels): `level_prefix(z) + m`, where
//! `level_prefix(z) = 2 (4^z − 1) / 3` counts the tiles of the coarser levels and
//! `m = interleave(x mod 2^z, y) | ((x >> z) << 2z)` is the Morton index within the level, the
//! hemisphere bit on top. The children of the tile with Morton `m` are `4m + c`, `c = (j << 1) | i`
//! for the child at column `2x + i`, row `2y + j`.

use core::f64::consts::PI;

use crate::angles::normalize_lon;
use crate::wgs84::A;

/// The finest level the format names (z13, about 19 m posts). // DATA: ADR-54
pub const MAX_LEVEL: u8 = 13;

/// The level of a cell: one `GeoPack` archive per tile of this level. // DATA: ADR-54
pub const CELL_LEVEL: u8 = 5;

/// Height posts per tile side. // DATA: GDD 44.3
pub const POSTS: usize = 129;

/// Intervals per tile side (`POSTS − 1`).
pub const INTERVALS: usize = 128;

/// Height posts per tile (129 x 129).
pub const POST_COUNT: usize = POSTS * POSTS;

/// Landcover cells per tile side. // DATA: ADR-54
pub const LC_SIDE: usize = 64;

/// Landcover cells per tile (64 x 64).
pub const LC_COUNT: usize = LC_SIDE * LC_SIDE;

/// One tile of the quadtree.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct TileId {
    /// Level, 0 to [`MAX_LEVEL`].
    pub z: u8,
    /// Column, `0 .. 2^(z+1)`, from 180° W eastwards.
    pub x: u32,
    /// Row, `0 .. 2^z`, from the north pole southwards.
    pub y: u32,
}

/// A tile's edges, rad.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TileBounds {
    /// North edge.
    pub lat_north: f64,
    /// South edge.
    pub lat_south: f64,
    /// West edge, in [−π, π).
    pub lon_west: f64,
    /// East edge, in (−π, π].
    pub lon_east: f64,
}

impl TileBounds {
    /// The tile's centre (lat, lon), rad.
    #[must_use]
    pub fn center(&self) -> (f64, f64) {
        (
            f64::midpoint(self.lat_north, self.lat_south),
            f64::midpoint(self.lon_west, self.lon_east),
        )
    }
}

/// The tiles of all levels coarser than `z`: `2 (4^z − 1) / 3`.
#[must_use]
pub const fn level_prefix(z: u8) -> u64 {
    2 * ((1u64 << (2 * z as u32)) - 1) / 3
}

/// The tile edge length at level `z`, `π / 2^z`, rad.
#[must_use]
pub fn delta_rad(z: u8) -> f64 {
    PI / f64::from(1u32 << z)
}

/// The post spacing at level `z` along the equator, `a Δ / 128`, m.
#[must_use]
pub fn post_spacing_equator_m(z: u8) -> f64 {
    A * delta_rad(z) / INTERVALS as f64
}

/// Bit `k` of `x` to bit `2k`, bit `k` of `y` to bit `2k + 1`, for `z` bits each.
const fn interleave(x: u32, y: u32, z: u8) -> u64 {
    let mut m = 0u64;
    let mut k = 0;
    while k < z as u32 {
        m |= (((x >> k) & 1) as u64) << (2 * k);
        m |= (((y >> k) & 1) as u64) << (2 * k + 1);
        k += 1;
    }
    m
}

/// The inverse of [`interleave`].
const fn deinterleave(m: u64, z: u8) -> (u32, u32) {
    let (mut x, mut y) = (0u32, 0u32);
    let mut k = 0;
    while k < z as u32 {
        x |= (((m >> (2 * k)) & 1) as u32) << k;
        y |= (((m >> (2 * k + 1)) & 1) as u32) << k;
        k += 1;
    }
    (x, y)
}

impl TileId {
    /// A tile from its parts; `None` if out of range.
    #[must_use]
    pub const fn new(z: u8, x: u32, y: u32) -> Option<Self> {
        if z > MAX_LEVEL || x >= (2u32 << z) || y >= (1u32 << z) {
            return None;
        }
        Some(Self { z, x, y })
    }

    /// The tile's id (module docs).
    #[must_use]
    pub const fn id(self) -> u64 {
        let side = 1u32 << self.z;
        let m = interleave(self.x % side, self.y, self.z)
            | (((self.x >> self.z) as u64) << (2 * self.z as u32));
        level_prefix(self.z) + m
    }

    /// The tile with this id; `None` if the id is past level [`MAX_LEVEL`].
    #[must_use]
    pub const fn from_id(id: u64) -> Option<Self> {
        if id >= level_prefix(MAX_LEVEL + 1) {
            return None;
        }
        let mut z = 0u8;
        while level_prefix(z + 1) <= id {
            z += 1;
        }
        let m = id - level_prefix(z);
        let bits = 2 * z as u32;
        let hemi = (m >> bits) as u32;
        let (xl, y) = deinterleave(m & ((1u64 << bits) - 1), z);
        Some(Self {
            z,
            x: xl + (hemi << z),
            y,
        })
    }

    /// The parent (one level coarser); `None` at level 0.
    #[must_use]
    pub const fn parent(self) -> Option<Self> {
        if self.z == 0 {
            return None;
        }
        Some(Self {
            z: self.z - 1,
            x: self.x >> 1,
            y: self.y >> 1,
        })
    }

    /// The four children in child-index order `c = (j << 1) | i` (NW, NE, SW, SE). Calling it at
    /// [`MAX_LEVEL`] gives tiles one level past the format's last.
    #[must_use]
    pub const fn children(self) -> [Self; 4] {
        let (z, x, y) = (self.z + 1, 2 * self.x, 2 * self.y);
        [
            Self { z, x, y },
            Self { z, x: x + 1, y },
            Self { z, x, y: y + 1 },
            Self {
                z,
                x: x + 1,
                y: y + 1,
            },
        ]
    }

    /// The ancestor at level `z` (`self` if `z == self.z`); `None` if `z > self.z`.
    #[must_use]
    pub const fn ancestor(self, z: u8) -> Option<Self> {
        if z > self.z {
            return None;
        }
        let s = (self.z - z) as u32;
        Some(Self {
            z,
            x: self.x >> s,
            y: self.y >> s,
        })
    }

    /// The cell (the level-5 ancestor's column and row) holding this tile; `None` below level 5.
    #[must_use]
    pub const fn cell(self) -> Option<(u32, u32)> {
        match self.ancestor(CELL_LEVEL) {
            Some(t) => Some((t.x, t.y)),
            None => None,
        }
    }

    /// The tile's edges.
    #[must_use]
    pub fn bounds(self) -> TileBounds {
        let n = f64::from(1u32 << self.z);
        TileBounds {
            lat_north: PI * (0.5 - f64::from(self.y) / n),
            lat_south: PI * (0.5 - f64::from(self.y + 1) / n),
            lon_west: PI * (f64::from(self.x) / n - 1.0),
            lon_east: PI * (f64::from(self.x + 1) / n - 1.0),
        }
    }

    /// The (lat, lon) of post (`row`, `col`), rad; row 0 on the north edge, col 0 on the west edge.
    /// The longitude is not normalised (the west edge of column 0 is −π).
    #[must_use]
    pub fn post_latlon(self, row: usize, col: usize) -> (f64, f64) {
        let n = f64::from(1u32 << self.z) * INTERVALS as f64;
        let r = f64::from(self.y) * INTERVALS as f64 + row as f64;
        let c = f64::from(self.x) * INTERVALS as f64 + col as f64;
        (PI * (0.5 - r / n), PI * (c / n - 1.0))
    }
}

/// The tile of level `z` holding (`lat`, `lon`). The column wraps modulo `2^(z+1)` (so 180° E is
/// column 0) and the row clamps to `[0, 2^z − 1]` (so the south pole is in the last row).
#[must_use]
pub fn tile_for(lat_rad: f64, lon_rad: f64, z: u8) -> TileId {
    let n = f64::from(1u32 << z);
    let cols = 2u32 << z;
    let u = (normalize_lon(lon_rad) / PI + 1.0) * n;
    let v = (0.5 - lat_rad / PI) * n;
    let x = (u.floor().max(0.0) as u32) % cols;
    let y = (v.floor().max(0.0) as u32).min((1u32 << z) - 1);
    TileId { z, x, y }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(z: u8, x: u32, y: u32) -> TileId {
        TileId::new(z, x, y).unwrap()
    }

    #[test]
    fn the_plan_vectors() {
        assert_eq!(level_prefix(0), 0);
        assert_eq!(level_prefix(3), 42);
        assert_eq!(level_prefix(5), 682);
        assert_eq!(level_prefix(6), 2_730);
        assert_eq!(level_prefix(10), 699_050);
        assert_eq!(level_prefix(12), 11_184_810);
        assert_eq!(t(5, 33, 7).id(), 1_749);
        let ids: Vec<u64> = t(5, 33, 7).children().iter().map(|c| c.id()).collect();
        assert_eq!(ids, [6_998, 6_999, 7_000, 7_001]);
        assert_eq!(
            t(5, 33, 7).children(),
            [t(6, 66, 14), t(6, 67, 14), t(6, 66, 15), t(6, 67, 15)]
        );
        assert_eq!(t(12, 4_321, 1_234).id(), 30_122_163);
        assert_eq!(t(13, 16_383, 8_191).id(), 178_956_969);
        assert_eq!(TileId::from_id(178_956_970), None);
    }

    #[test]
    fn ids_round_trip_and_children_are_4m_plus_c() {
        for z in 0..MAX_LEVEL {
            for &(x, y) in &[(0, 0), ((2u32 << z) - 1, (1u32 << z) - 1), (1u32 << z, 0)] {
                let tile = t(z, x, y);
                assert_eq!(TileId::from_id(tile.id()), Some(tile));
                let m = tile.id() - level_prefix(z);
                for (c, child) in tile.children().iter().enumerate() {
                    assert_eq!(child.id() - level_prefix(z + 1), 4 * m + c as u64);
                    assert_eq!(child.parent(), Some(tile));
                }
            }
        }
    }

    #[test]
    fn tile_for_wraps_and_clamps() {
        // 180° E is the west edge of column 0.
        assert_eq!(tile_for(0.1, PI, 4).x, 0);
        assert_eq!(tile_for(0.1, -PI, 4).x, 0);
        assert_eq!(tile_for(0.1, PI - 1e-9, 4).x, 31);
        assert_eq!(tile_for(PI / 2.0, 0.0, 4).y, 0);
        assert_eq!(tile_for(-PI / 2.0, 0.0, 4).y, 15);
        // The Alps cell.
        let c = tile_for(46.55 * PI / 180.0, 8.0 * PI / 180.0, 5);
        assert_eq!((c.x, c.y), (33, 7));
        assert_eq!(c.cell(), Some((33, 7)));
        assert_eq!(
            tile_for(46.55 * PI / 180.0, 8.0 * PI / 180.0, 12).cell(),
            Some((33, 7))
        );
        assert_eq!(t(4, 3, 3).cell(), None);
    }

    #[test]
    fn bounds_and_posts_agree() {
        let tile = t(5, 33, 7);
        let b = tile.bounds();
        assert!((b.lat_north - 50.625 * PI / 180.0).abs() < 1e-15);
        assert!((b.lat_south - 45.0 * PI / 180.0).abs() < 1e-15);
        assert!((b.lon_west - 5.625 * PI / 180.0).abs() < 1e-15);
        assert!((b.lon_east - 11.25 * PI / 180.0).abs() < 1e-15);
        assert_eq!(tile.post_latlon(0, 0), (b.lat_north, b.lon_west));
        assert_eq!(tile.post_latlon(128, 128), (b.lat_south, b.lon_east));
        assert!((post_spacing_equator_m(12) - 38.218).abs() < 0.001);
        assert!((post_spacing_equator_m(10) - 152.874).abs() < 0.001);
    }

    #[test]
    fn invalid_tiles_are_refused() {
        assert_eq!(TileId::new(14, 0, 0), None);
        assert_eq!(TileId::new(3, 16, 0), None);
        assert_eq!(TileId::new(3, 0, 8), None);
        assert_eq!(t(7, 5, 6).ancestor(8), None);
    }
}
