//! Attitude in the local level frame.
//!
//! BF is East-North-Up and curved (ADR-04): "level" means perpendicular to the local up vector,
//! which tilts away from BF `+z` with distance from the origin. Attitudes are body (FRD) to BF
//! quaternions; heading, pitch and roll are the usual aerospace angles (yaw, then pitch, then
//! roll) relative to the local level frame, whose north is BF north carried along the shortest
//! rotation from `+z` to local up.

use simmath::dmath;
use simmath::math::{DQuat, DVec3};

/// NED to ENU: a half turn about `(1, 1, 0) / sqrt(2)` sends north to `+y`, east to `+x` and down
/// to `-z`.
const NED_TO_ENU: DQuat = DQuat::new(
    0.0,
    core::f64::consts::FRAC_1_SQRT_2,
    core::f64::consts::FRAC_1_SQRT_2,
    0.0,
);

/// Heading, pitch and roll, rad.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Euler {
    /// Heading: 0 north, positive towards east (nose right).
    pub heading_rad: f64,
    /// Pitch: positive nose up.
    pub pitch_rad: f64,
    /// Roll (bank): positive right wing down.
    pub roll_rad: f64,
}

/// The shortest rotation taking BF `+z` to `up` (unit length).
fn tilt(up: DVec3) -> DQuat {
    let c = DVec3::Z.cross(up);
    DQuat::new(1.0 + DVec3::Z.dot(up), c.x, c.y, c.z).normalized()
}

/// Local level frame (as a rotation from local NED to BF) at a point with local up `up`.
fn local_ned_to_bf(up: DVec3) -> DQuat {
    tilt(up) * NED_TO_ENU
}

/// The body-to-BF attitude for Euler angles in the local level frame with up vector `up`.
#[must_use]
pub fn attitude(up: DVec3, e: Euler) -> DQuat {
    let half = |a: f64| (dmath::cos(0.5 * a), dmath::sin(0.5 * a));
    let (cy, sy) = half(e.heading_rad);
    let (cp, sp) = half(e.pitch_rad);
    let (cr, sr) = half(e.roll_rad);
    // Yaw about down, then pitch about the new right, then roll about the new forward.
    let body_to_ned = DQuat::new(
        cr * cp * cy + sr * sp * sy,
        sr * cp * cy - cr * sp * sy,
        cr * sp * cy + sr * cp * sy,
        cr * cp * sy - sr * sp * cy,
    );
    (local_ned_to_bf(up) * body_to_ned).normalized()
}

/// Euler angles of a body-to-BF attitude in the local level frame with up vector `up`. Pitch is
/// in `[-pi/2, pi/2]`, heading and roll in `(-pi, pi]`.
#[must_use]
pub fn euler(up: DVec3, q_body_to_bf: DQuat) -> Euler {
    let q = local_ned_to_bf(up).conjugate() * q_body_to_bf;
    let (w, x, y, z) = (q.w, q.x, q.y, q.z);
    Euler {
        heading_rad: dmath::atan2(2.0 * (w * z + x * y), 1.0 - 2.0 * (y * y + z * z)),
        pitch_rad: dmath::asin((2.0 * (w * y - z * x)).clamp(-1.0, 1.0)),
        roll_rad: dmath::atan2(2.0 * (w * x + y * z), 1.0 - 2.0 * (x * x + y * y)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: DVec3, b: DVec3) -> bool {
        (a - b).length() < 1e-12
    }

    #[test]
    fn level_flight_heading_north_matches_the_observer_convention() {
        let q = attitude(DVec3::Z, Euler::default());
        assert!(close(q.rotate(DVec3::X), DVec3::Y), "nose north");
        assert!(close(q.rotate(DVec3::Y), DVec3::X), "right wing east");
        assert!(close(q.rotate(DVec3::Z), -DVec3::Z), "belly down");
    }

    #[test]
    fn angles_have_the_aerospace_signs() {
        let up = DVec3::Z;
        let east = attitude(
            up,
            Euler {
                heading_rad: core::f64::consts::FRAC_PI_2,
                ..Euler::default()
            },
        );
        assert!(
            close(east.rotate(DVec3::X), DVec3::X),
            "heading 90: nose east"
        );
        let climb = attitude(
            up,
            Euler {
                pitch_rad: 0.3,
                ..Euler::default()
            },
        );
        assert!(
            (climb.rotate(DVec3::X).z - dmath::sin(0.3)).abs() < 1e-15,
            "nose up"
        );
        let bank = attitude(
            up,
            Euler {
                roll_rad: 0.4,
                ..Euler::default()
            },
        );
        assert!(bank.rotate(DVec3::Y).z < 0.0, "right wing down");
    }

    #[test]
    fn euler_angles_round_trip_on_a_tilted_frame() {
        let up = DVec3::new(0.3, -0.2, 1.0).normalized_or(DVec3::Z);
        for &(h, p, r) in &[
            (0.0, 0.0, 0.0),
            (1.0, 0.2, -0.5),
            (-2.5, -1.2, 3.0),
            (3.0, 0.7, 1.4),
        ] {
            let e = Euler {
                heading_rad: h,
                pitch_rad: p,
                roll_rad: r,
            };
            let q = attitude(up, e);
            let back = euler(up, q);
            assert!((back.heading_rad - h).abs() < 1e-12, "{back:?} vs {e:?}");
            assert!((back.pitch_rad - p).abs() < 1e-12, "{back:?} vs {e:?}");
            assert!((back.roll_rad - r).abs() < 1e-12, "{back:?} vs {e:?}");
            // Level means perpendicular to the local up.
            let level = attitude(
                up,
                Euler {
                    heading_rad: h,
                    ..Euler::default()
                },
            );
            assert!(level.rotate(DVec3::X).dot(up).abs() < 1e-15);
            assert!(close(level.rotate(DVec3::Z), -up));
        }
    }
}
