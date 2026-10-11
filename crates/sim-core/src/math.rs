//! Small `f64` vector and quaternion types (GDD 0.3.3).
//!
//! These stand in for `glam`'s `DVec3`/`DQuat`. `glam` pulls in a `libm` feature set that would
//! let transcendentals use target intrinsics, which breaks cross-architecture bit-exactness
//! (ADR-10, ADR-27). Everything here is `+ - * /`, `sqrt` and the software `dmath` functions.
//!
//! Quaternions are `(w, x, y, z)`, Hamilton convention, and are renormalised after every
//! integration step; the tolerance is `| |q| - 1 | < 1e-12`.

use core::ops::{Add, Mul, Neg, Sub};

use crate::dmath;

/// A 3-vector of `f64`. The frame is named by the variable holding it (`pos_bf`, `vel_body`, ...).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DVec3 {
    /// X component.
    pub x: f64,
    /// Y component.
    pub y: f64,
    /// Z component.
    pub z: f64,
}

impl DVec3 {
    /// The zero vector.
    pub const ZERO: Self = Self::new(0.0, 0.0, 0.0);
    /// Unit X.
    pub const X: Self = Self::new(1.0, 0.0, 0.0);
    /// Unit Y.
    pub const Y: Self = Self::new(0.0, 1.0, 0.0);
    /// Unit Z.
    pub const Z: Self = Self::new(0.0, 0.0, 1.0);

    /// Builds a vector.
    #[must_use]
    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    /// Dot product.
    #[must_use]
    pub fn dot(self, o: Self) -> f64 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }

    /// Cross product, right-handed.
    #[must_use]
    pub fn cross(self, o: Self) -> Self {
        Self::new(
            self.y * o.z - self.z * o.y,
            self.z * o.x - self.x * o.z,
            self.x * o.y - self.y * o.x,
        )
    }

    /// Squared length.
    #[must_use]
    pub fn length_squared(self) -> f64 {
        self.dot(self)
    }

    /// Length (IEEE-exact `sqrt`).
    #[must_use]
    pub fn length(self) -> f64 {
        dmath::sqrt(self.length_squared())
    }

    /// Unit vector in the same direction, or `fallback` if the length is zero or not finite.
    #[must_use]
    pub fn normalized_or(self, fallback: Self) -> Self {
        let len = self.length();
        if len > 0.0 && len.is_finite() {
            self * (1.0 / len)
        } else {
            fallback
        }
    }

    /// True if every component is finite.
    #[must_use]
    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite() && self.z.is_finite()
    }

    /// Components as an array, in `x, y, z` order.
    #[must_use]
    pub const fn to_array(self) -> [f64; 3] {
        [self.x, self.y, self.z]
    }
}

impl Add for DVec3 {
    type Output = Self;
    fn add(self, o: Self) -> Self {
        Self::new(self.x + o.x, self.y + o.y, self.z + o.z)
    }
}

impl Sub for DVec3 {
    type Output = Self;
    fn sub(self, o: Self) -> Self {
        Self::new(self.x - o.x, self.y - o.y, self.z - o.z)
    }
}

impl Neg for DVec3 {
    type Output = Self;
    fn neg(self) -> Self {
        Self::new(-self.x, -self.y, -self.z)
    }
}

impl Mul<f64> for DVec3 {
    type Output = Self;
    fn mul(self, s: f64) -> Self {
        Self::new(self.x * s, self.y * s, self.z * s)
    }
}

/// A rotation as a quaternion `(w, x, y, z)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DQuat {
    /// Scalar part.
    pub w: f64,
    /// X of the vector part.
    pub x: f64,
    /// Y of the vector part.
    pub y: f64,
    /// Z of the vector part.
    pub z: f64,
}

impl DQuat {
    /// No rotation.
    pub const IDENTITY: Self = Self::new(1.0, 0.0, 0.0, 0.0);

    /// Builds a quaternion from components (not normalised).
    #[must_use]
    pub const fn new(w: f64, x: f64, y: f64, z: f64) -> Self {
        Self { w, x, y, z }
    }

    /// Rotation of `angle_rad` about a unit `axis`.
    #[must_use]
    pub fn from_axis_angle(axis: DVec3, angle_rad: f64) -> Self {
        let half = 0.5 * angle_rad;
        let s = dmath::sin(half);
        Self::new(dmath::cos(half), axis.x * s, axis.y * s, axis.z * s)
    }

    /// Conjugate; the inverse for a unit quaternion.
    #[must_use]
    pub const fn conjugate(self) -> Self {
        Self::new(self.w, -self.x, -self.y, -self.z)
    }

    /// Squared norm.
    #[must_use]
    pub fn norm_squared(self) -> f64 {
        self.w * self.w + self.x * self.x + self.y * self.y + self.z * self.z
    }

    /// Unit quaternion in the same direction, or identity if the norm is zero or not finite.
    #[must_use]
    pub fn normalized(self) -> Self {
        let n = dmath::sqrt(self.norm_squared());
        if n > 0.0 && n.is_finite() {
            let inv = 1.0 / n;
            Self::new(self.w * inv, self.x * inv, self.y * inv, self.z * inv)
        } else {
            Self::IDENTITY
        }
    }

    /// True if every component is finite.
    #[must_use]
    pub fn is_finite(self) -> bool {
        self.w.is_finite() && self.x.is_finite() && self.y.is_finite() && self.z.is_finite()
    }

    /// Rotates `v` by this (unit) quaternion: `q v q*`.
    #[must_use]
    pub fn rotate(self, v: DVec3) -> DVec3 {
        let u = DVec3::new(self.x, self.y, self.z);
        let t = u.cross(v) * 2.0;
        v + t * self.w + u.cross(t)
    }

    /// One explicit step of `q' = q + 1/2 q (0, w_body) dt`, renormalised.
    ///
    /// `w_body` is the body-frame angular rate in rad/s. For the small steps used per tick the
    /// first-order update followed by renormalisation keeps the norm error far below `1e-12`.
    #[must_use]
    pub fn integrate_body_rate(self, w_body: DVec3, dt: f64) -> Self {
        let h = 0.5 * dt;
        let dw = -self.x * w_body.x - self.y * w_body.y - self.z * w_body.z;
        let dx = self.w * w_body.x + self.y * w_body.z - self.z * w_body.y;
        let dy = self.w * w_body.y + self.z * w_body.x - self.x * w_body.z;
        let dz = self.w * w_body.z + self.x * w_body.y - self.y * w_body.x;
        Self::new(
            self.w + h * dw,
            self.x + h * dx,
            self.y + h * dy,
            self.z + h * dz,
        )
        .normalized()
    }

    /// Components as `[w, x, y, z]`.
    #[must_use]
    pub const fn to_array(self) -> [f64; 4] {
        [self.w, self.x, self.y, self.z]
    }
}

impl Mul for DQuat {
    type Output = Self;
    /// Hamilton product: `self` applied after `rhs`.
    fn mul(self, o: Self) -> Self {
        Self::new(
            self.w * o.w - self.x * o.x - self.y * o.y - self.z * o.z,
            self.w * o.x + self.x * o.w + self.y * o.z - self.z * o.y,
            self.w * o.y - self.x * o.z + self.y * o.w + self.z * o.x,
            self.w * o.z + self.x * o.y - self.y * o.x + self.z * o.w,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::f64::consts::FRAC_PI_2;

    fn close(a: DVec3, b: DVec3, tol: f64) -> bool {
        (a - b).length() <= tol
    }

    #[test]
    fn cross_of_axes_is_right_handed() {
        assert_eq!(DVec3::X.cross(DVec3::Y), DVec3::Z);
        assert_eq!(DVec3::Y.cross(DVec3::Z), DVec3::X);
        assert_eq!(DVec3::Z.cross(DVec3::X), DVec3::Y);
    }

    #[test]
    fn length_and_normalisation() {
        // 3-4-5 triangle.
        let v = DVec3::new(3.0, 4.0, 0.0);
        assert_eq!(v.length(), 5.0);
        assert!(close(
            v.normalized_or(DVec3::Z),
            DVec3::new(0.6, 0.8, 0.0),
            1e-15
        ));
        assert_eq!(DVec3::ZERO.normalized_or(DVec3::Z), DVec3::Z);
        assert_eq!(
            DVec3::new(f64::NAN, 0.0, 0.0).normalized_or(DVec3::X),
            DVec3::X
        );
    }

    #[test]
    fn quarter_turn_about_z_takes_x_to_y() {
        let q = DQuat::from_axis_angle(DVec3::Z, FRAC_PI_2);
        assert!(close(q.rotate(DVec3::X), DVec3::Y, 1e-15));
        assert!(close(q.rotate(DVec3::Y), -DVec3::X, 1e-15));
        assert!(close(q.rotate(DVec3::Z), DVec3::Z, 1e-15));
    }

    #[test]
    fn product_composes_right_to_left() {
        // Rotate about X by 90 deg, then about Z by 90 deg: Y goes to Z, then Z stays.
        let qx = DQuat::from_axis_angle(DVec3::X, FRAC_PI_2);
        let qz = DQuat::from_axis_angle(DVec3::Z, FRAC_PI_2);
        let both = qz * qx;
        assert!(close(both.rotate(DVec3::Y), DVec3::Z, 1e-15));
        assert!(close(
            both.rotate(DVec3::X),
            qz.rotate(qx.rotate(DVec3::X)),
            1e-15
        ));
    }

    #[test]
    fn conjugate_undoes_rotation() {
        let q = DQuat::from_axis_angle(DVec3::new(1.0, 2.0, 2.0) * (1.0 / 3.0), 1.1);
        let v = DVec3::new(0.3, -7.0, 2.5);
        assert!(close(q.conjugate().rotate(q.rotate(v)), v, 1e-14));
    }

    #[test]
    fn integration_keeps_the_norm_and_turns_at_the_commanded_rate() {
        // 1 rad/s about body X for one second in 60 Hz steps is a one radian roll.
        let mut q = DQuat::IDENTITY;
        for _ in 0..60 {
            q = q.integrate_body_rate(DVec3::new(1.0, 0.0, 0.0), 1.0 / 60.0);
            assert!((q.norm_squared() - 1.0).abs() < 1e-12);
        }
        let expected = DQuat::from_axis_angle(DVec3::X, 1.0);
        let probe = DVec3::new(0.0, 1.0, 0.0);
        assert!(close(q.rotate(probe), expected.rotate(probe), 2e-4));
    }

    #[test]
    fn non_finite_quaternion_normalises_to_identity() {
        let q = DQuat::new(f64::NAN, 0.0, 0.0, 0.0).normalized();
        assert_eq!(q, DQuat::IDENTITY);
        assert!(!DQuat::new(f64::INFINITY, 0.0, 0.0, 0.0).is_finite());
    }
}
