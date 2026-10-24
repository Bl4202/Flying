//! The Reduced law (GDD 11.11.3): pitch-rate command, roll-rate command, a yaw damper and direct
//! pedals. There is no angle-of-attack hard limit and the load limit is a warning, so the aircraft
//! can be stalled and departed.

use flight::aircraft::{AircraftModel, Sense};
use flight::controls::PilotControls;
use simmath::math::DVec3;

use crate::inverse::{Wanted, effectiveness};
use crate::model::FcsModel;
use crate::sense::Sensed;
use crate::standard::{lag, roll_loop, roll_rate_limit, stability_to_body};
use crate::state::FcsState;

/// Ask for the angular acceleration the Reduced law wants.
pub fn wanted(
    model: &AircraftModel,
    fcs: &FcsModel,
    fs: &mut FcsState,
    s: &Sensed,
    sense: &Sense,
    c: &PilotControls,
) -> Wanted {
    let dt = fcs.tick_dt_s;
    let a = &model.actuators;

    // Pitch rate on the stick, up to what the dynamic pressure allows.
    let q_cmd = c.pitch * fcs.pitch.q_max.eval(s.qbar_kpa());
    fs.q_filt = lag(fs.q_filt, q_cmd, fcs.pitch.tau_cmd_s, dt);

    // Roll rate on the stick; no reductions with angle of attack or load.
    let p_raw = c.roll * roll_rate_limit(fcs, s, false);
    fs.p_filt = lag(fs.p_filt, p_raw, fcs.roll.tau_cmd_s, dt);
    let (p_b, _) = stability_to_body(fs.p_filt, 0.0, s.alpha);
    let nu_x = roll_loop(fcs, fs, s, p_b);

    // Yaw: the pedals move the rudder directly; the damper opposes the high-passed yaw rate with
    // the rudder that gives an acceleration of `-r_w / tau`.
    fs.r_prev = lag(fs.r_prev, s.rates.z, fcs.yaw.washout_s, dt);
    fs.r_wash = s.rates.z - fs.r_prev;
    let b_rudder = effectiveness(sense)[2].z;
    let damper = if b_rudder.abs() > 0.02 && fcs.yaw.yaw_damper_s > 0.0 {
        (-fs.r_wash / fcs.yaw.yaw_damper_s) / b_rudder
    } else {
        0.0
    };
    let authority = a.rudder.min.abs().max(a.rudder.max.abs());
    let rudder = (-c.yaw * authority + damper).clamp(a.rudder.min, a.rudder.max);
    Wanted {
        nu: DVec3::new(nu_x, fcs.pitch.k_q * (fs.q_filt - s.rates.y), 0.0),
        rudder_fixed: Some(rudder),
    }
}
