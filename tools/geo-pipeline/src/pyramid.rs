//! One cell's tile pyramid (ADR-54), for any height source.
//!
//! 1. **The finest level** (`4^(L − 5)` tiles):
//!    - Each post is sampled from the source.
//!    - Each landcover cell is 0 (ocean) where the source values around its centre are exactly 0.0,
//!      else 255.
//!    - The bounds are `bounds_f32` of the posts' range, and the posts are quantised.
//!    - `ε = 0`.
//! 2. **Each coarser level, down to `min_level`, by decimation:**
//!    - A parent post equals the child post at the same place, so refinement never moves a shared
//!      post.
//!    - A parent landcover cell is land if any of the four child cells under it is.
//!    - `ε(parent) = max over children (ε(child) + max |child post − parent surface|)`, where the
//!      parent surface is bilinear between the parent's stored posts. This is an upper bound on
//!      the parent's error against the finest level.
//! 3. **Ocean** (every post exactly 0.0 and every landcover cell 0) is not stored.
//!
//! The work is split across threads by contiguous id ranges (Morton blocks). Results are kept in
//! id order, so the output never depends on thread timing.

use geo::codec::{bounds_f32, dequantise, quantise};
use geo::gpk::{LC_LAND, LC_OCEAN, QUALITY_DERIVED};
use geo::tile::{
    INTERVALS, LC_COUNT, LC_SIDE, POST_COUNT, POSTS, TileBounds, TileId, level_prefix,
};
use geo_pack::DecodedTile;
use geo_pack::tile::{zero_landcover, zero_posts};

/// One sample of a height source.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sample {
    /// Height, m.
    pub h: f64,
    /// Whether every source value used is exactly 0.0 (ocean).
    pub zero: bool,
}

/// Where heights come from.
pub trait HeightSource: Sync {
    /// The height at (`lat`, `lon`), rad (the longitude may be unnormalised).
    fn sample(&self, lat: f64, lon: f64) -> Sample;
    /// Whether a tile with these bounds may hold land; `false` means every sample is 0 m and ocean.
    fn may_hold_land(&self, b: &TileBounds) -> bool;
    /// The quality of the finest level (`geo::gpk::QUALITY_*`).
    fn quality(&self) -> u8;
}

/// One level of a cell: the tiles in Morton order within the cell, `None` for ocean.
pub type Level = Vec<Option<DecodedTile>>;

/// The first tile id of level `z` inside the cell `root`.
#[must_use]
pub fn first_id(root: TileId, z: u8) -> u64 {
    let m = root.id() - level_prefix(root.z);
    level_prefix(z) + (m << (2 * u32::from(z - root.z)))
}

/// Runs `f(k)` for `k` in `0..n` over `threads` contiguous ranges; the results in order.
///
/// # Panics
/// If a worker panics.
pub fn par_map<T: Send>(n: usize, threads: usize, f: impl Fn(usize) -> T + Sync) -> Vec<T> {
    let threads = threads.clamp(1, n.max(1));
    let chunk = n.div_ceil(threads);
    std::thread::scope(|s| {
        let handles: Vec<_> = (0..threads)
            .map(|t| {
                let f = &f;
                s.spawn(move || {
                    (t * chunk..((t + 1) * chunk).min(n))
                        .map(f)
                        .collect::<Vec<T>>()
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|h| h.join().expect("a pipeline worker panicked"))
            .collect()
    })
}

/// A tile from f64 posts and landcover: quantised, or `None` if it is ocean.
#[must_use]
pub fn finish_tile(
    id: TileId,
    heights: &[f64],
    landcover: Box<[u8; LC_COUNT]>,
    eps: f32,
    quality: u8,
) -> Option<DecodedTile> {
    if heights.iter().all(|&h| h == 0.0) && landcover.iter().all(|&c| c == LC_OCEAN) {
        return None;
    }
    let (lo, hi) = heights
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), &h| {
            (a.min(h), b.max(h))
        });
    let (h_min, h_max) = bounds_f32(lo, hi);
    let mut posts = zero_posts();
    for (p, &h) in posts.iter_mut().zip(heights) {
        *p = quantise(h, h_min, h_max);
    }
    Some(DecodedTile {
        id,
        h_min,
        h_max,
        eps,
        quality,
        posts,
        landcover,
    })
}

/// The finest level's tile `id` from the source.
#[must_use]
pub fn source_tile(src: &dyn HeightSource, id: TileId) -> Option<DecodedTile> {
    if !src.may_hold_land(&id.bounds()) {
        return None;
    }
    let mut heights = vec![0.0; POST_COUNT];
    for r in 0..POSTS {
        for c in 0..POSTS {
            let (lat, lon) = id.post_latlon(r, c);
            heights[r * POSTS + c] = src.sample(lat, lon).h;
        }
    }
    let mut lc = zero_landcover();
    let per = INTERVALS / LC_SIDE; // 2 posts per landcover cell
    for a in 0..LC_SIDE {
        for b in 0..LC_SIDE {
            let (lat, lon) = id.post_latlon(per * a + 1, per * b + 1);
            lc[a * LC_SIDE + b] = if src.sample(lat, lon).zero {
                LC_OCEAN
            } else {
                LC_LAND
            };
        }
    }
    finish_tile(id, &heights, lc, 0.0, src.quality())
}

/// The child (index into the 2 x 2 block) and its post for parent post `k` along one axis.
fn child_post(k: usize) -> (usize, usize) {
    if 2 * k < INTERVALS {
        (0, 2 * k)
    } else {
        (1, 2 * k - INTERVALS)
    }
}

fn height_of(t: Option<&DecodedTile>, k: usize) -> f64 {
    t.map_or(0.0, |t| t.height(k))
}

/// The parent of four children (in child-index order), by decimation; `None` if ocean.
#[must_use]
pub fn decimate(id: TileId, children: [Option<&DecodedTile>; 4]) -> Option<DecodedTile> {
    if children.iter().all(Option::is_none) {
        return None;
    }
    let mut heights = vec![0.0; POST_COUNT];
    for r in 0..POSTS {
        let (j, cr) = child_post(r);
        for c in 0..POSTS {
            let (i, cc) = child_post(c);
            heights[r * POSTS + c] = height_of(children[(j << 1) | i], cr * POSTS + cc);
        }
    }
    let mut lc = zero_landcover();
    let half = LC_SIDE / 2;
    for a in 0..LC_SIDE {
        for b in 0..LC_SIDE {
            let (j, i) = (a / half, b / half);
            let land = children[(j << 1) | i].is_some_and(|t| {
                let (ca, cb) = (2 * (a - half * j), 2 * (b - half * i));
                [(ca, cb), (ca, cb + 1), (ca + 1, cb), (ca + 1, cb + 1)]
                    .iter()
                    .any(|&(x, y)| t.landcover[x * LC_SIDE + y] != LC_OCEAN)
            });
            lc[a * LC_SIDE + b] = if land { LC_LAND } else { LC_OCEAN };
        }
    }
    let mut parent = finish_tile(id, &heights, lc, 0.0, QUALITY_DERIVED)?;
    parent.eps = parent_eps(&parent, children);
    Some(parent)
}

/// The bilinear height of `t` at fractional post (`u` column, `v` row).
fn bilinear(t: &DecodedTile, u: f64, v: f64) -> f64 {
    let (i, j) = (
        (u.floor() as usize).min(INTERVALS - 1),
        (v.floor() as usize).min(INTERVALS - 1),
    );
    let (fu, fv) = (u - i as f64, v - j as f64);
    let h = |r: usize, c: usize| dequantise(f64::from(t.posts[r * POSTS + c]), t.h_min, t.h_max);
    let top = h(j, i) + (h(j, i + 1) - h(j, i)) * fu;
    let bot = h(j + 1, i) + (h(j + 1, i + 1) - h(j + 1, i)) * fu;
    top + (bot - top) * fv
}

fn parent_eps(parent: &DecodedTile, children: [Option<&DecodedTile>; 4]) -> f32 {
    let mut worst = 0.0f64;
    for (ci, child) in children.iter().enumerate() {
        let (i, j) = ((ci & 1) as f64, (ci >> 1) as f64);
        let mut e = 0.0f64;
        for r in 0..POSTS {
            for c in 0..POSTS {
                let surf = bilinear(
                    parent,
                    f64::midpoint(INTERVALS as f64 * i, c as f64),
                    f64::midpoint(INTERVALS as f64 * j, r as f64),
                );
                e = e.max((height_of(*child, r * POSTS + c) - surf).abs());
            }
        }
        worst = worst.max(e + child.map_or(0.0, |t| f64::from(t.eps)));
    }
    worst as f32
}

/// The cell's pyramid: `levels[k]` is level `min_level + k`.
///
/// # Panics
/// If a level holds an id past z13 (it cannot: the levels are at most `max_level`).
#[must_use]
pub fn build(
    root: TileId,
    min_level: u8,
    max_level: u8,
    src: &dyn HeightSource,
    threads: usize,
) -> Vec<Level> {
    let n_at = |z: u8| 1usize << (2 * u32::from(z - root.z));
    let first = first_id(root, max_level);
    let finest: Level = par_map(n_at(max_level), threads, |k| {
        let id = TileId::from_id(first + k as u64).expect("a tile id inside the cell");
        source_tile(src, id)
    });
    let mut levels = vec![finest];
    for z in (min_level..max_level).rev() {
        let below = levels.last().expect("the finer level");
        let f = first_id(root, z);
        let level: Level = par_map(n_at(z), threads, |k| {
            let id = TileId::from_id(f + k as u64).expect("a tile id inside the cell");
            let kids = [
                below[4 * k].as_ref(),
                below[4 * k + 1].as_ref(),
                below[4 * k + 2].as_ref(),
                below[4 * k + 3].as_ref(),
            ];
            decimate(id, kids)
        });
        levels.push(level);
    }
    levels.reverse();
    levels
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Ramp;
    impl HeightSource for Ramp {
        fn sample(&self, lat: f64, lon: f64) -> Sample {
            let h = 1_000.0 + 50_000.0 * (lat - 0.8) + 20_000.0 * (lon - 0.1);
            Sample { h, zero: false }
        }
        fn may_hold_land(&self, _: &TileBounds) -> bool {
            true
        }
        fn quality(&self) -> u8 {
            2
        }
    }

    #[test]
    fn decimation_keeps_shared_posts_and_a_plane_has_tiny_eps() {
        let root = TileId::new(5, 33, 7).unwrap();
        let levels = build(TileId::new(7, 133, 29).unwrap(), 7, 9, &Ramp, 4);
        assert_eq!(levels.len(), 3);
        assert_eq!(levels[2].len(), 16);
        let parent = levels[1][0].as_ref().unwrap();
        let child = levels[2][0].as_ref().unwrap();
        // Parent post (0, 0) is child 0's post (0, 0); parent post (10, 20) is child 0's (20, 40).
        assert!((parent.height(0) - child.height(0)).abs() < 0.05);
        assert!((parent.height(10 * POSTS + 20) - child.height(20 * POSTS + 40)).abs() < 0.05);
        // A plane is bilinear: ε is the quantisation noise, centimetres.
        assert!(parent.eps < 0.1, "{}", parent.eps);
        assert_eq!(parent.quality, QUALITY_DERIVED);
        assert_eq!(child.quality, 2);
        assert_eq!(child.eps, 0.0);
        assert_eq!(first_id(root, 6), TileId::new(6, 66, 14).unwrap().id());
    }

    #[test]
    fn ocean_is_not_stored_and_a_coast_is_land() {
        struct Coast;
        impl HeightSource for Coast {
            fn sample(&self, _lat: f64, lon: f64) -> Sample {
                if lon < 0.125 {
                    Sample { h: 0.0, zero: true }
                } else {
                    Sample {
                        h: 10.0,
                        zero: false,
                    }
                }
            }
            fn may_hold_land(&self, b: &TileBounds) -> bool {
                b.lon_east >= 0.125
            }
            fn quality(&self) -> u8 {
                0
            }
        }
        let levels = build(TileId::new(6, 66, 14).unwrap(), 6, 7, &Coast, 2);
        let stored = levels[1].iter().filter(|t| t.is_some()).count();
        assert!(stored > 0 && stored < 4, "{stored}");
        let p = levels[0][0].as_ref().unwrap();
        assert!(p.landcover.contains(&LC_OCEAN) && p.landcover.contains(&LC_LAND));
        // ε of a step is about the step.
        assert!(p.eps > 1.0 && p.eps <= 10.1, "{}", p.eps);
    }
}
