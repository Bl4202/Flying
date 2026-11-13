//! Distances and bearings (GDD 7.3) and the horizon (GDD 7.4.2).

use simmath::dmath;

use crate::angles::{RAD, delta_lon, wrap_tau};
use crate::wgs84::{A, B, F, MEAN_RADIUS_M, m_radius, n_radius};

/// How accurate a [`Geodesic`] is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Accuracy {
    /// Vincenty's inverse on the WGS84 ellipsoid converged (sub-millimetre).
    Ellipsoid,
    /// The spherical great circle on the mean radius, flagged (GDD 7.3): the points are within
    /// 0.5° of antipodal, or Vincenty did not converge.
    Spherical,
}

/// The answer of the inverse geodesic problem.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Geodesic {
    /// Distance along the geodesic, m.
    pub distance_m: f64,
    /// Initial bearing at the first point, clockwise from true north, rad in [0, 2π).
    pub bearing_a_rad: f64,
    /// Forward bearing at the second point, clockwise from true north, rad in [0, 2π).
    pub bearing_b_rad: f64,
    /// Which method answered.
    pub accuracy: Accuracy,
}

/// Separation at and above which [`inverse`] answers on the sphere, rad. // ALGO: GDD 7.3, 179.5°
const ANTIPODAL_RAD: f64 = 179.5 * RAD;

/// The most iterations of Vincenty's λ loop. // ALGO: convergence is slow only near antipodes
const VINCENTY_MAX_ITERATIONS: u32 = 200;

/// Vincenty's convergence threshold on λ, rad. // ALGO: about 6 µm on the ground
const VINCENTY_CONVERGED_RAD: f64 = 1e-12;

/// The haversine central angle between two points, rad (GDD 7.3).
#[must_use]
pub fn haversine_angle_rad(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let s_lat = dmath::sin((lat2 - lat1) * 0.5);
    let s_lon = dmath::sin(delta_lon(lon1, lon2) * 0.5);
    let a = s_lat * s_lat + dmath::cos(lat1) * dmath::cos(lat2) * s_lon * s_lon;
    2.0 * dmath::asin(a.clamp(0.0, 1.0).sqrt())
}

/// The great-circle distance on the mean radius R = 6 371 000 m, m (GDD 7.3).
#[must_use]
pub fn haversine_m(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    MEAN_RADIUS_M * haversine_angle_rad(lat1, lon1, lat2, lon2)
}

/// The initial great-circle bearing from point 1 to point 2, clockwise from true north, rad in
/// [0, 2π) (GDD 7.3): `θ = atan2(sin Δλ cos φ2, cos φ1 sin φ2 − sin φ1 cos φ2 cos Δλ)`.
#[must_use]
pub fn initial_bearing_rad(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let dl = delta_lon(lon1, lon2);
    let y = dmath::sin(dl) * dmath::cos(lat2);
    let x =
        dmath::cos(lat1) * dmath::sin(lat2) - dmath::sin(lat1) * dmath::cos(lat2) * dmath::cos(dl);
    wrap_tau(dmath::atan2(y, x))
}

/// The inverse geodesic problem on WGS84 (GDD 7.3).
///
/// Vincenty's inverse (at most 200 iterations, converged at `|Δλ| < 1e-12`). Points within 0.5° of
/// antipodal (haversine angle ≥ 179.5°), or a loop that does not converge, get the spherical
/// answer flagged [`Accuracy::Spherical`]. Coincident points give 0 m.
#[must_use]
pub fn inverse(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> Geodesic {
    let sigma_h = haversine_angle_rad(lat1, lon1, lat2, lon2);
    if sigma_h >= ANTIPODAL_RAD {
        return spherical(lat1, lon1, lat2, lon2, sigma_h);
    }
    vincenty(lat1, lon1, lat2, lon2).unwrap_or_else(|| spherical(lat1, lon1, lat2, lon2, sigma_h))
}

fn spherical(lat1: f64, lon1: f64, lat2: f64, lon2: f64, sigma: f64) -> Geodesic {
    let back = initial_bearing_rad(lat2, lon2, lat1, lon1);
    Geodesic {
        distance_m: MEAN_RADIUS_M * sigma,
        bearing_a_rad: initial_bearing_rad(lat1, lon1, lat2, lon2),
        bearing_b_rad: wrap_tau(back + core::f64::consts::PI),
        accuracy: Accuracy::Spherical,
    }
}

/// The reduced latitude `U = atan((1 − f) tan φ)`, as (sin U, cos U).
fn reduced(lat: f64) -> (f64, f64) {
    let u = dmath::atan2((1.0 - F) * dmath::sin(lat), dmath::cos(lat));
    (dmath::sin(u), dmath::cos(u))
}

/// Vincenty's inverse (1975). `None` if the λ loop does not converge.
fn vincenty(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> Option<Geodesic> {
    let l = delta_lon(lon1, lon2);
    let (su1, cu1) = reduced(lat1);
    let (su2, cu2) = reduced(lat2);
    let mut lambda = l;
    let mut it = 0;
    let (sin_sigma, cos_sigma, sigma, cos2_alpha, cos_2sm, sin_l, cos_l) = loop {
        let (sl, cl) = (dmath::sin(lambda), dmath::cos(lambda));
        let t1 = cu2 * sl;
        let t2 = cu1 * su2 - su1 * cu2 * cl;
        let sin_sigma = (t1 * t1 + t2 * t2).sqrt();
        if sin_sigma == 0.0 {
            return Some(Geodesic {
                distance_m: 0.0,
                bearing_a_rad: 0.0,
                bearing_b_rad: 0.0,
                accuracy: Accuracy::Ellipsoid,
            });
        }
        let cos_sigma = su1 * su2 + cu1 * cu2 * cl;
        let sigma = dmath::atan2(sin_sigma, cos_sigma);
        let sin_alpha = cu1 * cu2 * sl / sin_sigma;
        let cos2_alpha = 1.0 - sin_alpha * sin_alpha;
        let cos_2sm = if cos2_alpha == 0.0 {
            0.0 // ALGO: an equatorial line
        } else {
            cos_sigma - 2.0 * su1 * su2 / cos2_alpha
        };
        // ALGO: Vincenty's C = f/16 cos²α (4 + f (4 − 3 cos²α))
        let c = F / 16.0 * cos2_alpha * (4.0 + F * (4.0 - 3.0 * cos2_alpha));
        let next = l
            + (1.0 - c)
                * F
                * sin_alpha
                * (sigma
                    + c * sin_sigma * (cos_2sm + c * cos_sigma * (-1.0 + 2.0 * cos_2sm * cos_2sm)));
        let done = (next - lambda).abs() < VINCENTY_CONVERGED_RAD;
        lambda = next;
        it += 1;
        if done {
            break (sin_sigma, cos_sigma, sigma, cos2_alpha, cos_2sm, sl, cl);
        }
        if it >= VINCENTY_MAX_ITERATIONS || lambda.abs() > core::f64::consts::PI {
            return None;
        }
    };
    let u2 = cos2_alpha * (A * A - B * B) / (B * B);
    // ALGO: Vincenty's series coefficients A and B (1975, eq. 3 and 4)
    let big_a = 1.0 + u2 / 16_384.0 * (4_096.0 + u2 * (-768.0 + u2 * (320.0 - 175.0 * u2)));
    let big_b = u2 / 1_024.0 * (256.0 + u2 * (-128.0 + u2 * (74.0 - 47.0 * u2)));
    let c2 = cos_2sm * cos_2sm;
    let d_sigma = big_b
        * sin_sigma
        * (cos_2sm
            + big_b / 4.0
                * (cos_sigma * (-1.0 + 2.0 * c2)
                    - big_b / 6.0
                        * cos_2sm
                        * (-3.0 + 4.0 * sin_sigma * sin_sigma)
                        * (-3.0 + 4.0 * c2)));
    let distance_m = B * big_a * (sigma - d_sigma);
    // The bearings use the last pass's λ, within 1e-12 of the converged one.
    let a1 = dmath::atan2(cu2 * sin_l, cu1 * su2 - su1 * cu2 * cos_l);
    let a2 = dmath::atan2(cu1 * sin_l, -su1 * cu2 + cu1 * su2 * cos_l);
    Some(Geodesic {
        distance_m,
        bearing_a_rad: wrap_tau(a1),
        bearing_b_rad: wrap_tau(a2),
        accuracy: Accuracy::Ellipsoid,
    })
}

/// Metres per degree of latitude and of longitude at latitude `φ` on WGS84 (GDD 7.3):
/// `(M(φ) π/180, N(φ) cos φ π/180)`.
#[must_use]
pub fn metres_per_degree(lat_rad: f64) -> (f64, f64) {
    (
        m_radius(lat_rad) * RAD,
        n_radius(lat_rad) * dmath::cos(lat_rad) * RAD,
    )
}

/// The geometric horizon distance from height `h` over a sphere of radius `r`, m (GDD 7.4.2):
/// `sqrt(2 r h + h²)`.
#[must_use]
pub fn horizon_m(h_m: f64, r_m: f64) -> f64 {
    (2.0 * r_m * h_m + h_m * h_m).sqrt()
}

/// The visual dip of the horizon from height `h` over a sphere of radius `r`, rad (GDD 7.4.2):
/// `acos(r / (r + h))`.
#[must_use]
pub fn dip_rad(h_m: f64, r_m: f64) -> f64 {
    dmath::acos(r_m / (r_m + h_m))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::angles::DEG;

    #[test]
    fn ge8_the_gdd_metres_per_degree_table() {
        // GDD 7.3: latitude, m per ° latitude, m per ° longitude.
        let table: [(f64, f64, f64); 6] = [
            (0.0, 110_574.0, 111_319.0),
            (35.0, 110_941.0, 91_288.0),
            (45.0, 111_132.0, 78_847.0),
            (60.0, 111_412.0, 55_800.0),
            (70.0, 111_562.0, 38_187.0),
            (80.0, 111_660.0, 19_393.0),
        ];
        for (lat, m_lat, m_lon) in table {
            let (a, b) = metres_per_degree(lat * RAD);
            assert!(
                (a - m_lat).abs() < 1.0,
                "{lat}°: {a} m per ° latitude against {m_lat}"
            );
            assert!(
                (b - m_lon).abs() < 1.0,
                "{lat}°: {b} m per ° longitude against {m_lon}"
            );
        }
    }

    #[test]
    fn the_gdd_horizon_table() {
        // GDD 7.4.2 on R = 6 371 km: dip and geometric horizon.
        for (h, dip_deg, d_km) in [
            (100.0, 0.32, 35.7),
            (1_000.0, 1.02, 112.9),
            (5_000.0, 2.27, 252.5),
            (10_000.0, 3.21, 357.1),
            (15_000.0, 3.93, 437.4),
        ] {
            assert!(
                (dip_rad(h, MEAN_RADIUS_M) * DEG - dip_deg).abs() < 0.005,
                "{h}"
            );
            assert!(
                (horizon_m(h, MEAN_RADIUS_M) / 1_000.0 - d_km).abs() < 0.05,
                "{h}"
            );
        }
    }

    #[test]
    fn coincident_points_are_zero_apart() {
        let g = inverse(0.8, 0.14, 0.8, 0.14);
        assert_eq!(g.distance_m, 0.0);
        assert_eq!(g.accuracy, Accuracy::Ellipsoid);
    }

    #[test]
    fn bearings_east_and_north() {
        assert!((initial_bearing_rad(0.0, 0.0, 0.0, 0.1) - 90.0 * RAD).abs() < 1e-12);
        assert!(initial_bearing_rad(0.0, 0.0, 0.1, 0.0).abs() < 1e-12);
        // Across the antimeridian, 179° E to 179° W is due east.
        let b = initial_bearing_rad(0.0, 179.0 * RAD, 0.0, -179.0 * RAD);
        assert!((b - 90.0 * RAD).abs() < 1e-12, "{}", b * DEG);
    }

    #[test]
    fn antipodes_are_flagged_spherical() {
        let g = inverse(0.0, 0.0, 0.0, 179.8 * RAD);
        assert_eq!(g.accuracy, Accuracy::Spherical);
        assert!((g.distance_m - MEAN_RADIUS_M * 179.8 * RAD).abs() < 1e-6);
    }
}
