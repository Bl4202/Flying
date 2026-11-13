//! Angles: degree constants, longitude normalisation and latitude clamping (GDD 7.8).

use core::f64::consts::{FRAC_PI_2, PI, TAU};

/// Degrees per radian.
pub const DEG: f64 = 180.0 / PI;

/// Radians per degree.
pub const RAD: f64 = PI / 180.0;

/// A longitude normalised to (−π, π] (GDD 7.8).
#[must_use]
pub fn normalize_lon(lon_rad: f64) -> f64 {
    let mut l = lon_rad % TAU; // (−2π, 2π)
    if l <= -PI {
        l += TAU;
    } else if l > PI {
        l -= TAU;
    }
    l
}

/// The longitude difference `λ2 − λ1`, normalised to (−π, π] (GDD 7.8).
#[must_use]
pub fn delta_lon(lon1_rad: f64, lon2_rad: f64) -> f64 {
    normalize_lon(lon2_rad - lon1_rad)
}

/// A latitude clamped to [−π/2, π/2].
#[must_use]
pub fn clamp_lat(lat_rad: f64) -> f64 {
    lat_rad.clamp(-FRAC_PI_2, FRAC_PI_2)
}

/// `x` wrapped into [0, 2π).
#[must_use]
pub fn wrap_tau(x: f64) -> f64 {
    let mut w = x % TAU;
    if w < 0.0 {
        w += TAU;
    }
    if w >= TAU { 0.0 } else { w }
}

/// The unit vector `(cos φ cos λ, cos φ sin λ, sin φ)` of a direction on the sphere.
#[must_use]
pub fn unit_vector(lat_rad: f64, lon_rad: f64) -> [f64; 3] {
    let (sp, cp) = (simmath::dmath::sin(lat_rad), simmath::dmath::cos(lat_rad));
    let (sl, cl) = (simmath::dmath::sin(lon_rad), simmath::dmath::cos(lon_rad));
    [cp * cl, cp * sl, sp]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn longitudes_land_in_the_half_open_range() {
        assert_eq!(normalize_lon(PI), PI);
        assert_eq!(normalize_lon(-PI), PI);
        assert_eq!(normalize_lon(0.0), 0.0);
        assert!((normalize_lon(3.0 * PI) - PI).abs() < 1e-15);
        assert!((normalize_lon(190.0 * RAD) - -170.0 * RAD).abs() < 1e-12);
        assert!((delta_lon(179.0 * RAD, -179.0 * RAD) - 2.0 * RAD).abs() < 1e-12);
        assert!((delta_lon(-179.0 * RAD, 179.0 * RAD) - -2.0 * RAD).abs() < 1e-12);
    }

    #[test]
    fn latitudes_clamp_and_angles_wrap() {
        assert_eq!(clamp_lat(2.0), FRAC_PI_2);
        assert_eq!(clamp_lat(-2.0), -FRAC_PI_2);
        assert_eq!(wrap_tau(-0.5), TAU - 0.5);
        assert_eq!(wrap_tau(TAU), 0.0);
        assert_eq!(wrap_tau(1.0), 1.0);
    }
}
