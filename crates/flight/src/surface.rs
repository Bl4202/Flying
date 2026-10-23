//! What the ground is made of and how grippy it is (GDD 10.4, 11.13.2, ADR-35).
//!
//! The coefficients are the GDD's design targets for the surfaces of v0.1b. They are not aircraft
//! data and not yet world content; they move into the world pack at M2.

use simmath::math::DVec3;

/// What the wheels run on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SurfaceKind {
    /// Asphalt or concrete.
    #[default]
    Paved,
    /// Grass.
    Grass,
    /// Dirt or gravel.
    Dirt,
    /// Packed snow or ice.
    Snow,
    /// Water (a wheel on the sea is a wreck, but the model must give an answer).
    Water,
}

/// The ground at a point: where it is, which way it faces and what it is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SurfaceSample {
    /// Height above MSL, m.
    pub height_msl_m: f64,
    /// Unit normal in BF, pointing out of the ground.
    pub normal_bf: DVec3,
    /// What the surface is.
    pub kind: SurfaceKind,
    /// 0 dry to 1 wet (GDD 24.4); snow and ice are kinds, not wetness.
    pub wetness: f64,
}

/// Friction coefficients of a surface in its current state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Friction {
    /// Rolling resistance `mu_r`.
    pub mu_roll: f64,
    /// Braking friction `mu_b`.
    pub mu_brake: f64,
    /// Peak sideways friction (the cornering limit).
    pub mu_lat: f64,
}

/// Rolling resistance, dry braking friction and wet braking friction of a surface.
/// // DATA: GDD 10.4 design targets.
const TABLE: [(f64, f64, f64); 5] = [
    // Paved
    (0.02, 0.70, 0.40),
    // Grass
    (0.05, 0.40, 0.30),
    // Dirt
    (0.04, 0.50, 0.35),
    // Snow: no wet state; the same value both ways.
    (0.04, 0.20, 0.20),
    // Water: a placeholder with heavy drag; ditching is not modelled.
    (0.10, 0.20, 0.20),
];

impl SurfaceKind {
    fn index(self) -> usize {
        match self {
            Self::Paved => 0,
            Self::Grass => 1,
            Self::Dirt => 2,
            Self::Snow => 3,
            Self::Water => 4,
        }
    }
}

/// The friction of a surface at a wetness (0 dry, 1 wet; clamped). The sideways limit follows the
/// braking value, so a wet runway corners worse too. // ALGO: ADR-35
#[must_use]
pub fn friction(kind: SurfaceKind, wetness: f64) -> Friction {
    let (roll, dry, wet) = TABLE[kind.index()];
    let w = if wetness.is_nan() {
        0.0
    } else {
        wetness.clamp(0.0, 1.0)
    };
    let mu_b = dry + (wet - dry) * w;
    Friction {
        mu_roll: roll,
        mu_brake: mu_b,
        mu_lat: mu_b,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_is_the_gdd_table() {
        let dry = friction(SurfaceKind::Paved, 0.0);
        assert_eq!((dry.mu_roll, dry.mu_brake, dry.mu_lat), (0.02, 0.70, 0.70));
        let wet = friction(SurfaceKind::Paved, 1.0);
        assert_eq!(wet.mu_brake, 0.40);
        let grass = friction(SurfaceKind::Grass, 0.0);
        assert_eq!((grass.mu_roll, grass.mu_brake), (0.05, 0.40));
        let dirt = friction(SurfaceKind::Dirt, 0.0);
        assert_eq!((dirt.mu_roll, dirt.mu_brake), (0.04, 0.50));
        let snow = friction(SurfaceKind::Snow, 0.0);
        assert_eq!((snow.mu_roll, snow.mu_brake), (0.04, 0.20));
    }

    #[test]
    fn wetness_interpolates_and_is_clamped() {
        let half = friction(SurfaceKind::Paved, 0.5);
        assert!((half.mu_brake - 0.55).abs() < 1e-12);
        assert_eq!(
            friction(SurfaceKind::Paved, 7.0),
            friction(SurfaceKind::Paved, 1.0)
        );
        assert_eq!(
            friction(SurfaceKind::Paved, f64::NAN),
            friction(SurfaceKind::Paved, 0.0)
        );
    }

    #[test]
    fn every_surface_has_positive_coefficients() {
        for kind in [
            SurfaceKind::Paved,
            SurfaceKind::Grass,
            SurfaceKind::Dirt,
            SurfaceKind::Snow,
            SurfaceKind::Water,
        ] {
            for w in [0.0, 0.5, 1.0] {
                let f = friction(kind, w);
                assert!(
                    f.mu_roll > 0.0 && f.mu_brake > 0.0 && f.mu_lat > 0.0,
                    "{kind:?}"
                );
                assert!(
                    f.mu_roll < f.mu_brake,
                    "{kind:?}: braking grips more than rolling"
                );
            }
        }
    }
}
