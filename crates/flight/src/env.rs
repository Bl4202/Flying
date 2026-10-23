//! What the flight model needs to know about the world (ADR-29).
//!
//! `sim-core` implements [`FlightEnv`] over its `GeoService` and `WeatherProvider` seams;
//! `flight-test` and the unit tests use the analytic worlds here. Everything is a pure function
//! of position and time, so the model stays deterministic.

use simmath::math::DVec3;

use crate::atmosphere::NonStandardDay;
use crate::physical::{EARTH_RADIUS_M, G0};
use crate::surface::{SurfaceKind, SurfaceSample};

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

    /// The ground below a BF position: its height (equal to [`FlightEnv::ground_height_m`]), its
    /// normal in BF, what it is made of and how wet it is.
    fn surface(&self, pos_bf: DVec3) -> SurfaceSample;

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
/// standard day. Its ground is a smooth, dry surface of one kind: paved by default, so a wheel
/// test on it runs on asphalt; [`SphereSea::open`] has no ground at all.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SphereSea {
    /// Sphere radius, m.
    pub radius_m: f64,
    /// Height of the surface above MSL, m; `-inf` for none.
    pub ground_m: f64,
    /// What the surface is.
    pub kind: SurfaceKind,
}

impl Default for SphereSea {
    fn default() -> Self {
        Self {
            radius_m: EARTH_RADIUS_M,
            ground_m: 0.0,
            kind: SurfaceKind::Paved,
        }
    }
}

impl SphereSea {
    /// The same sphere with no surface: the air goes on below MSL and nothing is ever in ground
    /// effect. The Performance Conformance Test flies here, so its sea-level points measure the
    /// aircraft and not the ground (ADR-31).
    #[must_use]
    pub fn open() -> Self {
        Self {
            ground_m: f64::NEG_INFINITY,
            kind: SurfaceKind::Water,
            ..Self::default()
        }
    }

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
        self.ground_m
    }

    fn surface(&self, pos_bf: DVec3) -> SurfaceSample {
        SurfaceSample {
            height_msl_m: self.ground_m,
            normal_bf: self.up_bf(pos_bf),
            kind: self.kind,
            wetness: 0.0,
        }
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

    fn surface(&self, _pos_bf: DVec3) -> SurfaceSample {
        SurfaceSample {
            height_msl_m: 0.0,
            normal_bf: DVec3::Z,
            kind: SurfaceKind::Water,
            wetness: 0.0,
        }
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

    #[test]
    fn the_surface_agrees_with_the_ground_height_and_faces_up() {
        for w in [SphereSea::default(), SphereSea::open()] {
            for p in [DVec3::ZERO, DVec3::new(12_000.0, -4_000.0, 30.0)] {
                let s = w.surface(p);
                assert_eq!(s.height_msl_m, w.ground_height_m(p));
                assert!((s.normal_bf - w.up_bf(p)).length() < 1e-15);
                assert_eq!(s.wetness, 0.0);
            }
        }
        assert_eq!(
            SphereSea::default().surface(DVec3::ZERO).kind,
            SurfaceKind::Paved
        );
        assert_eq!(
            SphereSea::open().surface(DVec3::ZERO).kind,
            SurfaceKind::Water
        );
        let flat = FlatSea.surface(DVec3::new(1.0, 2.0, 50.0));
        assert_eq!((flat.height_msl_m, flat.normal_bf), (0.0, DVec3::Z));
    }

    #[test]
    fn the_open_sphere_has_no_ground() {
        let open = SphereSea::open();
        assert_eq!(open.ground_height_m(DVec3::ZERO), f64::NEG_INFINITY);
        assert_eq!(open.height_msl_m(DVec3::ZERO), 0.0);
        assert_eq!(SphereSea::default().ground_height_m(DVec3::ZERO), 0.0);
    }
}
