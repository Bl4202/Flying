//! Geodetic ↔ ECEF on the WGS84 ellipsoid (GDD 7.2.1, 7.2.2).

use simmath::dmath;
use simmath::math::DVec3;

use crate::wgs84::{B, E2, n_radius};

/// A geodetic position: latitude and longitude in radians, ellipsoid height in metres (MSL-as-
/// ellipsoid, ADR-05).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Geodetic {
    /// Latitude, radians, in [−π/2, π/2].
    pub lat_rad: f64,
    /// Longitude, radians, in (−π, π].
    pub lon_rad: f64,
    /// Height above the ellipsoid, metres.
    pub h_m: f64,
}

impl Geodetic {
    /// A geodetic position from its parts.
    #[must_use]
    pub const fn new(lat_rad: f64, lon_rad: f64, h_m: f64) -> Self {
        Self {
            lat_rad,
            lon_rad,
            h_m,
        }
    }
}

/// The largest number of iterations of [`ecef_to_geodetic`]. // ALGO: GDD 7.2.2 converges in 3–4
const MAX_ITERATIONS: u32 = 10;

/// The convergence threshold on latitude, rad. // ALGO: GDD 7.2.2
const CONVERGED_RAD: f64 = 1e-12;

/// Above this latitude the height is computed from Z (cos φ → 0), rad. // ALGO: GDD 7.2.2, 80°
const POLAR_FORM_RAD: f64 = 80.0 * core::f64::consts::PI / 180.0;

/// Below this distance from the polar axis a point is on the axis, m. // ALGO: no atan2(0, 0)
const ON_AXIS_M: f64 = 1e-9;

/// Geodetic → ECEF (GDD 7.2.1): `X = (N + h) cos φ cos λ`, `Y = (N + h) cos φ sin λ`,
/// `Z = (N (1 − e²) + h) sin φ`.
#[must_use]
pub fn geodetic_to_ecef(g: Geodetic) -> DVec3 {
    let n = n_radius(g.lat_rad);
    let (sp, cp) = (dmath::sin(g.lat_rad), dmath::cos(g.lat_rad));
    let (sl, cl) = (dmath::sin(g.lon_rad), dmath::cos(g.lon_rad));
    DVec3::new(
        (n + g.h_m) * cp * cl,
        (n + g.h_m) * cp * sl,
        (n * (1.0 - E2) + g.h_m) * sp,
    )
}

/// ECEF → geodetic (GDD 7.2.2), the iterative reference method.
///
/// `λ = atan2(Y, X)`, `p = sqrt(X² + Y²)`, `φ₀ = atan2(Z, p (1 − e²))`; then at most 10 times:
/// `N = N(φ)`, `h = p / cos φ − N` (or `Z / sin φ − N (1 − e²)` above 80°),
/// `φ' = atan2(Z, p (1 − e² N / (N + h)))`, until `|φ' − φ| < 1e-12`. The height is recomputed
/// with the final latitude. On the polar axis (`p < 1e-9 m`) it returns `φ = ±π/2`, `λ = 0`,
/// `h = |Z| − b`.
#[must_use]
pub fn ecef_to_geodetic(q: DVec3) -> Geodetic {
    let p = (q.x * q.x + q.y * q.y).sqrt();
    if p < ON_AXIS_M {
        let lat = if q.z < 0.0 {
            -core::f64::consts::FRAC_PI_2
        } else {
            core::f64::consts::FRAC_PI_2
        };
        return Geodetic::new(lat, 0.0, q.z.abs() - B);
    }
    let lon = dmath::atan2(q.y, q.x);
    let mut lat = dmath::atan2(q.z, p * (1.0 - E2));
    for _ in 0..MAX_ITERATIONS {
        let n = n_radius(lat);
        let h = height_at(q, p, lat, n);
        let next = dmath::atan2(q.z, p * (1.0 - E2 * n / (n + h)));
        let done = (next - lat).abs() < CONVERGED_RAD;
        lat = next;
        if done {
            break;
        }
    }
    let h = height_at(q, p, lat, n_radius(lat));
    Geodetic::new(lat, lon, h)
}

/// The ellipsoid height of `q` for the latitude `lat` (GDD 7.2.2's two forms).
fn height_at(q: DVec3, p: f64, lat: f64, n: f64) -> f64 {
    if lat.abs() <= POLAR_FORM_RAD {
        p / dmath::cos(lat) - n
    } else {
        q.z / dmath::sin(lat) - n * (1.0 - E2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::angles::RAD;
    use crate::wgs84::A;

    #[test]
    fn ge1_the_gdd_vector() {
        let q = geodetic_to_ecef(Geodetic::new(36.2 * RAD, -121.8 * RAD, 1_500.0));
        assert!((q.x - -2_716_000.612).abs() < 1e-3, "x {}", q.x);
        assert!((q.y - -4_380_460.552).abs() < 1e-3, "y {}", q.y);
        assert!((q.z - 3_747_008.625).abs() < 1e-3, "z {}", q.z);
    }

    #[test]
    fn the_poles_and_the_axis() {
        let n = geodetic_to_ecef(Geodetic::new(core::f64::consts::FRAC_PI_2, 0.0, 100.0));
        assert!(n.x.abs() < 1e-6 && (n.z - (B + 100.0)).abs() < 1e-6);
        let g = ecef_to_geodetic(DVec3::new(0.0, 0.0, -(B + 250.0)));
        assert_eq!(g.lat_rad, -core::f64::consts::FRAC_PI_2);
        assert_eq!(g.lon_rad, 0.0);
        assert!((g.h_m - 250.0).abs() < 1e-9);
    }

    #[test]
    fn a_point_on_the_equator() {
        let g = ecef_to_geodetic(DVec3::new(A, 0.0, 0.0));
        assert_eq!(g.lat_rad, 0.0);
        assert_eq!(g.lon_rad, 0.0);
        assert!(g.h_m.abs() < 1e-9);
    }
}
