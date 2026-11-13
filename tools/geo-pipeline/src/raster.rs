//! The source rasters of one cell and the bilinear sampler across 1° tiles (ADR-54).
//!
//! **Loading.** All of a cell's 1° tiles are decoded once, in parallel, before any sampling, and kept
//! for the whole cell. (The v0.2a plan named a 12-entry LRU. Eight workers on separate Morton blocks
//! need more than 12 tiles at once, so a cell holds them all instead: at most 49 tiles, 2.5 GB for
//! GLO-30 and 0.3 GB for GLO-90.)
//!
//! **Choosing a product.** A region cell takes each tile from GLO-30 if the GLO-30 list has it,
//! else from GLO-90; a world cell takes GLO-90 only. A tile in neither list is ocean (0 m).
//!
//! **Sampling** (`PixelIsPoint`, degrees):
//! 1. The tile `T = ceil(lat) − 1` and `floor(lon)` gives the north row `j0 = floor((lat0 − lat) / sy)`.
//! 2. The south row is `j0 + 1`, or row 0 of the tile below when `j0` is the last row.
//! 3. Each row is interpolated linearly along its own grid, its last column against column 0 of the
//!    tile to the east.
//! 4. The two rows are interpolated by the fraction.
//!
//! Each tile keeps its own grid, so GLO-30 and GLO-90 tiles meet without a gap.

use std::collections::BTreeMap;
use std::path::Path;

use geo::angles::{DEG, normalize_lon};
use geo::tile::TileBounds;

use crate::pyramid::{HeightSource, Sample};
use crate::sources::{Product, TileList, tile_name, wrap_lon};
use crate::tiff::{Raster, decode};
use crate::{Failure, read, sha256_hex};

/// One loaded source file.
#[derive(Clone, Debug)]
pub struct Loaded {
    /// The product.
    pub product: Product,
    /// The file name (`<name>.tif`).
    pub file: String,
    /// Its SHA-256.
    pub sha256: String,
    /// Its size, bytes.
    pub bytes: u64,
    /// The decoded raster.
    pub raster: Raster,
}

/// A cell's source tiles, keyed by (south latitude, west longitude) in degrees.
#[derive(Debug, Default)]
pub struct SourceStore {
    tiles: BTreeMap<(i32, i32), Loaded>,
}

/// The nominal rows per degree of a tile with no data (ocean). // ALGO: only weights rows next to it
const OCEAN_ROWS: f64 = 1_200.0;

impl SourceStore {
    /// Decodes the listed tiles among `wanted` from `cache` (files `<name>.tif`), with the product
    /// preference `prefs` (first match wins).
    ///
    /// # Errors
    /// A listed file missing from the cache, or one that does not decode.
    pub fn load(
        wanted: &[(i32, i32)],
        prefs: &[(Product, &TileList)],
        cache: &Path,
        threads: usize,
    ) -> Result<Self, Failure> {
        let jobs: Vec<((i32, i32), Product, String)> = wanted
            .iter()
            .filter_map(|&(lat, lon)| {
                prefs
                    .iter()
                    .find(|(p, l)| l.has(*p, lat, lon))
                    .map(|(p, _)| ((lat, lon), *p, tile_name(*p, lat, lon)))
            })
            .collect();
        let results = crate::pyramid::par_map(jobs.len(), threads, |k| {
            let (key, product, name) = &jobs[k];
            let file = format!("{name}.tif");
            let bytes = read(&cache.join(&file))?;
            let raster = decode(&bytes).map_err(|e| Failure::Check(format!("{file}: {e}")))?;
            Ok::<_, Failure>((
                *key,
                Loaded {
                    product: *product,
                    file,
                    sha256: sha256_hex(&bytes),
                    bytes: bytes.len() as u64,
                    raster,
                },
            ))
        });
        let mut tiles = BTreeMap::new();
        for r in results {
            let (k, v) = r?;
            tiles.insert(k, v);
        }
        Ok(Self { tiles })
    }

    /// A store from rasters (tests).
    #[must_use]
    pub fn from_rasters(rasters: Vec<((i32, i32), Raster)>) -> Self {
        let tiles = rasters
            .into_iter()
            .map(|(k, raster)| {
                (
                    k,
                    Loaded {
                        product: Product::Glo90,
                        file: String::new(),
                        sha256: String::new(),
                        bytes: 0,
                        raster,
                    },
                )
            })
            .collect();
        Self { tiles }
    }

    /// The loaded files, by key.
    pub fn files(&self) -> impl Iterator<Item = &Loaded> {
        self.tiles.values()
    }

    /// Voids left in all loaded tiles.
    #[must_use]
    pub fn unfilled_voids(&self) -> u64 {
        self.tiles.values().map(|t| t.raster.unfilled_voids).sum()
    }

    fn tile(&self, lat: i32, lon: i32) -> Option<&Raster> {
        self.tiles.get(&(lat, wrap_lon(lon))).map(|t| &t.raster)
    }

    /// The value of column 0 of the tile (`lat_t`, `lon_t`) at latitude `lat` (linear between
    /// that column's rows); 0 for ocean.
    fn column0(&self, lat_t: i32, lon_t: i32, lat: f64, used: &mut bool) -> f64 {
        let Some(r) = self.tile(lat_t, lon_t) else {
            return 0.0;
        };
        let fy = ((r.lat0 - lat) / r.sy).clamp(0.0, (r.height - 1) as f64);
        let j = (fy.floor() as usize).min(r.height - 1);
        let t = fy - j as f64;
        let a = f64::from(r.at(0, j));
        let b = if j + 1 < r.height {
            f64::from(r.at(0, j + 1))
        } else {
            a
        };
        *used |= a != 0.0 || b != 0.0;
        a + (b - a) * t
    }

    /// Row `j` of tile (`lat_t`, `lon_t`) at longitude `lon` (degrees in [−180, 180)).
    fn row(&self, lat_t: i32, lon_t: i32, r: &Raster, j: usize, lon: f64, used: &mut bool) -> f64 {
        let fx = ((lon - r.lon0) / r.sx).max(0.0);
        let i = (fx.floor() as usize).min(r.width - 1);
        let s = fx - i as f64;
        let a = f64::from(r.at(i, j));
        let b = if i + 1 < r.width {
            f64::from(r.at(i + 1, j))
        } else {
            self.column0(lat_t, lon_t + 1, r.lat0 - j as f64 * r.sy, used)
        };
        *used |= a != 0.0 || b != 0.0;
        a + (b - a) * s.min(1.0)
    }

    /// The bilinear height at (`lat`, `lon`), degrees.
    #[must_use]
    pub fn sample_deg(&self, lat: f64, lon: f64) -> Sample {
        let lat_t = ((lat.ceil() as i32) - 1).clamp(-90, 89);
        let lon_t = lon.floor() as i32;
        let mut used = false;
        let (north, south, t) = if let Some(r) = self.tile(lat_t, lon_t) {
            let fy = ((r.lat0 - lat) / r.sy).clamp(0.0, r.height as f64);
            let j = (fy.floor() as usize).min(r.height - 1);
            let t = (fy - j as f64).min(1.0);
            let north = self.row(lat_t, lon_t, r, j, lon, &mut used);
            let south = if j + 1 < r.height {
                self.row(lat_t, lon_t, r, j + 1, lon, &mut used)
            } else {
                self.edge_row(lat_t - 1, lon_t, lon, &mut used)
            };
            (north, south, t)
        } else {
            // An ocean tile: 0 m, except in the gap above the tile below's row 0.
            let gap = (f64::from(lat_t) + 1.0 / OCEAN_ROWS - lat) * OCEAN_ROWS;
            if gap > 0.0 {
                (
                    0.0,
                    self.edge_row(lat_t - 1, lon_t, lon, &mut used),
                    gap.min(1.0),
                )
            } else {
                (0.0, 0.0, 0.0)
            }
        };
        Sample {
            h: north + (south - north) * t,
            zero: !used,
        }
    }

    /// Row 0 of tile (`lat_t`, `lon_t`) at `lon`; 0 for ocean.
    fn edge_row(&self, lat_t: i32, lon_t: i32, lon: f64, used: &mut bool) -> f64 {
        match self.tile(lat_t, lon_t) {
            Some(r) => self.row(lat_t, lon_t, r, 0, lon, used),
            None => 0.0,
        }
    }
}

/// A [`SourceStore`] with the quality of its products.
#[derive(Debug)]
pub struct Mosaic {
    /// The tiles.
    pub store: SourceStore,
}

impl HeightSource for Mosaic {
    fn sample(&self, lat: f64, lon: f64) -> Sample {
        self.store.sample_deg(lat * DEG, normalize_lon(lon) * DEG)
    }

    fn may_hold_land(&self, b: &TileBounds) -> bool {
        let (s, n) = (b.lat_south * DEG, b.lat_north * DEG);
        let (w, e) = (b.lon_west * DEG, b.lon_east * DEG);
        let lat_range = (s.ceil() as i32 - 2)..=(n.ceil() as i32);
        lat_range.into_iter().any(|lat| {
            ((w.floor() as i32 - 1)..=(e.floor() as i32 + 1))
                .any(|lon| self.store.tile(lat, lon).is_some())
        })
    }

    fn quality(&self) -> u8 {
        geo::gpk::QUALITY_SOURCE
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 1° tile of `n` x `n` point samples with value `f(lat, lon)`.
    fn tile(lat: i32, lon: i32, n: usize, f: impl Fn(f64, f64) -> f32) -> ((i32, i32), Raster) {
        let s = 1.0 / n as f64;
        let (lat0, lon0) = (f64::from(lat) + 1.0, f64::from(lon));
        let mut data = Vec::with_capacity(n * n);
        for r in 0..n {
            for c in 0..n {
                data.push(f(lat0 - r as f64 * s, lon0 + c as f64 * s));
            }
        }
        (
            (lat, lon),
            Raster {
                width: n,
                height: n,
                lon0,
                lat0,
                sx: s,
                sy: s,
                data,
                unfilled_voids: 0,
            },
        )
    }

    #[test]
    fn a_plane_is_reproduced_across_tile_edges_and_products() {
        let plane = |lat: f64, lon: f64| (100.0 * lat + 10.0 * lon) as f32;
        // GLO-90-like and GLO-30-like neighbours.
        let st = SourceStore::from_rasters(vec![
            tile(46, 7, 12, plane),
            tile(46, 8, 36, plane),
            tile(45, 7, 12, plane),
            tile(45, 8, 12, plane),
        ]);
        for &(lat, lon) in &[
            (46.5, 7.5),
            (46.001, 7.97),
            (46.0, 7.999),
            (46.95, 7.96),
            (46.03, 8.4),
            (45.999, 7.2),
        ] {
            let s = st.sample_deg(lat, lon);
            let want = 100.0 * lat + 10.0 * lon;
            assert!(
                (s.h - want).abs() < 1e-2,
                "{lat} {lon}: {} against {want}",
                s.h
            );
            assert!(!s.zero);
        }
        // The ocean beyond.
        let s = st.sample_deg(48.5, 7.5);
        assert_eq!(s, Sample { h: 0.0, zero: true });
    }
}
