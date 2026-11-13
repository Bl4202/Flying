//! `synth`: the synthetic fixture (ADR-51 section 9.5, ADR-54). Two islands in a sea at exactly
//! 0 m; nothing is downloaded.
//!
//! **The islands:**
//! - **I1** at 46.55° N 8.0° E, in cell (33, 7), complete to z12.
//! - **I2** at 53.4° N 180.0° E, across the antimeridian, in cells (63, 6) and (0, 6), complete to
//!   z10.
//! - Each has an angular radius of 0.006 rad, about 38 km.
//!
//! **The height,** from the unit vector `u` and each island's centre vector `c`:
//! `h(u) = 2 500 m x max(0, 1 − |u − c| / 0.006)² x (0.6 + 0.4 n(u))`. It is continuous across
//! ±180° because it uses `u`, not the longitude.
//!
//! **The noise `n(u)`** is 3-D value noise in [0, 1]:
//! - an integer-hash lattice of spacing 0.002 in unit-vector space;
//! - 4 octaves, each with half the spacing and half the amplitude, normalised by the amplitude sum;
//! - trilinear interpolation with the fade `t² (3 − 2 t)`, hashed with integer operations only.
//!
//! **The output** is a complete datapack under `--out`: the cells, `base.gpk`, `qa.json`,
//! `licences.json` (descriptor `synthetic`) and `datapack.json`. Every date in it is fixed, so two
//! runs give the same bytes (GP1).

use std::path::Path;

use geo::angles::{RAD, unit_vector};
use geo::gpk::QUALITY_SYNTHETIC;
use geo::tile::TileBounds;
use geo_pack::SourceSet;

use crate::cell::{CellOut, DEFAULT_THREADS, build_levels};
use crate::descriptors::GeoSource;
use crate::pyramid::{HeightSource, Sample};
use crate::{Failure, audit, base, manifest, qa, write};

/// The fixture's datapack id.
pub const DATAPACK: &str = "geo-fixture";
/// The fixture's build, release and terms date (fixed, so it is reproducible). // DATA: ADR-54
pub const DATE: &str = "2026-11-12";
/// The fixture's descriptor id.
pub const LICENCE: &str = "synthetic";
/// The islands' centres, degrees. // DATA: ADR-51 section 9.5
pub const ISLANDS_DEG: [(f64, f64); 2] = [(46.55, 8.0), (53.4, 180.0)];
/// The islands' radius in unit-vector space. // DATA: ADR-51 section 9.5
pub const RADIUS: f64 = 0.006;
/// The peak height, m. // DATA: ADR-51 section 9.5
pub const PEAK_M: f64 = 2_500.0;
/// The coarsest noise lattice spacing. // DATA: ADR-51 section 9.5
pub const NOISE_SPACING: f64 = 0.002;
/// Noise octaves. // DATA: ADR-51 section 9.5
pub const OCTAVES: u32 = 4;
/// The fixture's cells and their finest levels. // DATA: ADR-51 section 9.5
pub const CELLS: [(u32, u32, u8); 3] = [(33, 7, 12), (63, 6, 10), (0, 6, 10)];

fn hash01(i: i64, j: i64, k: i64, octave: u32) -> f64 {
    // ALGO: multiplicative mixing, then the SplitMix64 finaliser.
    let mut h = (i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (j as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F)
        ^ (k as u64).wrapping_mul(0x1656_67B1_9E37_79F9)
        ^ u64::from(octave).wrapping_mul(0x27D4_EB2F_1656_67C5);
    h ^= h >> 30;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^= h >> 27;
    h = h.wrapping_mul(0x94D0_49BB_1331_11EB);
    h ^= h >> 31;
    (h >> 11) as f64 / (1u64 << 53) as f64
}

fn value_noise(u: [f64; 3], spacing: f64, octave: u32) -> f64 {
    let p = [u[0] / spacing, u[1] / spacing, u[2] / spacing];
    let base = [p[0].floor(), p[1].floor(), p[2].floor()];
    let f = [p[0] - base[0], p[1] - base[1], p[2] - base[2]];
    let w = f.map(|t| t * t * (3.0 - 2.0 * t));
    let (i, j, k) = (base[0] as i64, base[1] as i64, base[2] as i64);
    let mut acc = 0.0;
    for dz in 0..2i64 {
        for dy in 0..2i64 {
            for dx in 0..2i64 {
                let wx = if dx == 1 { w[0] } else { 1.0 - w[0] };
                let wy = if dy == 1 { w[1] } else { 1.0 - w[1] };
                let wz = if dz == 1 { w[2] } else { 1.0 - w[2] };
                acc += wx * wy * wz * hash01(i + dx, j + dy, k + dz, octave);
            }
        }
    }
    acc
}

/// The fixture's noise `n(u)` in [0, 1].
#[must_use]
pub fn noise(u: [f64; 3]) -> f64 {
    let (mut sum, mut norm, mut amp, mut s) = (0.0, 0.0, 1.0, NOISE_SPACING);
    for o in 0..OCTAVES {
        sum += amp * value_noise(u, s, o);
        norm += amp;
        amp *= 0.5;
        s *= 0.5;
    }
    sum / norm
}

fn chord(a: [f64; 3], b: [f64; 3]) -> f64 {
    ((a[0] - b[0]) * (a[0] - b[0]) + (a[1] - b[1]) * (a[1] - b[1]) + (a[2] - b[2]) * (a[2] - b[2]))
        .sqrt()
}

fn centres() -> [[f64; 3]; 2] {
    ISLANDS_DEG.map(|(lat, lon)| unit_vector(lat * RAD, lon * RAD))
}

/// The fixture's height at (`lat`, `lon`), rad (module docs).
#[must_use]
pub fn height(lat: f64, lon: f64) -> f64 {
    let u = unit_vector(lat, lon);
    let mut h = 0.0;
    for c in centres() {
        let d = chord(u, c);
        if d < RADIUS {
            let fall = 1.0 - d / RADIUS;
            h += PEAK_M * fall * fall * (0.6 + 0.4 * noise(u));
        }
    }
    h
}

/// The fixture as a height source.
#[derive(Clone, Copy, Debug, Default)]
pub struct Synth;

impl HeightSource for Synth {
    fn sample(&self, lat: f64, lon: f64) -> Sample {
        let h = height(lat, lon);
        Sample { h, zero: h == 0.0 }
    }

    fn may_hold_land(&self, b: &TileBounds) -> bool {
        let (clat, clon) = b.center();
        let cu = unit_vector(clat, clon);
        let mut r = 0.0f64;
        for lat in [b.lat_north, clat, b.lat_south] {
            for lon in [b.lon_west, clon, b.lon_east] {
                r = r.max(chord(cu, unit_vector(lat, lon)));
            }
        }
        // ALGO: 5 % over the farthest corner or edge midpoint covers the edges' bulge.
        centres().iter().any(|c| chord(cu, *c) <= RADIUS + 1.05 * r)
    }

    fn quality(&self) -> u8 {
        QUALITY_SYNTHETIC
    }
}

/// The fixture's descriptor.
#[must_use]
pub fn descriptor() -> GeoSource {
    GeoSource {
        schema: crate::descriptors::SCHEMA.into(),
        id: LICENCE.into(),
        name: "Synthetic test terrain".into(),
        licence: "Generated by geo-pipeline synth; no third-party data".into(),
        attribution: "Synthetic test terrain".into(),
        url: "https://github.com/Bl4202/Flying".into(),
        commercial_ok: true,
        cache_allowed: true,
        redistribution_allowed: true,
        share_alike: false,
        terms_checked: DATE.into(),
        required_notices: vec![],
        products: vec![],
        dump: None,
    }
}

/// Runs `synth`: the whole fixture datapack under `out`.
///
/// # Errors
/// I/O failures, or a failed QA or audit check.
pub fn run(out: &Path) -> Result<(), Failure> {
    for (x, y, max_level) in CELLS {
        let c = CellOut {
            x,
            y,
            max_level,
            datapack: DATAPACK,
            built: DATE,
            licences: vec![LICENCE.into()],
            sources: vec![SourceSet {
                product: "synthetic".into(),
                tile_list_sha256: String::new(),
                files: vec![],
            }],
            unfilled_voids: 0,
        };
        let levels = build_levels(&c, &Synth, DEFAULT_THREADS);
        c.write(out, &levels, DEFAULT_THREADS)?;
    }
    base::run(&out.join("z6"), DATAPACK, DATE, out)?;
    let text =
        serde_json::to_string_pretty(&descriptor()).map_err(|e| Failure::Io(e.to_string()))? + "\n";
    write(&out.join("sources/synthetic.json"), text.as_bytes())?;
    audit::run(out, &out.join("sources"), DATE)?;
    qa::run(out, &[])?;
    manifest::run(out, DATAPACK, DATE, None, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_height_function_at_five_points() {
        let at = |lat: f64, lon: f64| height(lat * RAD, lon * RAD);
        let i1 = at(46.55, 8.0);
        let i2 = at(53.4, 180.0);
        assert!((1_500.0..=2_500.0).contains(&i1), "{i1}");
        assert!((1_500.0..=2_500.0).contains(&i2), "{i2}");
        assert!(
            (at(53.4, -180.0) - i2).abs() < 1e-6,
            "continuous across 180°"
        );
        // 20 km east of each centre: about (1 − 20 / 38.2)² of the peak, times the noise.
        let east1 = at(46.55, 8.0 + 20.0 / (111.32 * geo::dmath::cos(46.55 * RAD)));
        assert!(east1 > 0.0 && east1 < i1, "{east1}");
        assert_eq!(at(46.55, 9.0), 0.0, "the sea");
        let n = noise(unit_vector(0.3, 0.4));
        assert!((0.0..=1.0).contains(&n));
    }
}
