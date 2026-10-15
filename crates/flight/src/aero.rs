//! Aerodynamics (GDD 11.6): the `AeroModel` seam and its table-driven implementation.
//!
//! [`TableAero`] builds the coefficients up from an aircraft's tables (GDD 11.6.1):
//!
//! ```text
//! C_L,att = C_L,base(a, M) + dC_L,flap + C_L,de(M) de + dC_L,ground
//! C_D,att = C_D0(M) + dC_D,wave(M) + phi K(M) (C_L,att - C_L0)^2 + dC_D,flap + dC_D,gear + dC_D,brake
//! C_L = (1 - s) C_L,att + s C_D90 sin a cos a          s: the post-stall blend (ADR-30)
//! C_D = (1 - s) C_D,att + s (C_D0 + C_D90 sin^2 a)
//! C_m = (1 - s) C_m0(a, M) - s C_D90 sin a (x_ac,post - x_ref) + C_mq q^ + C_ma. a.^ + C_m,de(M) de
//! C_Y = C_Yb sin b + C_Y,dr dr
//! C_l = C_lb sin b + C_lp p^ + C_lr r^ + C_l,da da + C_l,dr dr
//! C_n = C_nb sin b + C_np p^ + C_nr r^ + C_n,da da + C_n,dr dr
//! ```
//!
//! Notes on the choices (`docs/architecture/flight.md`):
//!
//! - Induced drag uses the total lift including the elevator's, and there is no separate
//!   elevator drag, so trimming does not change drag at a fixed total lift.
//! - The blend `s` is a smoothstep in `|alpha|` that is exactly 0 below `blend_start_deg(M)` and
//!   exactly 1 above `blend_end_deg(M)`; the stall itself is in `C_L,base` (ADR-30).
//! - Control and damping terms are not blended, so the surfaces keep their authority post-stall.
//! - Each surface's deflection is scaled by `eta(|deflection|)` (GDD 11.7).
//! - Sideslip enters as `sin b`, so the linear lateral terms saturate.
//! - Trailing-edge flaps at half travel give the take-off increments and at full travel the
//!   landing increments, linear in between.
//!
//! Forces go from wind to body axes per GDD 11.2.4 (with the side-force term of `Z` corrected to
//! `-Y sin a sin b`, which keeps the side force perpendicular to the airflow), and the moments are
//! transferred from the reference point to the CG: `M_cg = M_ref + (r_ref - r_cg) x F`.

use content::{AeroDef, AircraftDef, Curve, Grid};
use simmath::dmath;
use simmath::math::DVec3;

use crate::actuators::Surfaces;
use crate::airdata::{AirData, MIN_TAS_FOR_ANGLES_MPS};
use crate::error::{ModelError, table};
use crate::table::{Table1D, Table2D};

/// Ground effect is ignored above this many spans (`phi` > 0.9997 there). // ALGO: GDD 11.6.5
pub const GROUND_EFFECT_SPANS: f64 = 4.0;

/// Everything an aero model may depend on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AeroInput {
    /// Air-relative state: airspeed, flow angles, Mach, dynamic pressure, rate non-dimensionals.
    pub air: AirData,
    /// Rate of change of the angle of attack, rad/s.
    pub alpha_dot_rps: f64,
    /// Actual surface positions.
    pub surfaces: Surfaces,
    /// Height of the reference point above the ground, m (for ground effect).
    pub height_agl_m: f64,
    /// Centre of gravity, m from the reference point (body axes).
    pub cg_body_m: DVec3,
}

/// Aerodynamic force and moment in body axes (FRD).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AeroOutput {
    /// Force, N.
    pub force_body_n: DVec3,
    /// Moment about the centre of gravity, N m.
    pub moment_body_nm: DVec3,
}

impl AeroOutput {
    /// No force, no moment.
    pub const ZERO: Self = Self {
        force_body_n: DVec3::ZERO,
        moment_body_nm: DVec3::ZERO,
    };
}

/// Aerodynamics of one airframe (GDD 61.5). The table model is the real implementation; the
/// tick calls it concretely (no `dyn` on the hot path, GDD 61.4.2).
pub trait AeroModel: Send {
    /// Force and moment for a flight condition. Must be finite for finite input, and zero when
    /// the dynamic pressure is zero.
    fn evaluate(&self, input: &AeroInput) -> AeroOutput;
}

/// No aerodynamics at all.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoAero;

impl AeroModel for NoAero {
    fn evaluate(&self, _input: &AeroInput) -> AeroOutput {
        AeroOutput::ZERO
    }
}

/// Linear lift, parabolic drag, small-angle body axes: `C_L = clamp(C_La alpha)`,
/// `C_D = C_D0 + K C_L^2`. Lift acts along body -z, drag along -x, and there are no moments. A
/// placeholder with the right shape for tests that need some aerodynamics, not a flight model.
#[derive(Clone, Copy, Debug)]
pub struct LinearAero {
    /// Reference wing area, m^2.
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

impl AeroModel for LinearAero {
    fn evaluate(&self, input: &AeroInput) -> AeroOutput {
        let qs = input.air.qbar_pa.max(0.0) * self.wing_area_m2;
        if qs == 0.0 {
            return AeroOutput::ZERO;
        }
        let cl = (self.cl_alpha * input.air.alpha_rad).clamp(-self.cl_max, self.cl_max);
        let cd = self.cd0 + self.k * cl * cl;
        AeroOutput {
            force_body_n: DVec3::new(-qs * cd, 0.0, -qs * cl),
            moment_body_nm: DVec3::ZERO,
        }
    }
}

/// Non-dimensional coefficients of one evaluation, about the reference point.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Coefficients {
    /// Lift.
    pub cl: f64,
    /// Drag.
    pub cd: f64,
    /// Side force.
    pub cy: f64,
    /// Rolling moment.
    pub cl_roll: f64,
    /// Pitching moment.
    pub cm: f64,
    /// Yawing moment.
    pub cn: f64,
    /// Post-stall blend weight, 0 (tables) to 1 (flat plate).
    pub blend: f64,
}

/// Lateral and directional derivatives, per radian.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Lateral {
    cy_beta: f64,
    cl_beta: f64,
    cn_beta: f64,
    cl_p: f64,
    cl_r: f64,
    cn_p: f64,
    cn_r: f64,
    cl_da: f64,
    cn_da: f64,
    cl_dr: f64,
    cn_dr: f64,
    cy_dr: f64,
}

/// The table-driven aerodynamic model of one airframe, built once from content.
#[derive(Clone, Debug, PartialEq)]
pub struct TableAero {
    wing_area_m2: f64,
    span_m: f64,
    mac_m: f64,
    ref_point_body_m: DVec3,
    cl_base: Table2D,
    cl_max: Table1D,
    cl_alpha: Table1D,
    cl0: f64,
    cl_de: Table1D,
    cd0: Table1D,
    cd_wave: Table1D,
    k_induced: Table1D,
    cm_0: Table2D,
    cm_de: Table1D,
    cm_q: f64,
    cm_alpha_dot: f64,
    lateral: Lateral,
    blend_start_deg: Table1D,
    blend_end_deg: Table1D,
    cd90: f64,
    /// Flat-plate centre behind the reference point, fraction of MAC.
    plate_arm_mac: f64,
    eta: Table1D,
    gear_cd0: f64,
    brake_cd0: f64,
    flap_to: (f64, f64),
    flap_ld: (f64, f64),
    tef_max_rad: f64,
    ground_effect_k: f64,
    /// Angle of attack of the lift peak of each Mach row, degrees.
    alpha_peak_deg: Table1D,
}

fn curve(name: &'static str, c: &Curve) -> Result<Table1D, ModelError> {
    table(name, Table1D::from_f32(&c.xs, &c.ys))
}

fn grid(name: &'static str, g: &Grid) -> Result<Table2D, ModelError> {
    table(name, Table2D::from_f32(&g.xs, &g.ys, &g.zs))
}

/// The angle of attack of the highest lift in each row of `C_L,base`, as a curve over Mach.
fn lift_peaks(cl_base: &Table2D) -> Result<Table1D, ModelError> {
    let xs = cl_base.xs();
    let peaks: Vec<f64> = (0..cl_base.ys().len())
        .map(|j| {
            let row = cl_base.row(j);
            let mut best = 0;
            for (i, v) in row.iter().enumerate() {
                if *v > row[best] {
                    best = i;
                }
            }
            xs[best]
        })
        .collect();
    table("cl_base peaks", Table1D::new(cl_base.ys(), &peaks))
}

/// `0` below `start`, `1` above `end`, `3t^2 - 2t^3` between (C1 at both ends).
fn smoothstep(start: f64, end: f64, x: f64) -> f64 {
    if x <= start {
        return 0.0;
    }
    if x >= end {
        return 1.0;
    }
    let t = (x - start) / (end - start);
    t * t * (3.0 - 2.0 * t)
}

impl TableAero {
    /// Builds the model from an aircraft and its aero entry.
    ///
    /// # Errors
    /// [`ModelError::Table`] if a table is malformed; [`ModelError::Invalid`] for non-positive
    /// reference geometry.
    pub fn from_defs(ac: &AircraftDef, aero: &AeroDef) -> Result<Self, ModelError> {
        let f = f64::from;
        let g = &ac.geometry;
        if !(g.wing_area_m2 > 0.0 && g.span_m > 0.0 && g.mac_m > 0.0) {
            return Err(ModelError::Invalid("reference geometry"));
        }
        let t = &aero.lateral;
        let c = &aero.config;
        let cl_base = grid("cl_base", &aero.lift.cl_base)?;
        let alpha_peak_deg = lift_peaks(&cl_base)?;
        Ok(Self {
            wing_area_m2: f(g.wing_area_m2),
            span_m: f(g.span_m),
            mac_m: f(g.mac_m),
            ref_point_body_m: DVec3::new(
                f(g.ref_point_m[0]),
                f(g.ref_point_m[1]),
                f(g.ref_point_m[2]),
            ),
            cl_base,
            cl_max: curve("cl_max", &aero.lift.cl_max)?,
            cl_alpha: curve("cl_alpha_per_rad", &aero.lift.cl_alpha_per_rad)?,
            cl0: f(aero.lift.cl0),
            cl_de: curve("cl_de_per_rad", &aero.lift.cl_de_per_rad)?,
            cd0: curve("cd0", &aero.drag.cd0)?,
            cd_wave: curve("cd_wave", &aero.drag.cd_wave)?,
            k_induced: curve("k_induced", &aero.drag.k_induced)?,
            cm_0: grid("cm_0", &aero.pitch.cm_0)?,
            cm_de: curve("cm_de_per_rad", &aero.pitch.cm_de_per_rad)?,
            cm_q: f(aero.pitch.cm_q_per_rad),
            cm_alpha_dot: f(aero.pitch.cm_alpha_dot_per_rad),
            lateral: Lateral {
                cy_beta: f(t.cy_beta_per_rad),
                cl_beta: f(t.cl_beta_per_rad),
                cn_beta: f(t.cn_beta_per_rad),
                cl_p: f(t.cl_p_per_rad),
                cl_r: f(t.cl_r_per_rad),
                cn_p: f(t.cn_p_per_rad),
                cn_r: f(t.cn_r_per_rad),
                cl_da: f(t.cl_da_per_rad),
                cn_da: f(t.cn_da_per_rad),
                cl_dr: f(t.cl_dr_per_rad),
                cn_dr: f(t.cn_dr_per_rad),
                cy_dr: f(t.cy_dr_per_rad),
            },
            blend_start_deg: curve("blend_start_deg", &aero.post_stall.blend_start_deg)?,
            blend_end_deg: curve("blend_end_deg", &aero.post_stall.blend_end_deg)?,
            cd90: f(aero.post_stall.cd90),
            plate_arm_mac: f(aero.post_stall.x_ac_post_mac) - f(aero.x_ref_mac),
            eta: curve("control_effectiveness", &aero.control_effectiveness)?,
            gear_cd0: f(c.gear_cd0),
            brake_cd0: f(c.brake_cd0),
            flap_to: (f(c.flap_to_cl), f(c.flap_to_cd0)),
            flap_ld: (f(c.flap_ld_cl), f(c.flap_ld_cd0)),
            tef_max_rad: f(ac.fcs.tef.range_deg[1]).to_radians(),
            ground_effect_k: f(aero.ground_effect_k),
            alpha_peak_deg,
        })
    }

    /// Wing reference area, m^2.
    #[must_use]
    pub fn wing_area_m2(&self) -> f64 {
        self.wing_area_m2
    }

    /// Span, m.
    #[must_use]
    pub fn span_m(&self) -> f64 {
        self.span_m
    }

    /// Mean aerodynamic chord, m.
    #[must_use]
    pub fn mac_m(&self) -> f64 {
        self.mac_m
    }

    /// Aerodynamic reference point, m (body axes).
    #[must_use]
    pub fn ref_point_body_m(&self) -> DVec3 {
        self.ref_point_body_m
    }

    /// Angle of attack of the lift peak at a Mach number, degrees (interpolated between rows).
    #[must_use]
    pub fn alpha_peak_deg(&self, mach: f64) -> f64 {
        self.alpha_peak_deg.eval(mach)
    }

    /// Trimmed whole-aircraft `C_L,max` at a Mach number (for limiters and diagnostics).
    #[must_use]
    pub fn cl_max(&self, mach: f64) -> f64 {
        self.cl_max.eval(mach)
    }

    /// Lift-curve slope at a Mach number, per radian.
    #[must_use]
    pub fn cl_alpha_per_rad(&self, mach: f64) -> f64 {
        self.cl_alpha.eval(mach)
    }

    /// Elevator lift and moment per radian at a Mach number: `(C_L,de, C_m,de)`.
    #[must_use]
    pub fn elevator_power_per_rad(&self, mach: f64) -> (f64, f64) {
        (self.cl_de.eval(mach), self.cm_de.eval(mach))
    }

    /// Pitch damping derivatives `(C_mq, C_m,alpha_dot)`, per radian.
    #[must_use]
    pub fn pitch_damping_per_rad(&self) -> (f64, f64) {
        (self.cm_q, self.cm_alpha_dot)
    }

    /// The untrimmed lift and moment of the clean airframe with the elevator neutral, before the
    /// blend: `(C_L,base, C_m0)`.
    #[must_use]
    pub fn base_coefficients(&self, alpha_rad: f64, mach: f64) -> (f64, f64) {
        let deg = alpha_rad.to_degrees();
        (self.cl_base.eval(deg, mach), self.cm_0.eval(deg, mach))
    }

    /// A deflection scaled by the control-effectiveness table.
    fn effective(&self, deflection_rad: f64) -> f64 {
        self.eta.eval(deflection_rad.abs().to_degrees()) * deflection_rad
    }

    /// Flap lift and drag increments for a trailing-edge flap position.
    fn flap(&self, tef_rad: f64) -> (f64, f64) {
        if self.tef_max_rad.is_nan() || self.tef_max_rad <= 0.0 {
            return (0.0, 0.0);
        }
        let f = (tef_rad / self.tef_max_rad).clamp(0.0, 1.0);
        let (to, ld) = (self.flap_to, self.flap_ld);
        if f <= 0.5 {
            let w = 2.0 * f;
            (w * to.0, w * to.1)
        } else {
            let w = 2.0 * f - 1.0;
            (to.0 + w * (ld.0 - to.0), to.1 + w * (ld.1 - to.1))
        }
    }

    /// Ground effect (GDD 11.6.5): the induced-drag factor `phi` and the lift increment.
    fn ground_effect(&self, height_m: f64, alpha_rad: f64, mach: f64) -> (f64, f64) {
        let b = self.span_m;
        let h = height_m.max(0.0);
        if h >= GROUND_EFFECT_SPANS * b {
            return (1.0, 0.0);
        }
        let x = 16.0 * h / b;
        let phi = x * x / (1.0 + x * x);
        let dcl = self.ground_effect_k
            * self.cl_alpha.eval(mach)
            * alpha_rad
            * dmath::exp(-h / (0.5 * b));
        (phi, dcl)
    }

    /// The coefficients for a flight condition, about the reference point.
    #[must_use]
    pub fn coefficients(&self, input: &AeroInput) -> Coefficients {
        let air = &input.air;
        let s = &input.surfaces;
        let (a, b, m) = (air.alpha_rad, air.beta_rad, air.mach);
        let a_deg = a.to_degrees();
        let (sa, ca) = (dmath::sin(a), dmath::cos(a));
        let sb = dmath::sin(b);
        let de = self.effective(s.elevator_rad);
        let da = self.effective(s.aileron_rad);
        let dr = self.effective(s.rudder_rad);

        // Attached flow (GDD 11.6.1).
        let (flap_cl, flap_cd) = self.flap(s.tef_rad);
        let (phi, ground_cl) = self.ground_effect(input.height_agl_m, a, m);
        let cl_att = self.cl_base.eval(a_deg, m) + flap_cl + self.cl_de.eval(m) * de + ground_cl;
        let cd0 = self.cd0.eval(m);
        let induced = cl_att - self.cl0;
        let cd_att = cd0
            + self.cd_wave.eval(m)
            + phi * self.k_induced.eval(m) * induced * induced
            + flap_cd
            + self.gear_cd0 * s.gear
            + self.brake_cd0 * s.airbrake;

        // Post-stall blend to the flat plate (GDD 11.6.3, ADR-30).
        let blend = smoothstep(
            self.blend_start_deg.eval(m),
            self.blend_end_deg.eval(m),
            a_deg.abs(),
        );
        let plate_normal = self.cd90 * sa;
        let cl = (1.0 - blend) * cl_att + blend * plate_normal * ca;
        let cd = (1.0 - blend) * cd_att + blend * (cd0 + plate_normal * sa);
        let cm_static =
            (1.0 - blend) * self.cm_0.eval(a_deg, m) - blend * plate_normal * self.plate_arm_mac;

        // Rate terms: alpha-dot is non-dimensionalised like q.
        let v = air.tas_mps.max(MIN_TAS_FOR_ANGLES_MPS);
        let alpha_dot_hat = input.alpha_dot_rps * self.mac_m / (2.0 * v);
        let t = &self.lateral;
        Coefficients {
            cl,
            cd,
            cy: t.cy_beta * sb + t.cy_dr * dr,
            cl_roll: t.cl_beta * sb
                + t.cl_p * air.p_hat
                + t.cl_r * air.r_hat
                + t.cl_da * da
                + t.cl_dr * dr,
            cm: cm_static
                + self.cm_q * air.q_hat
                + self.cm_alpha_dot * alpha_dot_hat
                + self.cm_de.eval(m) * de,
            cn: t.cn_beta * sb
                + t.cn_p * air.p_hat
                + t.cn_r * air.r_hat
                + t.cn_da * da
                + t.cn_dr * dr,
            blend,
        }
    }

    /// Force and moment about the reference point from coefficients.
    #[must_use]
    pub fn forces(&self, c: &Coefficients, air: &AirData) -> (DVec3, DVec3) {
        let qs = air.qbar_pa * self.wing_area_m2;
        let (sa, ca) = (dmath::sin(air.alpha_rad), dmath::cos(air.alpha_rad));
        let (sb, cb) = (dmath::sin(air.beta_rad), dmath::cos(air.beta_rad));
        let (lift, drag, side) = (qs * c.cl, qs * c.cd, qs * c.cy);
        let force = DVec3::new(
            -drag * ca * cb - side * ca * sb + lift * sa,
            -drag * sb + side * cb,
            -drag * sa * cb - side * sa * sb - lift * ca,
        );
        let moment = DVec3::new(
            qs * self.span_m * c.cl_roll,
            qs * self.mac_m * c.cm,
            qs * self.span_m * c.cn,
        );
        (force, moment)
    }
}

impl AeroModel for TableAero {
    fn evaluate(&self, input: &AeroInput) -> AeroOutput {
        if input.air.qbar_pa.is_nan() || input.air.qbar_pa <= 0.0 {
            return AeroOutput::ZERO;
        }
        let c = self.coefficients(input);
        let (force, moment_ref) = self.forces(&c, &input.air);
        AeroOutput {
            force_body_n: force,
            moment_body_nm: moment_ref + (self.ref_point_body_m - input.cg_body_m).cross(force),
        }
    }
}

/// Behaviour every `AeroModel` must have (`AC-61.5`).
///
/// # Panics
/// Panics, with a message naming the property, if the implementation violates the contract.
pub fn contract(m: &dyn AeroModel) {
    let base = AeroInput {
        air: AirData {
            tas_mps: 200.0,
            alpha_rad: 0.1,
            beta_rad: 0.02,
            mach: 0.6,
            qbar_pa: 20_000.0,
            p_hat: 0.01,
            q_hat: -0.005,
            r_hat: 0.002,
        },
        alpha_dot_rps: 0.05,
        surfaces: Surfaces {
            elevator_rad: 0.1,
            aileron_rad: -0.1,
            ..Surfaces::NEUTRAL
        },
        height_agl_m: 3000.0,
        cg_body_m: DVec3::new(0.05, 0.0, 0.01),
    };
    let out = m.evaluate(&base);
    assert!(
        out.force_body_n.is_finite() && out.moment_body_nm.is_finite(),
        "finite output"
    );
    assert_eq!(out, m.evaluate(&base), "a pure function of the input");

    // No airflow, no force: this is what keeps a parked aircraft parked.
    let mut still = base;
    still.air.qbar_pa = 0.0;
    still.air.tas_mps = 0.0;
    assert_eq!(
        m.evaluate(&still),
        AeroOutput::ZERO,
        "zero dynamic pressure gives zero force and moment"
    );

    // Wild but finite conditions stay finite, all the way round in alpha and on the ground.
    for i in -12..=12 {
        let alpha = f64::from(i) * core::f64::consts::PI / 12.0;
        for q in [1.0, 5.0e4, 2.0e5] {
            let mut wild = base;
            wild.air.alpha_rad = alpha;
            wild.air.beta_rad = -alpha / 2.0;
            wild.air.mach = 2.4;
            wild.air.qbar_pa = q;
            wild.air.p_hat = 0.5;
            wild.alpha_dot_rps = -3.0;
            wild.height_agl_m = 0.0;
            wild.surfaces = Surfaces {
                elevator_rad: 1.0,
                aileron_rad: -1.0,
                rudder_rad: 1.0,
                lef_rad: 1.0,
                tef_rad: 1.0,
                airbrake: 1.0,
                gear: 1.0,
            };
            let out = m.evaluate(&wild);
            assert!(
                out.force_body_n.is_finite() && out.moment_body_nm.is_finite(),
                "finite at alpha {alpha}, q {q}"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_content::kestrel;

    fn kestrel_aero() -> TableAero {
        let k = kestrel();
        TableAero::from_defs(&k.aircraft, &k.aero).unwrap()
    }

    /// Clean, symmetric, no rates, at the reference point.
    fn at(alpha_deg: f64, mach: f64) -> AeroInput {
        AeroInput {
            air: AirData {
                tas_mps: 200.0,
                alpha_rad: alpha_deg.to_radians(),
                beta_rad: 0.0,
                mach,
                qbar_pa: 20_000.0,
                p_hat: 0.0,
                q_hat: 0.0,
                r_hat: 0.0,
            },
            alpha_dot_rps: 0.0,
            surfaces: Surfaces::NEUTRAL,
            height_agl_m: 5000.0,
            cg_body_m: DVec3::ZERO,
        }
    }

    #[test]
    fn every_model_passes_the_contract() {
        contract(&NoAero);
        contract(&LinearAero {
            wing_area_m2: 28.0,
            cl_alpha: 3.6,
            cl_max: 1.6,
            cd0: 0.0235,
            k: 0.135,
        });
        contract(&kestrel_aero());
    }

    #[test]
    fn pure_lift_is_perpendicular_to_the_airflow() {
        // GDD 11.2.4: with D = Y = 0 the force is (L sin a, 0, -L cos a).
        let m = kestrel_aero();
        for alpha_deg in [-20.0, 5.0, 25.0, 70.0, 150.0] {
            for beta_deg in [0.0, 10.0, -25.0] {
                let mut input = at(alpha_deg, 0.5);
                input.air.beta_rad = f64::to_radians(beta_deg);
                let (a, b) = (input.air.alpha_rad, input.air.beta_rad);
                let vel = DVec3::new(
                    dmath::cos(a) * dmath::cos(b),
                    dmath::sin(b),
                    dmath::sin(a) * dmath::cos(b),
                );
                let lift_only = Coefficients {
                    cl: 1.0,
                    ..Coefficients::default()
                };
                let (f, _) = m.forces(&lift_only, &input.air);
                assert!(
                    f.dot(vel).abs() < 1e-9 * f.length(),
                    "lift at {alpha_deg}/{beta_deg}"
                );
                assert_eq!(f.y, 0.0);
                let side_only = Coefficients {
                    cy: 1.0,
                    ..Coefficients::default()
                };
                let (f, _) = m.forces(&side_only, &input.air);
                assert!(
                    f.dot(vel).abs() < 1e-9 * f.length(),
                    "side at {alpha_deg}/{beta_deg}"
                );
                let drag_only = Coefficients {
                    cd: 1.0,
                    ..Coefficients::default()
                };
                let (f, _) = m.forces(&drag_only, &input.air);
                assert!(
                    (f.normalized_or(DVec3::X) + vel).length() < 1e-12,
                    "drag opposes"
                );
            }
        }
    }

    #[test]
    fn lift_drag_and_moment_at_a_worked_point() {
        // M 0.5, 6 deg, clean: C_L = 3.6 * 6 deg (linear range) = 0.37699; C_D = 0.0235 +
        // 0.135 C_L^2; C_m = 0.010 - 0.06 C_L (C.4: SM 0.06, C_m,ac 0.010).
        let m = kestrel_aero();
        let c = m.coefficients(&at(6.0, 0.5));
        let cl = 3.6 * 6f64.to_radians();
        assert!((c.cl - cl).abs() < 2e-4, "{}", c.cl);
        assert!(
            (c.cd - (0.0235 + 0.135 * c.cl * c.cl)).abs() < 1e-6,
            "{}",
            c.cd
        );
        assert!((c.cm - (0.010 - 0.06 * c.cl)).abs() < 2e-4, "{}", c.cm);
        assert_eq!(c.blend, 0.0);
        assert_eq!((c.cy, c.cl_roll, c.cn), (0.0, 0.0, 0.0), "symmetric flight");
        // Wave drag at M 1.05 is C.4's 0.026.
        let c = m.coefficients(&at(0.0, 1.05));
        assert!((c.cd - (0.0235 + 0.026)).abs() < 1e-6, "{}", c.cd);
    }

    #[test]
    fn each_row_peaks_at_the_compensated_cl_max() {
        // ADR-30: the untrimmed peak is (C_L,max - C_m,ac / 1.6) / (1 - SM / 1.6), within the
        // 2-degree grid's interpolation loss. Low Mach: 1.6558.
        let m = kestrel_aero();
        let peak_at = |mach: f64| {
            let mut best = f64::MIN;
            for i in 0..=900 {
                best = best.max(m.coefficients(&at(f64::from(i) * 0.1, mach)).cl);
            }
            best
        };
        assert!((peak_at(0.3) - 1.6558).abs() < 0.004, "{}", peak_at(0.3));
        let c12 = (1.00 + 0.015 / 1.6) / (1.0 - 0.15 / 1.6);
        assert!(
            (peak_at(1.2) - c12).abs() < 0.004,
            "{} vs {c12}",
            peak_at(1.2)
        );
        // The blend is exactly zero at the peak and for at least 5 degrees past it (the baker's
        // lint; the Kestrel's blend starts 10 degrees past the generator's peak).
        for mach in [0.0, 0.6, 0.9, 1.2, 2.0] {
            let peak = m.alpha_peak_deg(mach);
            assert_eq!(m.coefficients(&at(peak, mach)).blend, 0.0, "M {mach}");
            assert_eq!(m.coefficients(&at(peak + 4.9, mach)).blend, 0.0, "M {mach}");
        }
        assert!(
            (m.alpha_peak_deg(0.3) - 32.0).abs() < 1e-12,
            "on the grid at 32 deg"
        );
    }

    #[test]
    fn coefficients_are_continuous_and_finite_all_the_way_round() {
        let m = kestrel_aero();
        for mach in [0.2, 0.9, 1.5, 2.5] {
            let mut last = m.coefficients(&at(-180.0, mach));
            for i in -1799..=1800 {
                let c = m.coefficients(&at(f64::from(i) * 0.1, mach));
                for (now, before, what) in [
                    (c.cl, last.cl, "C_L"),
                    (c.cd, last.cd, "C_D"),
                    (c.cm, last.cm, "C_m"),
                ] {
                    assert!(
                        now.is_finite(),
                        "{what} at {} deg, M {mach}",
                        f64::from(i) * 0.1
                    );
                    assert!(
                        (now - before).abs() < 0.06,
                        "{what} jumps {before} -> {now} at {} deg, M {mach}",
                        f64::from(i) * 0.1
                    );
                }
                last = c;
            }
        }
        // Flat plate far past the stall: C_L = 1.2 sin cos, C_D = C_D0 + 1.2 sin^2.
        let c = m.coefficients(&at(120.0, 0.3));
        let a = 120f64.to_radians();
        assert_eq!(c.blend, 1.0);
        assert!((c.cl - 1.2 * dmath::sin(a) * dmath::cos(a)).abs() < 1e-6);
        assert!((c.cd - (0.0235 + 1.2 * dmath::sin(a) * dmath::sin(a))).abs() < 1e-6);
    }

    #[test]
    fn trim_does_not_add_drag_at_fixed_total_lift() {
        // Same total C_L reached with different elevator: the same drag.
        let m = kestrel_aero();
        let mut a = at(6.0, 0.5);
        let ca = m.coefficients(&a);
        a.surfaces.elevator_rad = -5f64.to_radians();
        let (cl_de, _) = m.elevator_power_per_rad(0.5);
        let dcl = cl_de * a.surfaces.elevator_rad;
        // Raise alpha by the lift the elevator took away (linear range, 3.6 /rad).
        let mut b = a;
        b.air.alpha_rad = a.air.alpha_rad - dcl / 3.6;
        let cb = m.coefficients(&b);
        assert!((cb.cl - ca.cl).abs() < 1e-3, "{} vs {}", cb.cl, ca.cl);
        let cd_at = |cl: f64| 0.0235 + 0.135 * cl * cl;
        assert!(
            (cb.cd - cd_at(cb.cl)).abs() < 1e-9,
            "drag is a function of total lift only"
        );
    }

    #[test]
    fn controls_and_rates_have_the_documented_signs() {
        let m = kestrel_aero();
        let base = m.coefficients(&at(5.0, 0.6));
        let with = |f: &dyn Fn(&mut AeroInput)| {
            let mut i = at(5.0, 0.6);
            f(&mut i);
            m.coefficients(&i)
        };
        // Elevator trailing edge down: nose-down moment, more lift.
        let c = with(&|i| i.surfaces.elevator_rad = 0.1);
        assert!(c.cm < base.cm && c.cl > base.cl);
        // Positive aileron: right wing down.
        assert!(with(&|i| i.surfaces.aileron_rad = 0.1).cl_roll > 0.0);
        // Positive rudder (trailing edge left): nose left, side force right.
        let c = with(&|i| i.surfaces.rudder_rad = 0.1);
        assert!(c.cn < 0.0 && c.cy > 0.0);
        // Wind from the right: weathercock nose right, dihedral rolls left, side force left.
        let c = with(&|i| i.air.beta_rad = 0.1);
        assert!(c.cn > 0.0 && c.cl_roll < 0.0 && c.cy < 0.0);
        // Damping opposes rates.
        assert!(with(&|i| i.air.p_hat = 0.05).cl_roll < 0.0);
        assert!(with(&|i| i.air.q_hat = 0.01).cm < base.cm);
        assert!(with(&|i| i.alpha_dot_rps = 0.5).cm < base.cm);
        assert!(with(&|i| i.air.r_hat = 0.05).cn < 0.0);
        // Control power falls beyond 15 degrees: 20 deg acts like 17 (eta 0.85).
        let c20 = with(&|i| i.surfaces.aileron_rad = 20f64.to_radians());
        assert!(
            (c20.cl_roll - 0.16 * 17f64.to_radians()).abs() < 1e-6,
            "{}",
            c20.cl_roll
        );
        let c10 = with(&|i| i.surfaces.aileron_rad = 10f64.to_radians());
        assert!(
            (c10.cl_roll - 0.16 * 10f64.to_radians()).abs() < 1e-6,
            "full power below 15"
        );
    }

    #[test]
    fn configuration_increments() {
        let m = kestrel_aero();
        let clean = m.coefficients(&at(5.0, 0.3));
        let with = |f: &dyn Fn(&mut Surfaces)| {
            let mut i = at(5.0, 0.3);
            f(&mut i.surfaces);
            m.coefficients(&i)
        };
        let gear = with(&|s| s.gear = 1.0);
        assert!((gear.cd - clean.cd - 0.015).abs() < 1e-7);
        let brake = with(&|s| s.airbrake = 1.0);
        assert!((brake.cd - clean.cd - 0.02).abs() < 1e-7);
        // Landing flaps: +0.30 lift (so flapped C_L,max is 1.90), +0.030 drag plus the induced
        // drag of the extra lift.
        let ld = with(&|s| s.tef_rad = 30f64.to_radians());
        assert!((ld.cl - clean.cl - 0.3).abs() < 1e-7);
        let induced = 0.135 * (ld.cl * ld.cl - clean.cl * clean.cl);
        assert!((ld.cd - clean.cd - 0.03 - induced).abs() < 1e-7);
        let to = with(&|s| s.tef_rad = 15f64.to_radians());
        assert!((to.cl - clean.cl - 0.3).abs() < 1e-7);
    }

    #[test]
    fn moments_transfer_to_the_cg() {
        // A CG 0.2 m behind the reference point under a lift force L (along -z): the moment
        // about the CG gains (r_ref - r_cg) x F = (0.2, 0, 0) x (0, 0, -L) = (0, 0.2 L, 0).
        let m = kestrel_aero();
        let at_ref = m.evaluate(&at(6.0, 0.5));
        let mut aft = at(6.0, 0.5);
        aft.cg_body_m = DVec3::new(-0.2, 0.0, 0.0);
        let at_cg = m.evaluate(&aft);
        assert_eq!(at_cg.force_body_n, at_ref.force_body_n);
        let expected = at_ref.moment_body_nm + DVec3::new(0.2, 0.0, 0.0).cross(at_ref.force_body_n);
        assert!((at_cg.moment_body_nm - expected).length() < 1e-6);
        assert!(
            at_cg.moment_body_nm.y > at_ref.moment_body_nm.y,
            "aft CG: less stable"
        );
    }

    #[test]
    fn ground_effect_cuts_induced_drag_near_the_ground_only() {
        let m = kestrel_aero();
        let high = m.coefficients(&at(8.0, 0.2));
        let mut low = at(8.0, 0.2);
        low.height_agl_m = 2.45; // h / b = 0.25: phi = 16 / 17
        let c = m.coefficients(&low);
        assert!(c.cl > high.cl, "ground lift");
        let induced = |cl: f64| 0.135 * cl * cl;
        assert!((c.cd - 0.0235 - induced(c.cl) * 16.0 / 17.0).abs() < 1e-9);
        let mut far = at(8.0, 0.2);
        far.height_agl_m = GROUND_EFFECT_SPANS * m.span_m();
        assert_eq!(m.coefficients(&far), high, "none above four spans");
    }
}
