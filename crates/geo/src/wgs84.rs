//! The WGS84 ellipsoid (GDD 0.6.5) and its radii of curvature.

use simmath::dmath;

/// Semi-major axis `a`, m. // PHYS: WGS84 (GDD 0.6.5)
pub const A: f64 = 6_378_137.0;

/// Flattening `f`. // PHYS: WGS84 (GDD 0.6.5), 1 / 298.257 223 563
pub const F: f64 = 1.0 / 298.257_223_563;

/// First eccentricity squared, `e² = f (2 − f)` (= 0.006 694 379 990 141 3).
pub const E2: f64 = e2(F);

/// Semi-minor axis `b = a (1 − f)`, m (= 6 356 752.314 245).
pub const B: f64 = semi_minor(A, F);

/// The mean Earth radius of the spherical formulas (haversine, GDD 7.3), m. // PHYS: GDD 0.6.5
pub const MEAN_RADIUS_M: f64 = 6_371_000.0;

const fn e2(f: f64) -> f64 {
    f * (2.0 - f)
}

const fn semi_minor(a: f64, f: f64) -> f64 {
    a * (1.0 - f)
}

/// The prime-vertical radius of curvature `N(φ) = a / sqrt(1 − e² sin²φ)`, m.
#[must_use]
pub fn n_radius(lat_rad: f64) -> f64 {
    let s = dmath::sin(lat_rad);
    A / (1.0 - E2 * s * s).sqrt()
}

/// The meridional radius of curvature `M(φ) = a (1 − e²) / (1 − e² sin²φ)^1.5`, m.
#[must_use]
pub fn m_radius(lat_rad: f64) -> f64 {
    let s = dmath::sin(lat_rad);
    let w = 1.0 - E2 * s * s;
    A * (1.0 - E2) / (w * w.sqrt())
}

/// The geocentric radius of the ellipsoid at geodetic latitude `φ` (the distance from the Earth's
/// centre to the surface point), m.
#[must_use]
pub fn geocentric_radius(lat_rad: f64) -> f64 {
    let n = n_radius(lat_rad);
    let c = dmath::cos(lat_rad);
    let s = dmath::sin(lat_rad);
    let p = n * c;
    let z = n * (1.0 - E2) * s;
    (p * p + z * z).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_derived_constants_match_the_published_values() {
        // NIMA TR8350.2 table 3.3 prints e² = 0.006 694 379 990 14 and b = 6 356 752.3142 m.
        assert!((E2 - 0.006_694_379_990_14).abs() < 5e-15);
        assert!((B - 6_356_752.314_2).abs() < 1e-4);
    }

    #[test]
    fn the_radii_at_the_equator_and_the_pole() {
        assert_eq!(n_radius(0.0), A);
        assert!((m_radius(0.0) - A * (1.0 - E2)).abs() < 1e-6);
        let pole = core::f64::consts::FRAC_PI_2;
        // At the pole N = M = a² / b.
        assert!((n_radius(pole) - A * A / B).abs() < 1e-6);
        assert!((m_radius(pole) - A * A / B).abs() < 1e-6);
        assert!((geocentric_radius(0.0) - A).abs() < 1e-6);
        assert!((geocentric_radius(pole) - B).abs() < 1e-6);
    }
}
