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

/// The aircraft's own controls, linearised for the flight control system (ADR-34): the moment
/// the airframe makes with the primary surfaces centred, and what each surface adds per radian.
///
/// All moments are about the CG in body axes (FRD), N m. Each derivative is per radian of raw
/// deflection and already includes the control-effectiveness fall-off at the surface's current
/// deflection (`eta(|d|)`, GDD 11.7), so the surface for a wanted moment is
/// `(M_wanted - M_free) / dM_dd` with the derivative read at the deflection being asked for.
/// Positive elevator is trailing edge down (nose down), so `dm_de` is negative.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ControlModel {
    /// Aerodynamic moment about the CG with the elevator, aileron and rudder at zero.
    pub moment_free_nm: DVec3,
    /// Pitching moment per radian of elevator.
    pub dm_de: f64,
    /// Rolling moment per radian of aileron.
    pub dl_da: f64,
    /// Rolling moment per radian of rudder.
    pub dl_dr: f64,
    /// Yawing moment per radian of aileron.
    pub dn_da: f64,
    /// Yawing moment per radian of rudder.
    pub dn_dr: f64,
    /// Body-axis `z` force per radian of elevator (the direct lift of the surface).
    pub dz_de: f64,
    /// Dynamic pressure times the reference area, N.
    pub qbar_s: f64,
    /// The whole matrix the scalars above are taken from: the moment vector per radian of
    /// elevator, aileron and rudder, in that order. The off-axis entries (the elevator's roll and
    /// yaw through the sideslip force, the rudder's pitch) are a thousandth of the main ones.
    pub moment_per_rad_nm: [DVec3; 3],
}

impl ControlModel {
    /// No airflow, no control.
    pub const ZERO: Self = Self {
        moment_free_nm: DVec3::ZERO,
        dm_de: 0.0,
        dl_da: 0.0,
        dl_dr: 0.0,
        dn_da: 0.0,
        dn_dr: 0.0,
        dz_de: 0.0,
        qbar_s: 0.0,
        moment_per_rad_nm: [DVec3::ZERO; 3],
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

/// Lateral and directional derivatives, per radian, each against `|alpha|` in degrees (ADR-38).
#[derive(Clone, Debug, PartialEq)]
struct Lateral {
    cy_beta: Table1D,
    cl_beta: Table1D,
    cn_beta: Table1D,
    cl_p: Table1D,
    cl_r: Table1D,
    cn_p: Table1D,
    cn_r: Table1D,
    cl_da: Table1D,
    cn_da: Table1D,
    cl_dr: Table1D,
    cn_dr: Table1D,
    cy_dr: Table1D,
}

/// The lateral derivatives at one angle of attack.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LateralAt {
    /// Side force from sideslip.
    pub cy_beta: f64,
    /// Rolling moment from sideslip.
    pub cl_beta: f64,
    /// Yawing moment from sideslip.
    pub cn_beta: f64,
    /// Roll damping.
    pub cl_p: f64,
    /// Rolling moment from yaw rate.
    pub cl_r: f64,
    /// Yawing moment from roll rate.
    pub cn_p: f64,
    /// Yaw damping.
    pub cn_r: f64,
    /// Rolling moment per radian of aileron.
    pub cl_da: f64,
    /// Yawing moment per radian of aileron.
    pub cn_da: f64,
    /// Rolling moment per radian of rudder.
    pub cl_dr: f64,
    /// Yawing moment per radian of rudder.
    pub cn_dr: f64,
    /// Side force per radian of rudder.
    pub cy_dr: f64,
}

impl Lateral {
    fn from_def(t: &content::aero::Lateral) -> Result<Self, ModelError> {
        Ok(Self {
            cy_beta: curve("cy_beta_per_rad", &t.cy_beta_per_rad)?,
            cl_beta: curve("cl_beta_per_rad", &t.cl_beta_per_rad)?,
            cn_beta: curve("cn_beta_per_rad", &t.cn_beta_per_rad)?,
            cl_p: curve("cl_p_per_rad", &t.cl_p_per_rad)?,
            cl_r: curve("cl_r_per_rad", &t.cl_r_per_rad)?,
            cn_p: curve("cn_p_per_rad", &t.cn_p_per_rad)?,
            cn_r: curve("cn_r_per_rad", &t.cn_r_per_rad)?,
            cl_da: curve("cl_da_per_rad", &t.cl_da_per_rad)?,
            cn_da: curve("cn_da_per_rad", &t.cn_da_per_rad)?,
            cl_dr: curve("cl_dr_per_rad", &t.cl_dr_per_rad)?,
            cn_dr: curve("cn_dr_per_rad", &t.cn_dr_per_rad)?,
            cy_dr: curve("cy_dr_per_rad", &t.cy_dr_per_rad)?,
        })
    }

    /// The derivatives at `|alpha|` = `alpha_deg`.
    fn at(&self, alpha_deg: f64) -> LateralAt {
        let a = alpha_deg.abs();
        LateralAt {
            cy_beta: self.cy_beta.eval(a),
            cl_beta: self.cl_beta.eval(a),
            cn_beta: self.cn_beta.eval(a),
            cl_p: self.cl_p.eval(a),
            cl_r: self.cl_r.eval(a),
            cn_p: self.cn_p.eval(a),
            cn_r: self.cn_r.eval(a),
            cl_da: self.cl_da.eval(a),
            cn_da: self.cn_da.eval(a),
            cl_dr: self.cl_dr.eval(a),
            cn_dr: self.cn_dr.eval(a),
            cy_dr: self.cy_dr.eval(a),
        }
    }
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
            lateral: Lateral::from_def(t)?,
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

    /// The lateral and directional derivatives at an angle of attack, degrees (read at
    /// `|alpha|`).
    #[must_use]
    pub fn lateral_at(&self, alpha_deg: f64) -> LateralAt {
        self.lateral.at(alpha_deg)
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

    /// Lift and drag coefficient increments `(dC_L, dC_D0)` of the trailing-edge flaps at a
    /// position: the take-off values at half travel, the landing values at full travel, linear
    /// between.
    #[must_use]
    pub fn flap_increments(&self, tef_rad: f64) -> (f64, f64) {
        self.flap(tef_rad)
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
        let alpha_dot_hat = self.alpha_dot_hat(input.alpha_dot_rps, air.tas_mps);
        let t = self.lateral.at(a_deg);
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

    /// The aircraft's own controls at a flight condition, for inversion by the control laws
    /// (ADR-34). The free moment is the model's own [`AeroModel::evaluate`] with the elevator,
    /// aileron and rudder centred; the derivatives are exact for the moment coefficients and take
    /// the force-induced term at the CG from the lift and induced-drag slopes at zero deflection.
    /// The `alpha_dot` the input carries is held, so the elevator's effect on it is not included.
    #[must_use]
    pub fn control_model(&self, input: &AeroInput) -> ControlModel {
        let air = &input.air;
        if air.qbar_pa.is_nan() || air.qbar_pa <= 0.0 {
            return ControlModel::ZERO;
        }
        let s = &input.surfaces;
        let centred = AeroInput {
            surfaces: Surfaces {
                elevator_rad: 0.0,
                aileron_rad: 0.0,
                rudder_rad: 0.0,
                ..*s
            },
            ..*input
        };
        let free = self.evaluate(&centred);

        // The attached-flow lift at zero deflection: the point the induced-drag slope is taken at.
        let (a_deg, m) = (air.alpha_rad.to_degrees(), air.mach);
        let (flap_cl, _) = self.flap(s.tef_rad);
        let (phi, ground_cl) = self.ground_effect(input.height_agl_m, air.alpha_rad, m);
        let cl_att = self.cl_base.eval(a_deg, m) + flap_cl + ground_cl;
        let blend = smoothstep(
            self.blend_start_deg.eval(m),
            self.blend_end_deg.eval(m),
            a_deg.abs(),
        );
        let t = self.lateral.at(a_deg);
        let (eta_e, eta_a, eta_r) = (
            self.eta.eval(s.elevator_rad.abs().to_degrees()),
            self.eta.eval(s.aileron_rad.abs().to_degrees()),
            self.eta.eval(s.rudder_rad.abs().to_degrees()),
        );

        // Per radian of raw deflection, as coefficient slopes.
        let d_cl_e = (1.0 - blend) * self.cl_de.eval(m) * eta_e;
        let d_cd_e = 2.0
            * (1.0 - blend)
            * phi
            * self.k_induced.eval(m)
            * (cl_att - self.cl0)
            * self.cl_de.eval(m)
            * eta_e;
        let elevator = Coefficients {
            cl: d_cl_e,
            cd: d_cd_e,
            cm: self.cm_de.eval(m) * eta_e,
            ..Coefficients::default()
        };
        let aileron = Coefficients {
            cl_roll: t.cl_da * eta_a,
            cn: t.cn_da * eta_a,
            ..Coefficients::default()
        };
        let rudder = Coefficients {
            cy: t.cy_dr * eta_r,
            cl_roll: t.cl_dr * eta_r,
            cn: t.cn_dr * eta_r,
            ..Coefficients::default()
        };
        let lever = self.ref_point_body_m - input.cg_body_m;
        let about_cg = |c: &Coefficients| -> (DVec3, DVec3) {
            let f = self.force(c, air);
            (self.moment_ref(c, air) + lever.cross(f), f)
        };
        let (m_e, f_e) = about_cg(&elevator);
        let (m_a, _) = about_cg(&aileron);
        let (m_r, _) = about_cg(&rudder);
        ControlModel {
            moment_free_nm: free.moment_body_nm,
            dm_de: m_e.y,
            dl_da: m_a.x,
            dl_dr: m_r.x,
            dn_da: m_a.z,
            dn_dr: m_r.z,
            dz_de: f_e.z,
            qbar_s: air.qbar_pa * self.wing_area_m2,
            moment_per_rad_nm: [m_e, m_a, m_r],
        }
    }

    /// `alpha_dot c / 2V`, with the airspeed guarded like the other rate non-dimensionals.
    fn alpha_dot_hat(&self, alpha_dot_rps: f64, tas_mps: f64) -> f64 {
        alpha_dot_rps * self.mac_m / (2.0 * tas_mps.max(MIN_TAS_FOR_ANGLES_MPS))
    }

    /// The body-axis force of the lift, drag and side-force coefficients (GDD 11.2.4).
    #[must_use]
    pub fn force(&self, c: &Coefficients, air: &AirData) -> DVec3 {
        let qs = air.qbar_pa * self.wing_area_m2;
        let (sa, ca) = (dmath::sin(air.alpha_rad), dmath::cos(air.alpha_rad));
        let (sb, cb) = (dmath::sin(air.beta_rad), dmath::cos(air.beta_rad));
        let (lift, drag, side) = (qs * c.cl, qs * c.cd, qs * c.cy);
        DVec3::new(
            -drag * ca * cb - side * ca * sb + lift * sa,
            -drag * sb + side * cb,
            -drag * sa * cb - side * sa * sb - lift * ca,
        )
    }

    /// The moment of the moment coefficients about the reference point.
    #[must_use]
    pub fn moment_ref(&self, c: &Coefficients, air: &AirData) -> DVec3 {
        let qs = air.qbar_pa * self.wing_area_m2;
        DVec3::new(
            qs * self.span_m * c.cl_roll,
            qs * self.mac_m * c.cm,
            qs * self.span_m * c.cn,
        )
    }

    /// Like [`AeroModel::evaluate`], with the angle-of-attack rate supplied once the
    /// aerodynamic force is known: `alpha_dot` receives the body force and returns the rate. The
    /// force does not depend on the rate (no lift term in it), so this is exact; the rigid-body
    /// step uses it to get the rate from the accelerations of the same stage.
    pub fn evaluate_with(
        &self,
        input: &AeroInput,
        alpha_dot: impl FnOnce(DVec3) -> f64,
    ) -> AeroOutput {
        if input.air.qbar_pa.is_nan() || input.air.qbar_pa <= 0.0 {
            return AeroOutput::ZERO;
        }
        let mut c = self.coefficients(&AeroInput {
            alpha_dot_rps: 0.0,
            ..*input
        });
        let force = self.force(&c, &input.air);
        c.cm += self.cm_alpha_dot * self.alpha_dot_hat(alpha_dot(force), input.air.tas_mps);
        AeroOutput {
            force_body_n: force,
            moment_body_nm: self.moment_ref(&c, &input.air)
                + (self.ref_point_body_m - input.cg_body_m).cross(force),
        }
    }
}

impl AeroModel for TableAero {
    fn evaluate(&self, input: &AeroInput) -> AeroOutput {
        self.evaluate_with(input, |_| input.alpha_dot_rps)
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
                let f = m.force(&lift_only, &input.air);
                assert!(
                    f.dot(vel).abs() < 1e-9 * f.length(),
                    "lift at {alpha_deg}/{beta_deg}"
                );
                assert_eq!(f.y, 0.0);
                let side_only = Coefficients {
                    cy: 1.0,
                    ..Coefficients::default()
                };
                let f = m.force(&side_only, &input.air);
                assert!(
                    f.dot(vel).abs() < 1e-9 * f.length(),
                    "side at {alpha_deg}/{beta_deg}"
                );
                let drag_only = Coefficients {
                    cd: 1.0,
                    ..Coefficients::default()
                };
                let f = m.force(&drag_only, &input.air);
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
        // M 1.2: C_L,max 1.00, C_m,ac -0.015, SM 0.13.
        let c12 = (1.00 + 0.015 / 1.6) / (1.0 - 0.13 / 1.6);
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
        // Control power falls beyond 15 degrees: 20 deg acts like 19 (eta 0.95).
        let c20 = with(&|i| i.surfaces.aileron_rad = 20f64.to_radians());
        assert!(
            (c20.cl_roll - 0.16 * 19f64.to_radians()).abs() < 1e-6,
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
    fn the_alpha_rate_enters_only_the_pitching_moment() {
        let m = kestrel_aero();
        let mut input = at(6.0, 0.5);
        let still = m.evaluate(&input);
        input.alpha_dot_rps = 0.4;
        let moving = m.evaluate(&input);
        assert_eq!(moving.force_body_n, still.force_body_n);
        assert_eq!(moving.moment_body_nm.x, still.moment_body_nm.x);
        // C_m,alpha_dot -3 per unit of alpha_dot c / 2V: dM = q S c (-3) (0.4 * 3.2 / 400).
        let expected = 20_000.0 * 28.0 * 3.2 * -3.0 * (0.4 * 3.2 / 400.0);
        let dm = moving.moment_body_nm.y - still.moment_body_nm.y;
        assert!(
            (dm - expected).abs() < 1e-6 * expected.abs(),
            "{dm} vs {expected}"
        );
        // Supplied after the force, the same result.
        let late = m.evaluate_with(&at(6.0, 0.5), |f| {
            assert_eq!(f, still.force_body_n);
            0.4
        });
        assert_eq!(late, moving);
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

    /// A deterministic stream of numbers in [0, 1) (xorshift64*), so the random-state tests are
    /// the same everywhere.
    struct Stream(u64);

    impl Stream {
        fn next(&mut self) -> f64 {
            self.0 ^= self.0 >> 12;
            self.0 ^= self.0 << 25;
            self.0 ^= self.0 >> 27;
            let v = self.0.wrapping_mul(0x2545_F491_4F6C_DD1D);
            (v >> 11) as f64 / (1u64 << 53) as f64
        }

        fn range(&mut self, lo: f64, hi: f64) -> f64 {
            lo + (hi - lo) * self.next()
        }
    }

    /// A random flight condition over the envelope, including the post-stall blend, the ground
    /// effect, every configuration and a CG off the reference point.
    fn random_input(r: &mut Stream) -> AeroInput {
        let tas = r.range(40.0, 700.0);
        AeroInput {
            air: AirData {
                tas_mps: tas,
                alpha_rad: r.range(-20.0, 70.0).to_radians(),
                beta_rad: r.range(-0.15, 0.15),
                mach: r.range(0.1, 2.3),
                qbar_pa: r.range(500.0, 70_000.0),
                p_hat: r.range(-0.05, 0.05),
                q_hat: r.range(-0.05, 0.05),
                r_hat: r.range(-0.05, 0.05),
            },
            alpha_dot_rps: r.range(-0.4, 0.4),
            surfaces: Surfaces {
                lef_rad: 0.0,
                tef_rad: r.range(0.0, 0.52),
                airbrake: r.range(0.0, 1.0),
                gear: if r.next() < 0.5 { 0.0 } else { 1.0 },
                ..Surfaces::NEUTRAL
            },
            height_agl_m: r.range(0.0, 60.0),
            cg_body_m: DVec3::new(r.range(-0.3, 0.3), 0.0, r.range(-0.1, 0.1)),
        }
    }

    /// The moment the control model predicts for deflections `d = (elevator, aileron, rudder)`.
    fn predicted(cm: &ControlModel, d: (f64, f64, f64)) -> DVec3 {
        let [e, a, r] = cm.moment_per_rad_nm;
        cm.moment_free_nm + e * d.0 + a * d.1 + r * d.2
    }

    #[test]
    fn the_control_model_reproduces_evaluate_at_ten_thousand_states() {
        let m = kestrel_aero();
        let mut r = Stream(0x9E37_79B9_7F4A_7C15);
        let mut worst = 0.0_f64;
        for _ in 0..10_000 {
            let mut input = random_input(&mut r);
            // Small deflections: the model is linear in the deflection to second order.
            let d = (
                r.range(-2e-4, 2e-4),
                r.range(-2e-4, 2e-4),
                r.range(-2e-4, 2e-4),
            );
            input.surfaces.elevator_rad = d.0;
            input.surfaces.aileron_rad = d.1;
            input.surfaces.rudder_rad = d.2;
            let cm = m.control_model(&input);
            let truth = m.evaluate(&input).moment_body_nm;
            // Relative to the airframe's moment scale q S b: the free moment can be tiny in one
            // axis while the deflection terms are not.
            let scale = cm.qbar_s * m.span_m();
            let err = (truth - predicted(&cm, d)).length() / scale;
            worst = worst.max(err);
            assert!(err < 1e-9, "relative error {err} at {input:?}");
        }
        println!("worst relative error of the control model: {worst:.2e}");
    }

    #[test]
    fn the_free_moment_is_the_evaluated_moment_with_the_surfaces_centred() {
        let m = kestrel_aero();
        let mut r = Stream(7);
        for _ in 0..200 {
            let mut input = random_input(&mut r);
            input.surfaces.elevator_rad = 0.2;
            input.surfaces.aileron_rad = -0.3;
            input.surfaces.rudder_rad = 0.1;
            let mut centred = input;
            centred.surfaces.elevator_rad = 0.0;
            centred.surfaces.aileron_rad = 0.0;
            centred.surfaces.rudder_rad = 0.0;
            assert_eq!(
                m.control_model(&input).moment_free_nm,
                m.evaluate(&centred).moment_body_nm
            );
        }
    }

    #[test]
    fn a_worked_point_at_the_reference_point() {
        // M 0.6, 5 deg, 20 kPa, CG on the reference point: the surfaces' moments are exactly
        // q S b C and q S c C_m,de, with the C.5 / A.1 derivatives.
        let m = kestrel_aero();
        let input = at(5.0, 0.6);
        let c = m.control_model(&input);
        let qs = 20_000.0 * m.wing_area_m2();
        assert_eq!(c.qbar_s, qs);
        let (cl_de, cm_de) = m.elevator_power_per_rad(0.6);
        assert!((c.dm_de - qs * m.mac_m() * cm_de).abs() < 1e-9 * qs);
        assert!(c.dm_de < 0.0, "trailing edge down pitches the nose down");
        let t = m.lateral_at(5.0);
        assert!((c.dl_da - qs * m.span_m() * t.cl_da).abs() < 1e-9 * qs);
        assert!((c.dn_da - qs * m.span_m() * t.cn_da).abs() < 1e-9 * qs);
        assert!((c.dl_dr - qs * m.span_m() * t.cl_dr).abs() < 1e-9 * qs);
        assert!((c.dn_dr - qs * m.span_m() * t.cn_dr).abs() < 1e-9 * qs);
        // The elevator's lift acts up, along -z, a little less than q S C_L,de (the induced drag
        // adds a small term along x that has no z at this attitude beyond sin(a)).
        assert!(c.dz_de < 0.0);
        assert!(
            (c.dz_de + qs * cl_de * dmath::cos(5f64.to_radians())).abs() < 0.01 * qs * cl_de,
            "{} vs {}",
            c.dz_de,
            -qs * cl_de
        );
        // Aileron: right wing down and adverse yaw; rudder: nose left.
        assert!(c.dl_da > 0.0 && c.dn_da < 0.0 && c.dn_dr < 0.0);
        // Symmetric flight, nothing rolls or yaws on its own.
        assert!(c.moment_free_nm.x.abs() < 1e-9 && c.moment_free_nm.z.abs() < 1e-9);
    }

    #[test]
    fn the_derivatives_carry_the_effectiveness_at_the_current_deflection() {
        let m = kestrel_aero();
        let neutral = m.control_model(&at(5.0, 0.6));
        // Aileron at 10 deg: full power, so the same slope.
        let mut i = at(5.0, 0.6);
        i.surfaces.aileron_rad = 10f64.to_radians();
        assert!((m.control_model(&i).dl_da - neutral.dl_da).abs() < 1e-9 * neutral.qbar_s);
        // At 20 deg it acts like 19: eta = 0.95.
        i.surfaces.aileron_rad = 20f64.to_radians();
        let c20 = m.control_model(&i);
        assert!(
            (c20.dl_da / neutral.dl_da - 0.95).abs() < 1e-6,
            "{}",
            c20.dl_da / neutral.dl_da
        );
        // And so does the elevator, which is exact for the moment the pitch law asks for.
        let mut e = at(5.0, 0.6);
        e.surfaces.elevator_rad = -20f64.to_radians();
        let ce = m.control_model(&e);
        assert!((ce.dm_de / neutral.dm_de - 0.95).abs() < 1e-6);
        let truth = m.evaluate(&e).moment_body_nm.y;
        let guess = ce.moment_free_nm.y + ce.dm_de * e.surfaces.elevator_rad;
        assert!(
            (truth - guess).abs() < 5e-3 * ce.qbar_s * m.mac_m(),
            "{truth} vs {guess}"
        );
    }

    #[test]
    fn the_inversion_hits_a_wanted_pitching_moment() {
        // What the pitch law does: pick the elevator for a wanted moment from the model.
        let m = kestrel_aero();
        let input = at(8.0, 0.5);
        let c = m.control_model(&input);
        let wanted = -0.1 * c.qbar_s * m.mac_m();
        let de = (wanted - c.moment_free_nm.y) / c.dm_de;
        assert!(de.abs() < 15f64.to_radians());
        let mut flown = input;
        flown.surfaces.elevator_rad = de;
        let got = m.evaluate(&flown).moment_body_nm.y;
        assert!(
            (got - wanted).abs() < 1e-3 * wanted.abs(),
            "{got} vs {wanted}"
        );
    }

    #[test]
    fn without_airflow_there_is_no_control() {
        let m = kestrel_aero();
        let mut input = at(5.0, 0.6);
        input.air.qbar_pa = 0.0;
        assert_eq!(m.control_model(&input), ControlModel::ZERO);
        input.air.qbar_pa = f64::NAN;
        assert_eq!(m.control_model(&input), ControlModel::ZERO);
    }
}
