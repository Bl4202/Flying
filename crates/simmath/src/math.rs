//! Small `f64` vector, quaternion and 3x3 matrix types (GDD 0.3.3).
//!
//! These stand in for `glam`'s `DVec3`/`DQuat`/`DMat3`. `glam` pulls in a `libm` feature set that would
//! let transcendentals use target intrinsics, which breaks cross-architecture bit-exactness
//! (ADR-10, ADR-27). Everything here is `+ - * /`, `sqrt` and the software `dmath` functions.
//!
//! Quaternions are `(w, x, y, z)`, Hamilton convention, and are renormalised after every
//! integration step; the tolerance is `| |q| - 1 | < 1e-12`.

use core::ops::{Add, AddAssign, Div, Mul, MulAssign, Neg, Sub, SubAssign};

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

    /// Linear interpolation: `self` at `t = 0`, `o` at `t = 1`.
    #[must_use]
    pub fn lerp(self, o: Self, t: f64) -> Self {
        self + (o - self) * t
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

impl Mul<DVec3> for f64 {
    type Output = DVec3;
    fn mul(self, v: DVec3) -> DVec3 {
        v * self
    }
}

impl Div<f64> for DVec3 {
    type Output = Self;
    /// Component-wise division (a true division, not a multiplication by the reciprocal).
    fn div(self, s: f64) -> Self {
        Self::new(self.x / s, self.y / s, self.z / s)
    }
}

impl AddAssign for DVec3 {
    fn add_assign(&mut self, o: Self) {
        *self = *self + o;
    }
}

impl SubAssign for DVec3 {
    fn sub_assign(&mut self, o: Self) {
        *self = *self - o;
    }
}

impl MulAssign<f64> for DVec3 {
    fn mul_assign(&mut self, s: f64) {
        *self = *self * s;
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

    /// Time derivative `q' = 1/2 q (0, w_body)` for a body-frame angular rate in rad/s.
    ///
    /// Used inside higher-order integrators (RK4); the result is not a rotation.
    #[must_use]
    pub fn derivative(self, w_body: DVec3) -> Self {
        Self::new(
            0.5 * (-self.x * w_body.x - self.y * w_body.y - self.z * w_body.z),
            0.5 * (self.w * w_body.x + self.y * w_body.z - self.z * w_body.y),
            0.5 * (self.w * w_body.y + self.z * w_body.x - self.x * w_body.z),
            0.5 * (self.w * w_body.z + self.x * w_body.y - self.y * w_body.x),
        )
    }

    /// `self + d * s`, component-wise (not normalised).
    #[must_use]
    pub fn add_scaled(self, d: Self, s: f64) -> Self {
        Self::new(
            self.w + d.w * s,
            self.x + d.x * s,
            self.y + d.y * s,
            self.z + d.z * s,
        )
    }

    /// The rotation matrix of this (unit) quaternion: `to_mat3() * v == rotate(v)`.
    #[must_use]
    pub fn to_mat3(self) -> DMat3 {
        let (w, x, y, z) = (self.w, self.x, self.y, self.z);
        DMat3::from_rows([
            [
                1.0 - 2.0 * (y * y + z * z),
                2.0 * (x * y - w * z),
                2.0 * (x * z + w * y),
            ],
            [
                2.0 * (x * y + w * z),
                1.0 - 2.0 * (x * x + z * z),
                2.0 * (y * z - w * x),
            ],
            [
                2.0 * (x * z - w * y),
                2.0 * (y * z + w * x),
                1.0 - 2.0 * (x * x + y * y),
            ],
        ])
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

/// A 3x3 matrix of `f64`, row-major: `m[row][col]`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DMat3 {
    /// Rows.
    pub m: [[f64; 3]; 3],
}

impl Default for DMat3 {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl DMat3 {
    /// The identity matrix.
    pub const IDENTITY: Self = Self::from_rows([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]);
    /// The zero matrix.
    pub const ZERO: Self = Self::from_rows([[0.0; 3]; 3]);

    /// Builds a matrix from its rows.
    #[must_use]
    pub const fn from_rows(m: [[f64; 3]; 3]) -> Self {
        Self { m }
    }

    /// A diagonal matrix.
    #[must_use]
    pub const fn diagonal(d: DVec3) -> Self {
        Self::from_rows([[d.x, 0.0, 0.0], [0.0, d.y, 0.0], [0.0, 0.0, d.z]])
    }

    /// The inertia tensor of an aircraft with a plane of symmetry, in body axes (FRD).
    ///
    /// `ixz` is the product of inertia `integral(x z dm)`; it enters the tensor with a minus sign,
    /// the aerospace convention (GDD 11.9, Appendix C.2).
    #[must_use]
    pub const fn from_inertia(ixx: f64, iyy: f64, izz: f64, ixz: f64) -> Self {
        Self::from_rows([[ixx, 0.0, -ixz], [0.0, iyy, 0.0], [-ixz, 0.0, izz]])
    }

    /// The outer product `a b^T`.
    #[must_use]
    pub fn outer(a: DVec3, b: DVec3) -> Self {
        Self::from_rows([
            [a.x * b.x, a.x * b.y, a.x * b.z],
            [a.y * b.x, a.y * b.y, a.y * b.z],
            [a.z * b.x, a.z * b.y, a.z * b.z],
        ])
    }

    /// Row `i` as a vector.
    #[must_use]
    pub const fn row(self, i: usize) -> DVec3 {
        DVec3::new(self.m[i][0], self.m[i][1], self.m[i][2])
    }

    /// Matrix times column vector.
    #[must_use]
    pub fn mul_vec(self, v: DVec3) -> DVec3 {
        DVec3::new(self.row(0).dot(v), self.row(1).dot(v), self.row(2).dot(v))
    }

    /// Transpose times column vector, `self^T v`, without forming the transpose.
    #[must_use]
    pub fn transpose_mul_vec(self, v: DVec3) -> DVec3 {
        let m = &self.m;
        DVec3::new(
            m[0][0] * v.x + m[1][0] * v.y + m[2][0] * v.z,
            m[0][1] * v.x + m[1][1] * v.y + m[2][1] * v.z,
            m[0][2] * v.x + m[1][2] * v.y + m[2][2] * v.z,
        )
    }

    /// The transpose.
    #[must_use]
    pub fn transpose(self) -> Self {
        let m = &self.m;
        Self::from_rows([
            [m[0][0], m[1][0], m[2][0]],
            [m[0][1], m[1][1], m[2][1]],
            [m[0][2], m[1][2], m[2][2]],
        ])
    }

    /// The determinant.
    #[must_use]
    pub fn determinant(self) -> f64 {
        self.row(0).dot(self.row(1).cross(self.row(2)))
    }

    /// The inverse, or `None` if the matrix is singular or not finite.
    ///
    /// Computed from the adjugate (cofactors are the cross products of the rows), so the result
    /// depends only on `+ - * /` and is bit-identical on every target.
    #[must_use]
    pub fn inverse(self) -> Option<Self> {
        let (r0, r1, r2) = (self.row(0), self.row(1), self.row(2));
        let c0 = r1.cross(r2);
        let c1 = r2.cross(r0);
        let c2 = r0.cross(r1);
        let det = r0.dot(c0);
        if det == 0.0 || !det.is_finite() {
            return None;
        }
        // The columns of the inverse are the cofactor vectors divided by the determinant.
        let inv = Self::from_rows([
            [c0.x / det, c1.x / det, c2.x / det],
            [c0.y / det, c1.y / det, c2.y / det],
            [c0.z / det, c1.z / det, c2.z / det],
        ]);
        inv.is_finite().then_some(inv)
    }

    /// True if every element is finite.
    #[must_use]
    pub fn is_finite(self) -> bool {
        self.m.iter().flatten().all(|v| v.is_finite())
    }
}

impl Add for DMat3 {
    type Output = Self;
    fn add(self, o: Self) -> Self {
        let mut r = self;
        for (row, orow) in r.m.iter_mut().zip(o.m.iter()) {
            for (v, ov) in row.iter_mut().zip(orow.iter()) {
                *v += ov;
            }
        }
        r
    }
}

impl Sub for DMat3 {
    type Output = Self;
    fn sub(self, o: Self) -> Self {
        let mut r = self;
        for (row, orow) in r.m.iter_mut().zip(o.m.iter()) {
            for (v, ov) in row.iter_mut().zip(orow.iter()) {
                *v -= ov;
            }
        }
        r
    }
}

impl Mul<f64> for DMat3 {
    type Output = Self;
    fn mul(self, s: f64) -> Self {
        let mut r = self;
        for v in r.m.iter_mut().flatten() {
            *v *= s;
        }
        r
    }
}

impl Mul for DMat3 {
    type Output = Self;
    fn mul(self, o: Self) -> Self {
        let mut r = Self::ZERO;
        for i in 0..3 {
            for j in 0..3 {
                r.m[i][j] =
                    self.m[i][0] * o.m[0][j] + self.m[i][1] * o.m[1][j] + self.m[i][2] * o.m[2][j];
            }
        }
        r
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

    #[test]
    fn vector_operators_agree_with_their_definitions() {
        let a = DVec3::new(1.0, -2.0, 4.0);
        let b = DVec3::new(0.5, 3.0, -1.0);
        assert_eq!(2.0 * a, a * 2.0);
        assert_eq!(a / 4.0, DVec3::new(0.25, -0.5, 1.0));
        let mut c = a;
        c += b;
        assert_eq!(c, a + b);
        c -= b;
        assert_eq!(c, a);
        c *= 3.0;
        assert_eq!(c, a * 3.0);
        assert_eq!(a.lerp(b, 0.0), a);
        assert_eq!(a.lerp(b, 1.0), b);
        assert_eq!(a.lerp(b, 0.5), DVec3::new(0.75, 0.5, 1.5));
    }

    #[test]
    fn quaternion_matrix_rotates_like_the_quaternion() {
        let q = DQuat::from_axis_angle(DVec3::new(2.0, -1.0, 2.0) * (1.0 / 3.0), 0.7);
        let r = q.to_mat3();
        for v in [DVec3::X, DVec3::Y, DVec3::Z, DVec3::new(0.3, -7.0, 2.5)] {
            assert!(close(r.mul_vec(v), q.rotate(v), 1e-14));
            // The transpose of a rotation is its inverse.
            assert!(close(r.transpose_mul_vec(q.rotate(v)), v, 1e-14));
        }
        assert!((r.determinant() - 1.0).abs() < 1e-14);
    }

    #[test]
    fn quaternion_derivative_matches_the_first_order_step() {
        let q = DQuat::from_axis_angle(DVec3::Y, 0.4);
        let w = DVec3::new(0.3, -1.2, 0.8);
        let dt = 1e-3;
        let explicit = q.add_scaled(q.derivative(w), dt).normalized();
        let reference = q.integrate_body_rate(w, dt);
        for (a, b) in explicit.to_array().iter().zip(reference.to_array()) {
            assert!((a - b).abs() < 1e-15);
        }
        // A pure rate about body X changes only the w and x parts of the identity.
        let d = DQuat::IDENTITY.derivative(DVec3::new(2.0, 0.0, 0.0));
        assert_eq!(d, DQuat::new(0.0, 1.0, 0.0, 0.0));
    }

    #[test]
    fn inertia_tensor_uses_the_aerospace_sign_and_inverts() {
        // Appendix C.2 at 10 000 kg: Ixx 13 000, Iyy 76 000, Izz 86 000, Ixz 1 300 kg m^2.
        let i = DMat3::from_inertia(13_000.0, 76_000.0, 86_000.0, 1_300.0);
        assert_eq!(i.m[0][2], -1_300.0);
        assert_eq!(i.m[2][0], -1_300.0);
        assert_eq!(i, i.transpose());
        let inv = i.inverse().expect("positive definite");
        let p = i * inv;
        for r in 0..3 {
            for c in 0..3 {
                let expected = if r == c { 1.0 } else { 0.0 };
                assert!((p.m[r][c] - expected).abs() < 1e-15, "I I^-1 [{r}][{c}]");
            }
        }
        // Hand value: det = Iyy (Ixx Izz - Ixz^2).
        let det = 76_000.0 * (13_000.0 * 86_000.0 - 1_300.0 * 1_300.0);
        assert!((i.determinant() - det).abs() / det < 1e-15);
        assert!((inv.m[1][1] - 1.0 / 76_000.0).abs() < 1e-20);
    }

    #[test]
    fn singular_or_non_finite_matrices_have_no_inverse() {
        assert_eq!(DMat3::ZERO.inverse(), None);
        let rank2 = DMat3::from_rows([[1.0, 2.0, 3.0], [2.0, 4.0, 6.0], [0.0, 1.0, 1.0]]);
        assert_eq!(rank2.inverse(), None);
        let nan = DMat3::diagonal(DVec3::new(f64::NAN, 1.0, 1.0));
        assert_eq!(nan.inverse(), None);
        assert_eq!(DMat3::IDENTITY.inverse(), Some(DMat3::IDENTITY));
    }

    #[test]
    fn matrix_arithmetic() {
        let a = DMat3::from_rows([[1.0, 2.0, 0.0], [0.0, 1.0, -1.0], [3.0, 0.0, 2.0]]);
        let v = DVec3::new(1.0, -1.0, 2.0);
        assert_eq!(a.mul_vec(v), DVec3::new(-1.0, -3.0, 7.0));
        assert_eq!(a.transpose_mul_vec(v), a.transpose().mul_vec(v));
        assert_eq!((a * DMat3::IDENTITY), a);
        assert_eq!((a + a) - a, a);
        assert_eq!(a * 2.0, a + a);
        let o = DMat3::outer(DVec3::X, DVec3::new(1.0, 2.0, 3.0));
        assert_eq!(o.row(0), DVec3::new(1.0, 2.0, 3.0));
        assert_eq!(o.row(1), DVec3::ZERO);
    }
}
