//! The 6-DOF rigid body and its RK4 integrator (GDD 11.2.2, 11.10).
//!
//! ```text
//! p' = v
//! v' = R(q) F_body / m + g(p)
//! q' = 1/2 q (0, w)
//! I w' = M_body - w x (I w)
//! ```
//!
//! `p` is the centre of gravity in BF, `v` its velocity, `q` the body-to-BF attitude and `w` the
//! body rates. `F_body` is every non-gravitational force and `M_body` the moment about the CG.
//! Mass and inertia are held over a step (they change only with fuel, between steps).

use simmath::math::{DQuat, DVec3};

use crate::mass::MassProps;

/// The integrated state: 13 numbers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RigidState {
    /// Centre of gravity, BF, m.
    pub pos_bf: DVec3,
    /// Velocity of the CG, BF, m/s.
    pub vel_bf: DVec3,
    /// Attitude, body (FRD) to BF.
    pub att: DQuat,
    /// Body rates (roll, pitch, yaw), rad/s.
    pub omega_body: DVec3,
}

/// The time derivative of a [`RigidState`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RigidDeriv {
    /// Velocity, m/s.
    pub pos: DVec3,
    /// Acceleration, m/s^2.
    pub vel: DVec3,
    /// Quaternion rate.
    pub att: DQuat,
    /// Angular acceleration, rad/s^2.
    pub omega: DVec3,
}

impl RigidState {
    /// `self + d * h`, component-wise (the attitude is not renormalised).
    #[must_use]
    pub fn add_scaled(&self, d: &RigidDeriv, h: f64) -> Self {
        Self {
            pos_bf: self.pos_bf + d.pos * h,
            vel_bf: self.vel_bf + d.vel * h,
            att: self.att.add_scaled(d.att, h),
            omega_body: self.omega_body + d.omega * h,
        }
    }

    /// True if every component is finite.
    #[must_use]
    pub fn is_finite(&self) -> bool {
        self.pos_bf.is_finite()
            && self.vel_bf.is_finite()
            && self.att.is_finite()
            && self.omega_body.is_finite()
    }
}

/// The equations of motion: the derivative of `s` under a body force and a moment about the CG,
/// gravity in BF and the mass properties.
#[must_use]
pub fn derivative(
    s: &RigidState,
    force_body_n: DVec3,
    moment_body_nm: DVec3,
    gravity_bf: DVec3,
    mass: &MassProps,
) -> RigidDeriv {
    let i_omega = mass.inertia_kgm2.mul_vec(s.omega_body);
    RigidDeriv {
        pos: s.vel_bf,
        vel: s.att.rotate(force_body_n) / mass.mass_kg + gravity_bf,
        att: s.att.derivative(s.omega_body),
        omega: mass
            .inertia_inv
            .mul_vec(moment_body_nm - s.omega_body.cross(i_omega)),
    }
}

/// One classical RK4 step of length `dt` (GDD 11.10). `deriv` evaluates the derivative at a stage
/// state; it is called four times, in order. The attitude is renormalised at the end.
#[must_use]
pub fn step_rk4(
    s: &RigidState,
    dt: f64,
    mut deriv: impl FnMut(&RigidState) -> RigidDeriv,
) -> RigidState {
    // ALGO: classical RK4 weights 1/6, 1/3, 1/3, 1/6.
    let k1 = deriv(s);
    let k2 = deriv(&s.add_scaled(&k1, 0.5 * dt));
    let k3 = deriv(&s.add_scaled(&k2, 0.5 * dt));
    let k4 = deriv(&s.add_scaled(&k3, dt));
    let w = dt / 6.0;
    let mut n = s
        .add_scaled(&k1, w)
        .add_scaled(&k2, 2.0 * w)
        .add_scaled(&k3, 2.0 * w)
        .add_scaled(&k4, w);
    n.att = n.att.normalized();
    n
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::energy::specific_energy_m;
    use crate::env::{FlightEnv, SphereSea};
    use crate::physical::G0;
    use simmath::math::DMat3;

    fn props(mass_kg: f64, ixx: f64, iyy: f64, izz: f64, ixz: f64) -> MassProps {
        let inertia = DMat3::from_inertia(ixx, iyy, izz, ixz);
        MassProps {
            mass_kg,
            cg_body_m: DVec3::ZERO,
            inertia_kgm2: inertia,
            inertia_inv: inertia.inverse().unwrap(),
        }
    }

    /// The C.2 Kestrel at 10 000 kg.
    fn kestrel() -> MassProps {
        props(10_000.0, 13_000.0, 76_000.0, 86_000.0, 1300.0)
    }

    fn spinning() -> RigidState {
        RigidState {
            pos_bf: DVec3::ZERO,
            vel_bf: DVec3::ZERO,
            att: DQuat::IDENTITY,
            omega_body: DVec3::new(1.2, 0.3, -0.4),
        }
    }

    fn torque_free(m: &MassProps) -> impl FnMut(&RigidState) -> RigidDeriv + '_ {
        move |s| derivative(s, DVec3::ZERO, DVec3::ZERO, DVec3::ZERO, m)
    }

    #[test]
    fn ac_11_6_the_quaternion_stays_unit_over_a_million_steps() {
        let m = kestrel();
        let mut s = spinning();
        let mut worst = 0.0_f64;
        for _ in 0..1_000_000 {
            s = step_rk4(&s, 1.0 / 120.0, torque_free(&m));
            worst = worst.max((s.att.norm_squared().sqrt() - 1.0).abs());
        }
        assert!(worst < 1e-12, "norm error {worst}");
    }

    #[test]
    fn a_torque_free_spin_conserves_angular_momentum_and_energy() {
        // With I_xz the rates couple, but H = R I w (in BF) and T = w.I w / 2 are invariants.
        let m = kestrel();
        let mut s = spinning();
        let h = |s: &RigidState| s.att.rotate(m.inertia_kgm2.mul_vec(s.omega_body));
        let t = |s: &RigidState| 0.5 * s.omega_body.dot(m.inertia_kgm2.mul_vec(s.omega_body));
        let (h0, t0) = (h(&s), t(&s));
        for _ in 0..1200 {
            s = step_rk4(&s, 1.0 / 120.0, torque_free(&m));
        }
        let dh = (h(&s) - h0).length() / h0.length();
        let dt = (t(&s) - t0).abs() / t0;
        assert!(dh < 1e-9, "angular momentum drift {dh}");
        assert!(dt < 1e-9, "energy drift {dt}");
        assert!(
            (s.omega_body - spinning().omega_body).length() > 0.01,
            "the rates did couple"
        );
    }

    #[test]
    fn rk4_is_fourth_order() {
        // A coupled rotation and a gravity drop: halving the step cuts the error about 16x.
        let m = kestrel();
        let run = |dt: f64| {
            let mut s = RigidState {
                vel_bf: DVec3::new(200.0, 0.0, 0.0),
                ..spinning()
            };
            let n = (2.0 / dt).round() as usize;
            for _ in 0..n {
                s = step_rk4(&s, dt, |st| {
                    // A body-fixed thrust makes position depend on the rotation.
                    derivative(
                        st,
                        DVec3::new(5e4, 0.0, 0.0),
                        DVec3::new(0.0, 2e3, 0.0),
                        DVec3::new(0.0, 0.0, -G0),
                        &m,
                    )
                });
            }
            s
        };
        let reference = run(1.0 / 1920.0);
        let err = |dt: f64| (run(dt).pos_bf - reference.pos_bf).length();
        let (e1, e2) = (err(1.0 / 60.0), err(1.0 / 120.0));
        let ratio = e1 / e2;
        assert!(
            (12.0..20.0).contains(&ratio),
            "error ratio {ratio} ({e1} / {e2})"
        );
    }

    #[test]
    fn ac_11_4_a_drag_free_zoom_conserves_energy_height() {
        // Lift perpendicular to the velocity does no work: E_s = h + V^2 / 2g stays put (0.5 %)
        // through a 30 s pull-up and zoom. Point-mass lift pitched up at 0.15 rad/s.
        let world = SphereSea::default();
        let m = kestrel();
        let mut s = RigidState {
            pos_bf: DVec3::new(0.0, 0.0, 3000.0),
            vel_bf: DVec3::new(0.0, 250.0, 0.0),
            att: DQuat::IDENTITY,
            omega_body: DVec3::ZERO,
        };
        let energy =
            |s: &RigidState| specific_energy_m(world.height_msl_m(s.pos_bf), s.vel_bf.length());
        let e0 = energy(&s);
        let mut worst = 0.0_f64;
        for _ in 0..(30 * 120) {
            s = step_rk4(&s, 1.0 / 120.0, |st| {
                let v = st.vel_bf;
                let side = v.cross(world.up_bf(st.pos_bf)).normalized_or(DVec3::X);
                let lift_dir = side.cross(v).normalized_or(DVec3::Z);
                // Enough lift for 0.15 rad/s of path rotation plus gravity's normal part.
                let g = world.gravity_bf(st.pos_bf);
                let a_n = 0.15 * v.length() - g.dot(lift_dir);
                let force_bf = lift_dir * (a_n * m.mass_kg);
                derivative(st, st.att.conjugate().rotate(force_bf), DVec3::ZERO, g, &m)
            });
            worst = worst.max((energy(&s) - e0).abs() / e0);
        }
        // Gravity falls off with height, so g0-based E_s drifts by about h dg/g: 0.1 % here.
        assert!(worst < 0.005, "E_s drift {worst}");
        assert!(world.height_msl_m(s.pos_bf) > 4000.0, "it did zoom");
    }

    #[test]
    fn free_fall_follows_local_up_on_the_sphere() {
        let world = SphereSea::default();
        let m = kestrel();
        let p = DVec3::new(250_000.0, -80_000.0, 6000.0);
        let s = RigidState {
            pos_bf: p,
            vel_bf: DVec3::ZERO,
            att: DQuat::IDENTITY,
            omega_body: DVec3::ZERO,
        };
        let d = derivative(&s, DVec3::ZERO, DVec3::ZERO, world.gravity_bf(p), &m);
        let down = -world.up_bf(p);
        assert!((d.vel.normalized_or(DVec3::Z) - down).length() < 1e-12);
        assert!(down.z > -0.9995, "the tilt is real 250 km out");
    }
}
