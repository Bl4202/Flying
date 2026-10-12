//! `AeroModel`: forces and moments on an airframe (GDD 11.6, 61.5). The first implementations are
//! a null model and a linear lift/drag model in body axes; the table-driven model with the
//! Kestrel's three tables replaces them in M1 (v0.1a).

use crate::math::DVec3;

/// Everything an aero model may depend on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AeroState {
    /// Angle of attack, radians.
    pub alpha_rad: f64,
    /// Sideslip, radians.
    pub beta_rad: f64,
    /// Mach number.
    pub mach: f64,
    /// Dynamic pressure, pascals.
    pub dynamic_pressure_pa: f64,
    /// Body angular rates (roll, pitch, yaw), rad/s.
    pub rates_body_rps: DVec3,
    /// Control surface positions, each in `[-1, 1]`: elevator, aileron, rudder, flap.
    pub surfaces: [f64; 4],
}

/// Aerodynamic force and moment in body axes (FRD).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AeroOutput {
    /// Force, newtons.
    pub force_body_n: DVec3,
    /// Moment about the reference point, newton-metres.
    pub moment_body_nm: DVec3,
}

impl AeroOutput {
    /// No force, no moment.
    pub const ZERO: Self = Self {
        force_body_n: DVec3::ZERO,
        moment_body_nm: DVec3::ZERO,
    };
}

/// Aerodynamics of one airframe.
pub trait AeroModel: Send {
    /// Force and moment for a flight condition. Must be finite for finite input.
    fn evaluate(&self, state: &AeroState) -> AeroOutput;
}

/// No aerodynamics at all.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoAero;

impl AeroModel for NoAero {
    fn evaluate(&self, _state: &AeroState) -> AeroOutput {
        AeroOutput::ZERO
    }
}

/// Linear lift, parabolic drag, small-angle body axes: `CL = clamp(CLa * alpha)`,
/// `CD = CD0 + K CL^2`. Lift acts along body -z (up), drag along -x. A placeholder with the right
/// shape, not a flight model.
#[derive(Clone, Copy, Debug)]
pub struct LinearAero {
    /// Reference wing area, square metres.
    pub wing_area_m2: f64,
    /// Lift-curve slope, per radian.
    pub cl_alpha: f64,
    /// Largest lift coefficient magnitude.
    pub cl_max: f64,
    /// Zero-lift drag coefficient.
    pub cd0: f64,
    /// Induced-drag factor.
    pub k: f64,
}

impl LinearAero {
    /// The Kestrel's reference polar (Appendix C.1, C.4): S 28 m2, `CLa` 3.6, CD0 0.0235, K 0.135.
    pub const KESTREL: Self = Self {
        wing_area_m2: 28.0,
        cl_alpha: 3.6,
        cl_max: 1.6,
        cd0: 0.0235,
        k: 0.135,
    };
}

impl AeroModel for LinearAero {
    fn evaluate(&self, s: &AeroState) -> AeroOutput {
        let qs = s.dynamic_pressure_pa.max(0.0) * self.wing_area_m2;
        let cl = (self.cl_alpha * s.alpha_rad).clamp(-self.cl_max, self.cl_max);
        let cd = self.cd0 + self.k * cl * cl;
        AeroOutput {
            force_body_n: DVec3::new(-qs * cd, 0.0, -qs * cl),
            moment_body_nm: DVec3::ZERO,
        }
    }
}

/// Behaviour every `AeroModel` must have (`AC-61.5`).
///
/// # Panics
/// Panics, with a message naming the property, if the implementation violates the contract.
pub fn contract(m: &dyn AeroModel) {
    let base = AeroState {
        alpha_rad: 0.1,
        beta_rad: 0.02,
        mach: 0.6,
        dynamic_pressure_pa: 20_000.0,
        rates_body_rps: DVec3::new(0.1, -0.2, 0.05),
        surfaces: [0.1, -0.1, 0.0, 0.0],
    };
    let out = m.evaluate(&base);
    assert!(
        out.force_body_n.is_finite() && out.moment_body_nm.is_finite(),
        "finite output"
    );
    assert_eq!(out, m.evaluate(&base), "a pure function of the state");

    // No airflow, no force: this is what keeps a parked aircraft parked.
    let still = m.evaluate(&AeroState {
        dynamic_pressure_pa: 0.0,
        ..base
    });
    assert_eq!(
        still,
        AeroOutput::ZERO,
        "zero dynamic pressure gives zero force and moment"
    );

    // Wild but finite conditions stay finite, including past the stall.
    for alpha in [-3.1, -1.0, 0.0, 0.5, 1.6, 3.1] {
        for q in [1.0, 5.0e4, 2.0e5] {
            let wild = m.evaluate(&AeroState {
                alpha_rad: alpha,
                beta_rad: -alpha / 2.0,
                mach: 2.4,
                dynamic_pressure_pa: q,
                surfaces: [1.0, -1.0, 1.0, 1.0],
                ..base
            });
            assert!(
                wild.force_body_n.is_finite() && wild.moment_body_nm.is_finite(),
                "finite at alpha {alpha}, q {q}"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(alpha: f64, q: f64) -> AeroState {
        AeroState {
            alpha_rad: alpha,
            beta_rad: 0.0,
            mach: 0.5,
            dynamic_pressure_pa: q,
            rates_body_rps: DVec3::ZERO,
            surfaces: [0.0; 4],
        }
    }

    #[test]
    fn both_models_pass_the_contract() {
        contract(&NoAero);
        contract(&LinearAero::KESTREL);
    }

    #[test]
    fn lift_matches_a_hand_calculation() {
        // q = 17 732 Pa (SL, Mach 0.5, Appendix C.5), alpha = 0.1 rad:
        // CL = 0.36, CD = 0.0235 + 0.135 * 0.1296 = 0.040996, lift = q S CL.
        let out = LinearAero::KESTREL.evaluate(&at(0.1, 17_732.0));
        let qs = 17_732.0 * 28.0;
        assert!(
            (out.force_body_n.z + qs * 0.36).abs() < 1e-6,
            "lift {}",
            out.force_body_n.z
        );
        assert!(
            (out.force_body_n.x + qs * 0.040_996).abs() < 1e-3,
            "drag {}",
            out.force_body_n.x
        );
    }

    #[test]
    fn lift_saturates_at_the_stall() {
        let m = LinearAero::KESTREL;
        let a = m.evaluate(&at(0.6, 10_000.0));
        let b = m.evaluate(&at(1.2, 10_000.0));
        assert_eq!(a.force_body_n.z, b.force_body_n.z, "CL is capped at CLmax");
    }
}
