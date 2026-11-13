//! The offline geodata pipeline (GDD 45.5, ADR-54): Copernicus DEM tiles in, `GeoPack` v1 archives
//! out, one per 5.625° cell plus the base pack, with a QA report, a licence audit and
//! `datapack.json`.
//!
//! * [`sources`]: the Copernicus tile lists, file names and URLs; which cells have land.
//! * [`tiff`]: the minimal `GeoTIFF` reader (float32, zlib, predictor 3, tiled or stripped).
//! * [`raster`]: the source rasters of a cell and the bilinear sampler across 1° tiles.
//! * [`cell`]: one cell's pyramid: the finest level from the source, coarser levels by decimation.
//! * [`base`]: the base pack (z3 to z5) from every cell's z6 tiles.
//! * [`synth`]: the synthetic fixture (two islands in a sea).
//! * [`qa`], [`audit`], [`manifest`], [`descriptors`]: the reports, the licence audit and the
//!   datapack manifest.
//!
//! Every output is a pure function of the inputs: tiles are sorted by id before they are written,
//! and no file records a time.
#![deny(missing_docs)]

pub mod audit;
pub mod base;
pub mod cell;
pub mod descriptors;
pub mod manifest;
pub mod pyramid;
pub mod qa;
pub mod raster;
pub mod sources;
pub mod synth;
pub mod tiff;

use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use sha2::{Digest, Sha256};

/// Why a command failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
    /// A check failed (exit 1).
    Check(String),
    /// Bad arguments (exit 2).
    Usage(String),
    /// A file could not be read or written (exit 2).
    Io(String),
}

/// An I/O error with its path.
pub fn io_err(path: &Path, e: &impl std::fmt::Display) -> Failure {
    Failure::Io(format!("{}: {e}", path.display()))
}

/// Reads a file.
///
/// # Errors
/// [`Failure::Io`].
pub fn read(path: &Path) -> Result<Vec<u8>, Failure> {
    fs::read(path).map_err(|e| io_err(path, &e))
}

/// Writes a file, creating its directory.
///
/// # Errors
/// [`Failure::Io`].
pub fn write(path: &Path, bytes: &[u8]) -> Result<(), Failure> {
    if let Some(d) = path.parent() {
        fs::create_dir_all(d).map_err(|e| io_err(d, &e))?;
    }
    fs::write(path, bytes).map_err(|e| io_err(path, &e))
}

/// The lower-case hex SHA-256 of `bytes`.
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    let d = Sha256::digest(bytes);
    let mut s = String::with_capacity(64);
    for b in d {
        let _ = write!(s, "{b:02x}");
    }
    s
}

/// The days since 1970-01-01 of an ISO date `YYYY-MM-DD` (proleptic Gregorian).
///
/// # Errors
/// [`Failure::Usage`] if it is not a date.
pub fn days_from_iso(date: &str) -> Result<i64, Failure> {
    let bad = || Failure::Usage(format!("not an ISO date: {date:?}"));
    let parts: Vec<&str> = date.split('-').collect();
    if parts.len() != 3 || parts[0].len() != 4 || parts[1].len() != 2 || parts[2].len() != 2 {
        return Err(bad());
    }
    let y: i64 = parts[0].parse().map_err(|_| bad())?;
    let m: i64 = parts[1].parse().map_err(|_| bad())?;
    let d: i64 = parts[2].parse().map_err(|_| bad())?;
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return Err(bad());
    }
    // ALGO: Howard Hinnant's days_from_civil.
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Ok(era * 146_097 + doe - 719_468)
}

/// The cell's file stem, `5-X-Y`.
#[must_use]
pub fn cell_stem(x: u32, y: u32) -> String {
    format!("5-{x}-{y}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso_dates() {
        assert_eq!(days_from_iso("1970-01-01"), Ok(0));
        assert_eq!(
            days_from_iso("2026-11-13").unwrap() - days_from_iso("2025-11-13").unwrap(),
            365
        );
        assert_eq!(
            days_from_iso("2024-03-01").unwrap() - days_from_iso("2024-02-28").unwrap(),
            2
        );
        assert!(days_from_iso("2026-13-01").is_err());
        assert!(days_from_iso("26-1-1").is_err());
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
