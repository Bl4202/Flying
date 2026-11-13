//! Copernicus DEM sources (ADR-54): tile lists, file names, URLs, the 1° tiles a cell needs and the
//! cells with land.
//!
//! A tile is named for its south-west corner, e.g. `Copernicus_DSM_COG_30_N46_00_E007_00_DEM` for
//! latitudes 46° to 47° N and longitudes 7° to 8° E (GLO-90; GLO-30 is `COG_10`). The rasters are
//! `PixelIsPoint`: pixel (0, 0) is centred on the north-west corner (47° N, 7° E), so the tile holds
//! latitudes (46°, 47°] and longitudes [7°, 8°). Ocean has no tile: its height is 0.

use std::collections::BTreeSet;
use std::path::Path;

use crate::{Failure, read};
use geo::tile::{CELL_LEVEL, TileId};

/// A Copernicus DEM product.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Product {
    /// GLO-30, 1 arc-second.
    Glo30,
    /// GLO-90, 3 arc-seconds.
    Glo90,
}

impl Product {
    /// Parses `glo30` or `glo90`.
    ///
    /// # Errors
    /// [`Failure::Usage`].
    pub fn parse(s: &str) -> Result<Self, Failure> {
        match s {
            "glo30" => Ok(Self::Glo30),
            "glo90" => Ok(Self::Glo90),
            _ => Err(Failure::Usage(format!(
                "unknown product {s:?}; use glo30 or glo90"
            ))),
        }
    }

    /// The product's short name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Glo30 => "glo30",
            Self::Glo90 => "glo90",
        }
    }

    const fn code(self) -> &'static str {
        match self {
            Self::Glo30 => "10",
            Self::Glo90 => "30",
        }
    }

    const fn bucket(self) -> &'static str {
        match self {
            Self::Glo30 => "copernicus-dem-30m",
            Self::Glo90 => "copernicus-dem-90m",
        }
    }
}

/// The name of the 1° tile with south-west corner (`lat`, `lon`), degrees.
#[must_use]
pub fn tile_name(p: Product, lat: i32, lon: i32) -> String {
    let ns = if lat < 0 { 'S' } else { 'N' };
    let ew = if lon < 0 { 'W' } else { 'E' };
    format!(
        "Copernicus_DSM_COG_{}_{ns}{:02}_00_{ew}{:03}_00_DEM",
        p.code(),
        lat.unsigned_abs(),
        lon.unsigned_abs()
    )
}

/// The URL of a tile.
#[must_use]
pub fn tile_url(p: Product, name: &str) -> String {
    format!("https://{}.s3.amazonaws.com/{name}/{name}.tif", p.bucket())
}

/// A tile list (`tileList.txt`): one name per line.
#[derive(Clone, Debug, Default)]
pub struct TileList {
    names: BTreeSet<String>,
    /// The SHA-256 of the file.
    pub sha256: String,
}

impl TileList {
    /// Parses a list's text.
    #[must_use]
    pub fn parse(text: &str) -> Self {
        Self {
            names: text
                .lines()
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .map(String::from)
                .collect(),
            sha256: crate::sha256_hex(text.as_bytes()),
        }
    }

    /// Reads a list.
    ///
    /// # Errors
    /// [`Failure::Io`].
    pub fn read(path: &Path) -> Result<Self, Failure> {
        let bytes = read(path)?;
        Ok(Self::parse(&String::from_utf8_lossy(&bytes)))
    }

    /// Whether the list holds `name`.
    #[must_use]
    pub fn contains(&self, name: &str) -> bool {
        self.names.contains(name)
    }

    /// Whether the list holds the tile of `p` at (`lat`, `lon`).
    #[must_use]
    pub fn has(&self, p: Product, lat: i32, lon: i32) -> bool {
        self.contains(&tile_name(p, lat, lon))
    }

    /// The number of names.
    #[must_use]
    pub fn len(&self) -> usize {
        self.names.len()
    }

    /// Whether the list is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }
}

/// Parses `X,Y`.
///
/// # Errors
/// [`Failure::Usage`] unless 0 ≤ X < 64 and 0 ≤ Y < 32.
pub fn parse_cell(s: &str) -> Result<(u32, u32), Failure> {
    let bad = || Failure::Usage(format!("not a cell: {s:?} (want X,Y with X < 64, Y < 32)"));
    let (a, b) = s.split_once(',').ok_or_else(bad)?;
    let (x, y): (u32, u32) = (
        a.trim().parse().map_err(|_| bad())?,
        b.trim().parse().map_err(|_| bad())?,
    );
    if x < 64 && y < 32 {
        Ok((x, y))
    } else {
        Err(bad())
    }
}

/// A cell's bounds in degrees: (south, north, west, east).
///
/// # Panics
/// If (`x`, `y`) is not a cell (x < 64, y < 32).
#[must_use]
pub fn cell_bounds_deg(x: u32, y: u32) -> (f64, f64, f64, f64) {
    let t = TileId::new(CELL_LEVEL, x, y).expect("a cell");
    let b = t.bounds();
    let d = geo::angles::DEG;
    (
        b.lat_south * d,
        b.lat_north * d,
        b.lon_west * d,
        b.lon_east * d,
    )
}

/// A longitude index wrapped to [−180, 179].
#[must_use]
pub fn wrap_lon(lon: i32) -> i32 {
    (lon + 180).rem_euclid(360) - 180
}

/// The 1° tiles (south-west corners) whose pixels a cell's sampling may read: latitude rows from
/// the one holding the south edge (`ceil(s) − 1`: an edge at a multiple of 5.625° never falls in the
/// gap below a tile's last row) to the one holding the north edge, and longitude columns from the
/// west edge to the east edge.
#[must_use]
pub fn tiles_for_cell(x: u32, y: u32) -> Vec<(i32, i32)> {
    let (s, n, w, e) = cell_bounds_deg(x, y);
    let lat_lo = (s.ceil() as i32 - 1).max(-90);
    let lat_hi = (n.ceil() as i32 - 1).min(89);
    let (lon_lo, lon_hi) = (w.floor() as i32, e.floor() as i32);
    let mut out = Vec::new();
    for lat in lat_lo..=lat_hi {
        for lon in lon_lo..=lon_hi {
            let l = wrap_lon(lon);
            if !out.contains(&(lat, l)) {
                out.push((lat, l));
            }
        }
    }
    out
}

/// The URLs of a cell's tiles that the list holds.
#[must_use]
pub fn urls_for_cell(list: &TileList, p: Product, x: u32, y: u32) -> Vec<String> {
    tiles_for_cell(x, y)
        .into_iter()
        .map(|(lat, lon)| tile_name(p, lat, lon))
        .filter(|n| list.contains(n))
        .map(|n| tile_url(p, &n))
        .collect()
}

/// The cells named by the region files (`region/1`, field `cells`).
///
/// # Errors
/// [`Failure::Io`] for an unreadable file or directory; [`Failure::Check`] for a malformed one.
pub fn region_cells(dir: &Path) -> Result<BTreeSet<(u32, u32)>, Failure> {
    let mut out = BTreeSet::new();
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .map_err(|e| crate::io_err(dir, &e))?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    files.sort();
    for f in files {
        let v: serde_json::Value = serde_json::from_slice(&read(&f)?)
            .map_err(|e| Failure::Check(format!("{}: {e}", f.display())))?;
        for c in v["cells"].as_array().into_iter().flatten() {
            let (Some(x), Some(y)) = (c[0].as_u64(), c[1].as_u64()) else {
                return Err(Failure::Check(format!(
                    "{}: a cell is not [x, y]",
                    f.display()
                )));
            };
            out.insert((x as u32, y as u32));
        }
    }
    Ok(out)
}

/// The cells with land (a GLO-90 tile meets them), the region cells first (level 12), then the
/// others (level 10), each group sorted by (y, x).
#[must_use]
pub fn cells_with_land(list90: &TileList, regions: &BTreeSet<(u32, u32)>) -> Vec<(u32, u32, u8)> {
    let mut land = Vec::new();
    for y in 0..32 {
        for x in 0..64 {
            let (s, n, w, e) = cell_bounds_deg(x, y);
            let mut any = false;
            for lat in (s.floor() as i32)..(n.ceil() as i32) {
                for lon in (w.floor() as i32)..(e.ceil() as i32) {
                    any |= list90.has(Product::Glo90, lat.clamp(-90, 89), wrap_lon(lon));
                }
            }
            if any || regions.contains(&(x, y)) {
                land.push((x, y));
            }
        }
    }
    let level = |c: &(u32, u32)| if regions.contains(c) { 12 } else { 10 };
    let mut out: Vec<(u32, u32, u8)> = land
        .iter()
        .filter(|c| regions.contains(c))
        .map(|&(x, y)| (x, y, 12))
        .collect();
    out.extend(
        land.iter()
            .filter(|c| !regions.contains(c))
            .map(|c| (c.0, c.1, level(c))),
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_urls() {
        assert_eq!(
            tile_name(Product::Glo90, 46, 7),
            "Copernicus_DSM_COG_30_N46_00_E007_00_DEM"
        );
        assert_eq!(
            tile_name(Product::Glo30, -12, -123),
            "Copernicus_DSM_COG_10_S12_00_W123_00_DEM"
        );
        assert_eq!(
            tile_url(Product::Glo90, "Copernicus_DSM_COG_30_N46_00_E007_00_DEM"),
            "https://copernicus-dem-90m.s3.amazonaws.com/Copernicus_DSM_COG_30_N46_00_E007_00_DEM/Copernicus_DSM_COG_30_N46_00_E007_00_DEM.tif"
        );
        assert_eq!(parse_cell("33,7"), Ok((33, 7)));
        assert!(parse_cell("64,0").is_err());
        assert_eq!(wrap_lon(180), -180);
        assert_eq!(wrap_lon(-181), 179);
    }

    #[test]
    fn the_alps_cell_reads_7_by_7_tiles() {
        let t = tiles_for_cell(33, 7);
        // Latitude rows N44 (row 0 is the 45° edge) to N50, longitudes E005 to E011.
        assert_eq!(t.len(), 49);
        assert!(t.contains(&(44, 5)) && t.contains(&(50, 11)));
        assert!(!t.contains(&(43, 7)) && !t.contains(&(51, 7)) && !t.contains(&(46, 12)));
        let list = TileList::parse("Copernicus_DSM_COG_30_N46_00_E007_00_DEM\n");
        assert_eq!(urls_for_cell(&list, Product::Glo90, 33, 7).len(), 1);
        let cells = cells_with_land(&list, &BTreeSet::from([(31, 7)]));
        assert_eq!(cells, vec![(31, 7, 12), (33, 7, 10)]);
    }
}
