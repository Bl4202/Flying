//! Weapons (GDD 15, ADR-46): the gun's rounds, the director gunsight, the infrared seeker, the
//! proportional-navigation missile, its proximity fuze and the capsules that hits are tested
//! against.
//!
//! Layering (ADR-29, ADR-46): this crate depends only on `content`, `flight` and `simmath`. It
//! never touches the world: `sim-core` hands it plain structs (positions, velocities, attitudes,
//! the weapon's parameters) and an [`flight::env::FlightEnv`], and keeps every pool, timer and
//! random stream itself. Random numbers come in as arguments (standard normals drawn by
//! `sim-core`), so every function here is a pure function of its inputs and allocates nothing.
//!
//! Conventions (GDD 0.3.2): SI units, radians; BF is East-North-Up and curved; BODY is
//! forward-right-down from the aircraft's reference point; an attitude rotates BODY to BF.
#![deny(missing_docs)]

pub mod ballistics;
pub mod capsule;
pub mod fuze;
pub mod ir;
pub mod missile;
pub mod predict;
pub mod seeker;
pub mod sight;

pub use ballistics::{GunBallistics, Round};
pub use capsule::Capsule;
pub use fuze::Detonation;
pub use ir::IrSignature;
pub use missile::{Missile, MissileParams, MissilePhase};
pub use seeker::{SeekerMode, SeekerParams, SeekerState, TargetView};
pub use sight::SightSolution;

use simmath::math::DVec3;

/// Two unit vectors perpendicular to `axis` and to each other, `(e1, e2)` with `e2 = axis × e1`.
///
/// `e1 = normalize(axis × Z)`; when `axis` is within about 0.06° of `±Z`, `X` takes the place of
/// `Z`. For a gun axis in body axes (`Z` = down) `e1` points left of the axis and `e2` up. `axis`
/// must be unit length.
#[must_use]
pub fn orthonormal_pair(axis: DVec3) -> (DVec3, DVec3) {
    // ALGO: below 1e-6 (|sin| about 1e-3) the cross product with Z loses too many digits.
    let c = axis.cross(DVec3::Z);
    let e1 = if c.length_squared() > 1e-6 {
        c.normalized_or(DVec3::Y)
    } else {
        axis.cross(DVec3::X).normalized_or(DVec3::Y)
    };
    (e1, axis.cross(e1))
}

#[cfg(test)]
pub(crate) mod test_content {
    //! The core content pack, baked once for the unit tests.

    use std::sync::OnceLock;

    use content::{AircraftDef, Registry, WeaponClass, WeaponDef};

    /// The baked `content/core` pack.
    pub(crate) fn registry() -> &'static Registry {
        static PACK: OnceLock<Registry> = OnceLock::new();
        PACK.get_or_init(|| {
            let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content/core");
            let baked = content_bake::bake::bake_dir(&dir)
                .unwrap_or_else(|d| panic!("the core pack bakes: {d:?}"));
            Registry::from_cpack(&baked.cpack).expect("the baked pack reads back")
        })
    }

    /// The first aircraft of the pack (the Kestrel).
    pub(crate) fn kestrel() -> &'static AircraftDef {
        let r = registry();
        let h = r.aircraft_handles().next().expect("an aircraft");
        r.aircraft(h).expect("aircraft")
    }

    /// The Kestrel's gun (the GS-27).
    pub(crate) fn gun() -> &'static WeaponDef {
        let install = kestrel().gun.as_ref().expect("a gun");
        let w = registry().weapon(install.weapon).expect("the gun's entry");
        assert_eq!(w.class, WeaponClass::Gun);
        w
    }

    /// The Kestrel's first rail store (the Dart-1).
    pub(crate) fn missile() -> &'static WeaponDef {
        let rail = kestrel().rails.first().expect("a rail");
        let w = registry().weapon(rail.store).expect("the store's entry");
        assert_eq!(w.class, WeaponClass::Srm);
        w
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pair_is_orthonormal_and_right_handed() {
        // Worked example: the gun axis straight ahead in body axes gives e1 = X × Z = -Y (left)
        // and e2 = X × -Y = -Z (up).
        let (e1, e2) = orthonormal_pair(DVec3::X);
        assert_eq!(e1, DVec3::new(0.0, -1.0, 0.0));
        assert_eq!(e2, DVec3::new(0.0, 0.0, -1.0));
        for axis in [
            DVec3::new(0.6, 0.0, -0.8),
            DVec3::Z,
            DVec3::new(0.0, 0.0, -1.0),
            DVec3::new(1e-5, 0.0, 1.0).normalized_or(DVec3::Z),
            DVec3::new(0.3, -0.4, 0.5).normalized_or(DVec3::Z),
        ] {
            let (e1, e2) = orthonormal_pair(axis);
            assert!((e1.length() - 1.0).abs() < 1e-12, "{axis:?}");
            assert!((e2.length() - 1.0).abs() < 1e-12, "{axis:?}");
            assert!(e1.dot(axis).abs() < 1e-12 && e2.dot(axis).abs() < 1e-12);
            assert!(e1.dot(e2).abs() < 1e-12);
            assert!((axis.cross(e1) - e2).length() < 1e-12);
        }
    }
}
