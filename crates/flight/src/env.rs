//! What the flight model needs to know about the world (ADR-29).
//!
//! `sim-core` implements [`FlightEnv`] over its `GeoService` and `WeatherProvider` seams;
//! `flight-test` and the unit tests use the analytic worlds here. Everything is a pure function
//! of position and time, so the model stays deterministic.

use simmath::math::DVec3;

use crate::atmosphere::NonStandardDay;
use crate::physical::{EARTH_RADIUS_M, G0};

/// The world as seen by one aircraft.
pub trait FlightEnv {
    /// Geometric height above MSL of a BF position, m.
    fn height_msl_m(&self, pos_bf: DVec3) -> f64;

    /// Local up (the outward normal of the reference surface) at a BF position, unit length.
    fn up_bf(&self, pos_bf: DVec3) -> DVec3;

    /// Radius used for gravity with height, m.
    fn earth_radius_m(&self) -> f64 {
        EARTH_RADIUS_M
    }

    /// Gravitational acceleration in BF, m/s^2: `g0 (R / (R + h))^2` along local down (GDD 7.4.3).
    fn gravity_bf(&self, pos_bf: DVec3) -> DVec3 {
        let r = self.earth_radius_m();
        let ratio = r / (r + self.height_msl_m(pos_bf));
        self.up_bf(pos_bf) * (-G0 * ratio * ratio)
    }

    /// Terrain (or sea) height above MSL below a BF position, m.
    fn ground_height_m(&self, pos_bf: DVec3) -> f64;

    /// Wind velocity in BF at a position and simulation time, m/s.
    fn wind_bf(&self, _pos_bf: DVec3, _t_s: f64) -> DVec3 {
        DVec3::ZERO
    }

    /// Temperature and pressure offsets of the day.
    fn day(&self) -> NonStandardDay {
        NonStandardDay::ISA
    }
}

/// A sphere covered by sea at height 0, BF origin on its surface with `+z` up there. Still air,
/// standard day. The Performance Conformance Test flies in this world (ADR-31).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SphereSea {
    /// Sphere radius, m.
    pub radius_m: f64,
}

impl Default for SphereSea {
    fn default() -> Self {
        Self {
            radius_m: EARTH_RADIUS_M,
        }
    }
}

impl SphereSea {
    fn centre_bf(self) -> DVec3 {
        DVec3::new(0.0, 0.0, -self.radius_m)
    }
}

impl FlightEnv for SphereSea {
    fn height_msl_m(&self, pos_bf: DVec3) -> f64 {
        (pos_bf - self.centre_bf()).length() - self.radius_m
    }

    fn up_bf(&self, pos_bf: DVec3) -> DVec3 {
        (pos_bf - self.centre_bf()).normalized_or(DVec3::Z)
    }

    fn earth_radius_m(&self) -> f64 {
        self.radius_m
    }

    fn ground_height_m(&self, _pos_bf: DVec3) -> f64 {
        0.0
    }
}

/// A flat sea at `z = 0` with `+z` up everywhere. Gravity still weakens with height.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FlatSea;

impl FlightEnv for FlatSea {
    fn height_msl_m(&self, pos_bf: DVec3) -> f64 {
        pos_bf.z
    }

    fn up_bf(&self, _pos_bf: DVec3) -> DVec3 {
        DVec3::Z
    }

    fn ground_height_m(&self, _pos_bf: DVec3) -> f64 {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sphere_heights_and_up_vectors() {
        let w = SphereSea::default();
        assert_eq!(w.height_msl_m(DVec3::ZERO), 0.0);
        assert_eq!(w.up_bf(DVec3::ZERO), DVec3::Z);
        assert!((w.height_msl_m(DVec3::new(0.0, 0.0, 11_000.0)) - 11_000.0).abs() < 1e-9);
        // 100 km along the tangent plane, the surface has dropped d^2 / 2R = 785 m (GDD B.1).
        let far = DVec3::new(100_000.0, 0.0, 0.0);
        assert!(
            (w.height_msl_m(far) - 784.8).abs() < 0.1,
            "{}",
            w.height_msl_m(far)
        );
        let up = w.up_bf(far);
        assert!((up.length() - 1.0).abs() < 1e-15);
        let expected_x = 100_000.0 / (100_000.0_f64 * 100_000.0 + w.radius_m * w.radius_m).sqrt();
        assert!((up.x - expected_x).abs() < 1e-15);
    }

    #[test]
    fn gravity_points_down_and_weakens_with_height() {
        let w = SphereSea::default();
        assert_eq!(w.gravity_bf(DVec3::ZERO), DVec3::new(0.0, 0.0, -G0));
        let g11 = w.gravity_bf(DVec3::new(0.0, 0.0, 11_000.0)).length();
        // (6371 / 6382)^2 = 0.99655.
        assert!((g11 / G0 - 0.996_555).abs() < 1e-5, "{}", g11 / G0);
        let far = DVec3::new(300_000.0, 0.0, 5_000.0);
        let g = w.gravity_bf(far);
        assert!((g.normalized_or(DVec3::Z) + w.up_bf(far)).length() < 1e-12);
        assert_eq!(
            FlatSea.gravity_bf(DVec3::new(5e5, 0.0, 0.0)),
            DVec3::new(0.0, 0.0, -G0)
        );
        assert_eq!(FlatSea.wind_bf(DVec3::ZERO, 0.0), DVec3::ZERO);
        assert_eq!(FlatSea.day(), NonStandardDay::ISA);
    }
}
