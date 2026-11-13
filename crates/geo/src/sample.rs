//! The bicubic (Catmull–Rom) height sampler over a tile's posts (GDD 44.6, ADR-52).
//!
//! Post coordinates: `u` = column (0 at the west edge, 128 at the east), `v` = row (0 at the north
//! edge, 128 at the south). Per axis the Catmull–Rom cubic through `P0..P3`, with `P1` at the
//! integer below the sample and `t` the fraction, is
//! `p(t) = ½ [2 P1 + (P2 − P0) t + (2 P0 − 5 P1 + 4 P2 − P3) t² + (3 P1 − P0 − 3 P2 + P3) t³]`.
//! Neighbours beyond the tile are clamped to the edge post, so two tiles that share an edge give the
//! same height along it.

use crate::angles::normalize_lon;
use crate::codec::dequantise;
use crate::tile::{INTERVALS, POSTS, TileBounds, TileId};

/// One tile's quantised posts, ready to sample.
#[derive(Clone, Copy, Debug)]
pub struct TilePosts<'a> {
    /// 129 x 129 codes, row 0 north, **not** delta-coded.
    pub posts: &'a [u16],
    /// The tile's quantisation minimum, m.
    pub h_min: f32,
    /// The tile's quantisation maximum, m.
    pub h_max: f32,
    /// The tile's edges.
    pub bounds: TileBounds,
    /// The post spacing, rad.
    pub step_rad: f64,
}

impl<'a> TilePosts<'a> {
    /// The posts of `tile`. `posts` must hold 16 641 codes.
    #[must_use]
    pub fn new(tile: TileId, posts: &'a [u16], h_min: f32, h_max: f32) -> Self {
        debug_assert_eq!(posts.len(), POSTS * POSTS);
        let bounds = tile.bounds();
        Self {
            posts,
            h_min,
            h_max,
            bounds,
            step_rad: (bounds.lat_north - bounds.lat_south) / INTERVALS as f64,
        }
    }

    /// The fractional post coordinates (u, v) of (`lat`, `lon`), clamped to the tile.
    #[must_use]
    pub fn uv(&self, lat_rad: f64, lon_rad: f64) -> (f64, f64) {
        let max = INTERVALS as f64;
        let u = normalize_lon(lon_rad - self.bounds.lon_west) / self.step_rad;
        let v = (self.bounds.lat_north - lat_rad) / self.step_rad;
        (u.clamp(0.0, max), v.clamp(0.0, max))
    }

    fn code(&self, row: usize, col: usize) -> f64 {
        f64::from(self.posts[row * POSTS + col])
    }

    /// The bicubic height at (`lat`, `lon`), m.
    #[must_use]
    pub fn bicubic(&self, lat_rad: f64, lon_rad: f64) -> f64 {
        let (u, v) = self.uv(lat_rad, lon_rad);
        let (q, _, _) = catmull_rom_2d(|r, c| self.code(r, c), u, v);
        dequantise(q, self.h_min, self.h_max)
    }

    /// The bicubic height and its gradient per radian of latitude and of longitude:
    /// `(h, ∂h/∂φ, ∂h/∂λ)`.
    #[must_use]
    pub fn bicubic_grad(&self, lat_rad: f64, lon_rad: f64) -> (f64, f64, f64) {
        let (u, v) = self.uv(lat_rad, lon_rad);
        let (q, dq_du, dq_dv) = catmull_rom_2d(|r, c| self.code(r, c), u, v);
        let scale = (f64::from(self.h_max) - f64::from(self.h_min)) / crate::codec::Q_MAX;
        (
            dequantise(q, self.h_min, self.h_max),
            -dq_dv * scale / self.step_rad, // v grows southwards
            dq_du * scale / self.step_rad,
        )
    }
}

/// The Catmull–Rom cubic and its derivative at `t` through `p` (see the module docs).
fn cubic(p: [f64; 4], t: f64) -> (f64, f64) {
    let b = p[2] - p[0];
    let c = 2.0 * p[0] - 5.0 * p[1] + 4.0 * p[2] - p[3];
    let d = 3.0 * p[1] - p[0] - 3.0 * p[2] + p[3];
    (
        p[1] + 0.5 * t * (b + t * (c + t * d)),
        0.5 * b + t * (c + 1.5 * t * d),
    )
}

/// The integer below `x` in `[0, 127]` and the fraction.
fn split(x: f64) -> (usize, f64) {
    let i = (x.floor() as usize).min(INTERVALS - 1);
    (i, x - i as f64)
}

/// The tensor-product Catmull–Rom of the grid `get(row, col)` (rows and columns `0..=128`, clamped)
/// at (`u`, `v`): `(value, ∂/∂u, ∂/∂v)`.
pub fn catmull_rom_2d(get: impl Fn(usize, usize) -> f64, u: f64, v: f64) -> (f64, f64, f64) {
    let (i, tu) = split(u);
    let (j, tv) = split(v);
    let clamp = |k: isize| k.clamp(0, INTERVALS.cast_signed()).cast_unsigned();
    let mut rows = [0.0; 4];
    let mut drows = [0.0; 4];
    for (n, dr) in (-1isize..=2).enumerate() {
        let r = clamp(j.cast_signed() + dr);
        let p = [-1isize, 0, 1, 2].map(|dc| get(r, clamp(i.cast_signed() + dc)));
        (rows[n], drows[n]) = cubic(p, tu);
    }
    let (h, dh_dv) = cubic(rows, tv);
    let (dh_du, _) = cubic(drows, tv);
    (h, dh_du, dh_dv)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quadratic(u: f64, v: f64) -> f64 {
        3.0 + 0.5 * u - 0.25 * v + 0.01 * u * u + 0.02 * u * v - 0.015 * v * v
    }

    #[test]
    fn it_reproduces_a_quadratic_on_interior_posts() {
        for &(u, v) in &[(1.25, 1.5), (60.3, 77.7), (126.9, 2.1), (64.0, 64.0)] {
            let (h, du, dv) = catmull_rom_2d(|r, c| quadratic(c as f64, r as f64), u, v);
            assert!((h - quadratic(u, v)).abs() < 1e-9, "{u} {v}: {h}");
            assert!((du - (0.5 + 0.02 * u + 0.02 * v)).abs() < 1e-9);
            assert!((dv - (-0.25 + 0.02 * u - 0.03 * v)).abs() < 1e-9);
        }
    }

    #[test]
    fn it_equals_the_posts_at_the_posts() {
        let grid = |r: usize, c: usize| ((r * 31 + c * 17) % 101) as f64;
        for &(r, c) in &[(0, 0), (5, 9), (128, 128), (128, 0), (64, 127)] {
            let (h, _, _) = catmull_rom_2d(grid, c as f64, r as f64);
            assert_eq!(h, grid(r, c));
        }
    }

    #[test]
    fn two_tiles_agree_along_their_shared_edge() {
        // One height function sampled by two neighbouring z12 tiles (west and east).
        let west = TileId::new(12, 4_321, 1_234).unwrap();
        let east = TileId::new(12, 4_322, 1_234).unwrap();
        let f = |lat: f64, lon: f64| {
            let q = 30_000.0
                + 20_000.0 * simmath::dmath::sin(lat * 900.0) * simmath::dmath::cos(lon * 700.0);
            q.round() as u16
        };
        let build = |t: TileId| {
            let mut p = vec![0u16; POSTS * POSTS];
            for r in 0..POSTS {
                for c in 0..POSTS {
                    let (lat, lon) = t.post_latlon(r, c);
                    p[r * POSTS + c] = f(lat, lon);
                }
            }
            p
        };
        let (pw, pe) = (build(west), build(east));
        let tw = TilePosts::new(west, &pw, 0.0, 6_553.5);
        let te = TilePosts::new(east, &pe, 0.0, 6_553.5);
        let edge = tw.bounds.lon_east;
        for k in 0..50 {
            let lat = tw.bounds.lat_south
                + (tw.bounds.lat_north - tw.bounds.lat_south) * (f64::from(k) + 0.37) / 50.0;
            let a = tw.bicubic(lat, edge);
            let b = te.bicubic(lat, edge);
            assert!((a - b).abs() < 1e-9, "{a} {b}");
        }
    }

    #[test]
    fn the_gradient_is_per_radian() {
        let tile = TileId::new(10, 1_000, 300).unwrap();
        // Codes rising by one per column eastwards: ∂h/∂λ = scale / step.
        let mut p = vec![0u16; POSTS * POSTS];
        for r in 0..POSTS {
            for c in 0..POSTS {
                p[r * POSTS + c] = (c * 100) as u16;
            }
        }
        let t = TilePosts::new(tile, &p, 0.0, 65_535.0);
        let (lat, lon) = tile.post_latlon(40, 40);
        let (_, dlat, dlon) = t.bicubic_grad(lat, lon + 0.3 * t.step_rad);
        assert!(dlat.abs() < 1e-6);
        assert!(
            (dlon - 100.0 / t.step_rad).abs() < 1e-3 * dlon.abs(),
            "{dlon}"
        );
    }
}
