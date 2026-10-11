//! Battlefield-frame to render-frame conversion (GDD 0.3.2, ADR-18).
//!
//! BF is East (+x), North (+y), Up (+z). The renderer is right-handed, Y-up:
//! `r.x = BF.x`, `r.y = BF.z`, `r.z = -BF.y`. That is a proper rotation (determinant +1) of
//! -90 degrees about the East axis, so no handedness flip is needed. This is the only place the
//! mapping is written in Rust; `packages/sim-bridge/src/frame-math.ts` mirrors it and both are
//! tested against `tests/vectors/frame_math.json`, which `cargo xtask gen-abi` produces from this
//! file. Only camera-relative data ever crosses into the renderer (ADR-03, GDD 44.8).

use core::f64::consts::FRAC_1_SQRT_2;

use crate::math::{DQuat, DVec3};

/// The BF -> render rotation as a quaternion: -90 degrees about +X.
pub const BF_TO_RENDER: DQuat = DQuat::new(FRAC_1_SQRT_2, -FRAC_1_SQRT_2, 0.0, 0.0);

/// A BF vector (direction or camera-relative offset) in render axes.
#[must_use]
pub const fn to_render_vec(bf: DVec3) -> DVec3 {
    DVec3::new(bf.x, bf.z, -bf.y)
}

/// A render-axes vector back in BF.
#[must_use]
pub const fn from_render_vec(r: DVec3) -> DVec3 {
    DVec3::new(r.x, -r.z, r.y)
}

/// A BF rotation expressed in render axes: `R q R^-1`.
///
/// If `q` turns BF vector `v` into `q v`, the result turns `to_render_vec(v)` into
/// `to_render_vec(q v)`.
#[must_use]
pub fn to_render_quat(q_bf: DQuat) -> DQuat {
    BF_TO_RENDER * q_bf * BF_TO_RENDER.conjugate()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: DVec3, b: DVec3) -> bool {
        (a - b).length() <= 1e-14
    }

    #[test]
    fn cardinal_directions_map_as_specified() {
        // East stays +X, Up becomes +Y, North becomes -Z.
        assert_eq!(to_render_vec(DVec3::X), DVec3::X);
        assert_eq!(to_render_vec(DVec3::Z), DVec3::Y);
        assert_eq!(to_render_vec(DVec3::Y), -DVec3::Z);
    }

    #[test]
    fn mapping_is_a_proper_rotation() {
        // det = +1 means cross products map to cross products.
        let (a, b) = (DVec3::new(1.0, 2.0, 3.0), DVec3::new(-4.0, 0.5, 2.0));
        let lhs = to_render_vec(a.cross(b));
        let rhs = to_render_vec(a).cross(to_render_vec(b));
        assert!(close(lhs, rhs));
        assert_eq!(a.dot(b), to_render_vec(a).dot(to_render_vec(b)));
    }

    #[test]
    fn round_trip_is_exact() {
        let v = DVec3::new(123_456.789, -98.5, 7.25);
        assert_eq!(from_render_vec(to_render_vec(v)), v);
    }

    #[test]
    fn the_quaternion_form_matches_the_vector_form() {
        // R as a rotation really is the vector map.
        let v = DVec3::new(0.3, -1.7, 2.9);
        assert!(close(BF_TO_RENDER.rotate(v), to_render_vec(v)));
    }

    #[test]
    fn conjugated_rotation_commutes_with_the_mapping() {
        let q = DQuat::from_axis_angle(DVec3::new(2.0, -1.0, 2.0) * (1.0 / 3.0), 0.9);
        for v in [DVec3::X, DVec3::Y, DVec3::Z, DVec3::new(10.0, -20.0, 30.0)] {
            let via_bf = to_render_vec(q.rotate(v));
            let via_render = to_render_quat(q).rotate(to_render_vec(v));
            assert!(close(via_bf, via_render), "{via_bf:?} vs {via_render:?}");
        }
    }

    #[test]
    fn a_heading_change_about_bf_up_is_a_turn_about_render_up() {
        // Yawing 90 degrees about BF Up takes East to North; in render axes that is +X to -Z.
        let q = DQuat::from_axis_angle(DVec3::Z, core::f64::consts::FRAC_PI_2);
        let east = to_render_vec(DVec3::X);
        assert!(close(to_render_quat(q).rotate(east), -DVec3::Z));
    }
}
