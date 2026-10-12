//! `LosService`: can A see B? (GDD 16.9, 61.5). The first implementations are "open sky" and a
//! terrain-only ray-march delegated to a [`GeoService`]; max-mip horizon profiles with clouds and
//! buildings replace them in M4.

use super::geo::{GeoService, LosResult};
use crate::math::DVec3;

/// Line-of-sight queries between BF points.
pub trait LosService: Send {
    /// Whether `a` can see `b`. `k_factor` is the effective-Earth-radius factor (1.0 visual,
    /// 4/3 radar).
    fn los(&self, a: DVec3, b: DVec3, k_factor: f64) -> LosResult;
}

/// Nothing ever blocks the view.
#[derive(Clone, Copy, Debug, Default)]
pub struct OpenSky;

impl LosService for OpenSky {
    fn los(&self, _a: DVec3, _b: DVec3, _k_factor: f64) -> LosResult {
        LosResult {
            visible: true,
            blocker: None,
        }
    }
}

/// Terrain-only line of sight, answered by the world's [`GeoService`].
pub struct TerrainLos {
    geo: Box<dyn GeoService>,
}

impl TerrainLos {
    /// Wraps a geo service.
    #[must_use]
    pub fn new(geo: Box<dyn GeoService>) -> Self {
        Self { geo }
    }
}

impl LosService for TerrainLos {
    fn los(&self, a: DVec3, b: DVec3, k_factor: f64) -> LosResult {
        self.geo.los(a, b, k_factor)
    }
}

/// Behaviour every `LosService` must have (`AC-61.5`).
///
/// # Panics
/// Panics, with a message naming the property, if the implementation violates the contract.
pub fn contract(s: &dyn LosService) {
    let high = [
        DVec3::new(0.0, 0.0, 3_000.0),
        DVec3::new(10_000.0, -4_000.0, 6_000.0),
        DVec3::new(-20_000.0, 15_000.0, 4_500.0),
    ];
    // Low enough to be inside the ground in the demo world. Seeing itself is only required of
    // points in open air, so this one joins the pairwise checks but not that one.
    let low = DVec3::new(2_000.0, 2_000.0, -30.0);
    let points = [high[0], high[1], high[2], low];
    for &a in &high {
        assert!(s.los(a, a, 1.0).visible, "a point sees itself: {a:?}");
    }
    for &a in &points {
        for &b in &points {
            let ab = s.los(a, b, 1.0);
            let ba = s.los(b, a, 1.0);
            assert_eq!(ab.visible, ba.visible, "symmetric for {a:?} / {b:?}");
            assert_eq!(ab, s.los(a, b, 1.0), "repeatable for {a:?} / {b:?}");
            if !ab.visible {
                assert!(
                    ab.blocker.is_some_and(DVec3::is_finite),
                    "a blocked view names a finite blocker"
                );
            }
        }
    }
    // Garbage in must not panic; it must not claim a clear view with a blocker attached.
    let bad = s.los(DVec3::new(f64::NAN, 0.0, 0.0), points[0], 1.0);
    assert!(!(bad.visible && bad.blocker.is_some()));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::seams::geo::ProceduralGeo;

    #[test]
    fn open_sky_passes_the_contract() {
        contract(&OpenSky);
    }

    #[test]
    fn terrain_los_passes_the_contract_and_sees_terrain() {
        let s = TerrainLos::new(Box::new(ProceduralGeo::default_frame()));
        contract(&s);
        let underground = DVec3::new(0.0, 0.0, -500.0);
        assert!(
            !s.los(DVec3::new(0.0, 0.0, 4_000.0), underground, 1.0)
                .visible
        );
        assert!(
            OpenSky
                .los(DVec3::new(0.0, 0.0, 4_000.0), underground, 1.0)
                .visible
        );
    }
}
