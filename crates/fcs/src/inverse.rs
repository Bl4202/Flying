//! Incremental dynamic inversion of the angular acceleration (ADR-34).
//!
//! The airframe model gives the angular acceleration it has *now*, `w_dot`, and how much each
//! primary surface changes it, `B` (rad/s^2 per radian, from the aircraft's own tables through
//! [`flight::aero::ControlModel`]). To get a wanted angular acceleration `nu` the surfaces move by
//! `B^-1 (nu - w_dot)` from where they are. There is no gain schedule over dynamic pressure
//! because the inversion divides by the control power; this is the loop the v0.1a test pilot
//! closes, and it stays well below the 15 rad/s the integrator is built for (GDD 11.10).

use flight::actuators::{Actuator, Surfaces};
use flight::aircraft::{AircraftModel, Sense};
use simmath::math::DVec3;

use crate::sense::Sensed;

/// A command runs at most this far ahead of the surface: the travel its rate limit allows in this
/// time, s. Without it the inversion at low dynamic pressure asks for steps the rate limit turns
/// into phase lag. // ALGO: the value that settled the v0.1a test pilot
pub const LEAD_S: f64 = 0.1;
/// Control power below this (rad/s^2 per rad) is treated as none.
const MIN_POWER: f64 = 0.02;

/// The columns of the effectiveness matrix: angular acceleration per radian of elevator,
/// aileron and rudder, body axes.
#[must_use]
pub fn effectiveness(sense: &Sense) -> [DVec3; 3] {
    let inv = sense.mass.inertia_inv;
    sense.controls.moment_per_rad_nm.map(|m| inv.mul_vec(m))
}

/// Solves the 2 x 2 system `[a b; c d] x = [e; f]`; `None` if it is near singular.
fn solve2(m: [[f64; 2]; 2], rhs: [f64; 2]) -> Option<[f64; 2]> {
    let det = m[0][0] * m[1][1] - m[0][1] * m[1][0];
    let scale = (m[0][0] * m[1][1]).abs().max((m[0][1] * m[1][0]).abs());
    if !det.is_finite() || det.abs() < 1e-3 * scale || det.abs() < 1e-12 {
        return None;
    }
    Some([
        (rhs[0] * m[1][1] - m[0][1] * rhs[1]) / det,
        (m[0][0] * rhs[1] - rhs[0] * m[1][0]) / det,
    ])
}

/// What the inversion is asked for.
#[derive(Clone, Copy, Debug, Default)]
pub struct Wanted {
    /// Wanted body angular acceleration, rad/s^2.
    pub nu: DVec3,
    /// A rudder deflection to hold instead of solving for it (the Reduced law's pedals), rad.
    pub rudder_fixed: Option<f64>,
}

/// The surface increments `(elevator, aileron, rudder)`, rad, that give `wanted` from the state
/// the airframe is in. Zero for an axis with no control power.
#[must_use]
pub fn increments(sense: &Sense, wanted: &Wanted, rudder_now: f64) -> [f64; 3] {
    let b = effectiveness(sense);
    let err = wanted.nu - sense.omega_dot_rps2;
    let de = if b[0].y.abs() > MIN_POWER {
        err.y / b[0].y
    } else {
        0.0
    };
    // The elevator's own effect on roll and yaw (through the sideslip force) is small but known.
    let ex = err.x - b[0].x * de;
    let ez = err.z - b[0].z * de;
    if let Some(dr) = wanted.rudder_fixed {
        let d_rudder = dr - rudder_now;
        let da = if b[1].x.abs() > MIN_POWER {
            (ex - b[2].x * d_rudder) / b[1].x
        } else {
            0.0
        };
        return [de, da, d_rudder];
    }
    if let Some([da, dr]) = solve2([[b[1].x, b[2].x], [b[1].z, b[2].z]], [ex, ez]) {
        return [de, da, dr];
    }
    // Degenerate (no yaw power, say): each axis on its own surface.
    let da = if b[1].x.abs() > MIN_POWER {
        ex / b[1].x
    } else {
        0.0
    };
    let dr = if b[2].z.abs() > MIN_POWER {
        ez / b[2].z
    } else {
        0.0
    };
    [de, da, dr]
}

/// A surface command from the surface now plus an increment, run at most [`LEAD_S`] ahead and
/// kept in the actuator's travel, rad.
#[must_use]
pub fn lead(model: &AircraftModel, act: &Actuator, now: f64, delta: f64) -> f64 {
    let most = act.max_step * f64::from(flight::aircraft::SUBSTEPS) * LEAD_S / model.tick_dt_s();
    act.clamp(now + delta.clamp(-most, most))
}

/// Inversion to surface commands: the elevator, aileron and rudder to give `wanted`, with the
/// other surfaces as they are.
#[must_use]
pub fn surfaces(model: &AircraftModel, sense: &Sense, now: &Surfaces, wanted: &Wanted) -> [f64; 3] {
    let d = increments(sense, wanted, now.rudder_rad);
    let a = &model.actuators;
    [
        lead(model, &a.elevator, now.elevator_rad, d[0]),
        lead(model, &a.aileron, now.aileron_rad, d[1]),
        lead(model, &a.rudder, now.rudder_rad, d[2]),
    ]
}

/// The airframe's load-factor slope with angle of attack, g per radian (`q S C_L,alpha / W`),
/// floored so the loops stay finite at low dynamic pressure.
#[must_use]
pub fn n_alpha(model: &AircraftModel, s: &Sensed) -> f64 {
    let qs = s.qbar_pa * model.aero.wing_area_m2();
    let w = s.tel.mass_kg * flight::physical::G0;
    (model.aero.cl_alpha_per_rad(s.mach) * qs / w).max(1.5)
}
