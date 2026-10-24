//! The Full law (GDD 11.11.3): flight-path-rate pitch, bank-angle roll, automatic yaw.
//!
//! Stick back commands a flight-path rate; centred, the flight-path angle is held. Stick sideways
//! commands a bank angle; centred, the wings are levelled. Both ride on the Standard loops with
//! the angle-of-attack limit tightened by `alpha_margin`, so stall protection is stricter than in
//! Standard. During G-LOC the stick is zero and the law holds wings level and a climb or level
//! flight path, never a descent.

use flight::aircraft::{AircraftModel, Sense};
use flight::controls::PilotControls;
use simmath::dmath;
use simmath::math::DVec3;

use crate::inverse::Wanted;
use crate::model::FcsModel;
use crate::sense::Sensed;
use crate::standard::{
    beta_for_pedals, coordinated_yaw_rate, lag, pitch_n_loop, roll_loop, roll_rate_limit,
    stability_to_body,
};
use crate::state::{FcsState, Limiting};

/// The stick is centred below this, so the flight-path angle is held.
pub const STICK_CENTRED: f64 = 0.05;
/// The bank used in the load-factor feed-forward is limited to this, rad. // DATA: GDD 11.11.3
const FF_BANK_MAX_RAD: f64 = 1.047_197_551_196_597_7;
/// Above this pitch the bank angle is not defined well enough to command, rad.
const BANK_PITCH_MAX_RAD: f64 = 1.308_996_938_995_747_2;

/// Ask for the angular acceleration the Full law wants.
#[allow(clippy::too_many_arguments, reason = "the law's inputs")]
pub fn wanted(
    model: &AircraftModel,
    fcs: &FcsModel,
    fs: &mut FcsState,
    s: &Sensed,
    sense: &Sense,
    c: &PilotControls,
    gloc: bool,
    lim: &mut Limiting,
) -> Wanted {
    let k = &fcs.full;
    let dt = fcs.tick_dt_s;

    // Pitch: flight-path rate on the stick, the angle held at the centre.
    if c.pitch.abs() >= STICK_CENTRED {
        fs.gamma_ref = s.gamma;
    }
    if gloc {
        fs.gamma_ref = fs.gamma_ref.max(0.0);
    }
    let gamma_dot_cmd = c.pitch * k.gamma_rate_max;
    let cos_bank = dmath::cos(s.bank_w.abs().min(FF_BANK_MAX_RAD));
    let n_raw = s.v / flight::physical::G0 * (gamma_dot_cmd + k.k_gamma * (fs.gamma_ref - s.gamma))
        + dmath::cos(s.gamma) / cos_bank;
    let alpha_limit = fcs.limits.alpha_limit_rad - k.alpha_margin_rad;
    let q_des = pitch_n_loop(
        model,
        fcs,
        fs,
        s,
        n_raw,
        alpha_limit,
        true,
        crate::standard::pitch_power(model, sense),
        lim,
    );

    // Roll: bank angle on the stick, wings level at the centre.
    let phi_cmd = if gloc { 0.0 } else { c.roll * k.bank_max_rad };
    let p_lim = roll_rate_limit(fcs, s, true).min(crate::standard::coupling_rate_limit(
        model,
        sense,
        s,
        q_des.abs().max(s.rates.y.abs()),
    ));
    let p_raw = if s.theta.abs() > BANK_PITCH_MAX_RAD {
        0.0
    } else {
        (k.k_bank * wrap(phi_cmd - s.bank_w)).clamp(-p_lim, p_lim)
    };
    fs.p_filt = lag(fs.p_filt, p_raw, fcs.roll.tau_cmd_s, dt);

    // Yaw: coordinated, the pedals trim the sideslip.
    let beta_cmd = beta_for_pedals(fcs, s, c.yaw);
    let r_s = coordinated_yaw_rate(fcs, s, beta_cmd);
    let (p_b, r_b) = stability_to_body(fs.p_filt, r_s, s.alpha);
    let nu_x = roll_loop(fcs, fs, s, p_b);
    Wanted {
        nu: DVec3::new(
            nu_x,
            fcs.pitch.k_q * (q_des - s.rates.y),
            fcs.yaw.k_r * (r_b - s.rates.z),
        ),
        rudder_fixed: None,
    }
}

/// An angle wrapped to `(-pi, pi]`.
#[must_use]
pub fn wrap(a: f64) -> f64 {
    let two_pi = 2.0 * core::f64::consts::PI;
    let mut x = a % two_pi;
    if x > core::f64::consts::PI {
        x -= two_pi;
    } else if x <= -core::f64::consts::PI {
        x += two_pi;
    }
    x
}
