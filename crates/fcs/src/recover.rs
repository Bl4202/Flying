//! The recovery button (GDD 11.11.3, 11.15).
//!
//! Held in Full or Standard it rolls the wings level the short way and then pulls up to a small
//! climb at a limited load factor, with the angle-of-attack limiter still active. Held in Reduced
//! or Manual, and only if the aircraft has departed, it applies the anti-spin controls: ailerons
//! neutral, rudder against the yaw rate, stick forward, until the rotation has stopped.

use flight::aircraft::AircraftModel;
use flight::physical::G0;
use simmath::dmath;
use simmath::math::DVec3;

use crate::full::wrap;
use crate::inverse::Wanted;
use crate::model::FcsModel;
use crate::sense::Sensed;
use crate::standard::{
    beta_for_pedals, coordinated_yaw_rate, lag, pitch_n_loop, roll_loop, roll_rate_limit,
    stability_to_body,
};
use crate::state::{FcsState, Limiting, RecoverPhase};

/// Yaw rate above which the aircraft counts as departed, rad/s. // DATA: ADR-33 D1
pub const DEPARTED_RATE_RPS: f64 = core::f64::consts::FRAC_PI_6;
/// Angle of attack above which the aircraft counts as departed, rad. // DATA: ADR-34
pub const DEPARTED_ALPHA_RAD: f64 = 0.610_865_238_198_015_4;
/// Yaw rate below which a spin is recovered, rad/s. // DATA: ADR-33 D3
pub const RECOVERED_RATE_RPS: f64 = 0.174_532_925_199_432_96;
/// Angle of attack below which a spin is recovered, rad. // ALGO: ADR-34
pub const RECOVERED_ALPHA_RAD: f64 = core::f64::consts::FRAC_PI_6;
/// Angle of attack at which the anti-spin controls are fully off, rad: the wing flies again, and
/// holding full rudder and stick against a rotation that is no longer a stall would only start a
/// roll the other way. They come in linearly up to [`RECOVERED_ALPHA_RAD`]. // ALGO: ADR-34
pub const EASE_ALPHA_RAD: f64 = 0.261_799_387_799_149_44;
/// The bank beyond which the recovery unloads while it rolls, rad. // ALGO
const UNLOAD_BANK_RAD: f64 = core::f64::consts::FRAC_PI_2;
/// The bank within which the recovery starts to pull, rad. // ALGO
const PULL_BANK_RAD: f64 = 0.349_065_850_398_865_9;

/// Whether the aircraft has departed from controlled flight: yawing about the velocity vector
/// (a commanded roll at a high angle of attack yaws the body without departing) or stalled deep.
#[must_use]
pub fn departed(s: &Sensed) -> bool {
    s.yaw_rate_stability().abs() > DEPARTED_RATE_RPS || s.alpha.abs() > DEPARTED_ALPHA_RAD
}

/// The Full and Standard recovery: the angular acceleration to ask for.
pub fn wanted(
    model: &AircraftModel,
    fcs: &FcsModel,
    fs: &mut FcsState,
    s: &Sensed,
    sense: &flight::aircraft::Sense,
    alpha_limit_rad: f64,
    lim: &mut Limiting,
) -> Wanted {
    let r = &fcs.recover;
    let bank = wrap(s.bank_w).abs();
    let n_raw = if bank > UNLOAD_BANK_RAD {
        fs.recover = RecoverPhase::Levelling;
        0.0
    } else if bank > PULL_BANK_RAD {
        fs.recover = RecoverPhase::Levelling;
        1.0
    } else {
        fs.recover = RecoverPhase::Pulling;
        (dmath::cos(s.gamma) + s.v / G0 * fcs.full.k_gamma * (r.gamma_rad - s.gamma)).min(r.n_pull)
    };
    let power = crate::standard::pitch_power(model, sense);
    let q_des = pitch_n_loop(model, fcs, fs, s, n_raw, alpha_limit_rad, true, power, lim);
    let p_lim = roll_rate_limit(fcs, s, false);
    let p_raw = (fcs.full.k_bank * wrap(-s.bank_w)).clamp(-p_lim, p_lim);
    fs.p_filt = lag(fs.p_filt, p_raw, fcs.roll.tau_cmd_s, fcs.tick_dt_s);
    let r_s = coordinated_yaw_rate(fcs, s, beta_for_pedals(fcs, s, 0.0));
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

/// The Reduced and Manual anti-spin controls `(elevator, aileron, rudder)`, rad, if there is
/// something to recover from; ends itself once the rotation has stopped.
pub fn anti_spin(model: &AircraftModel, fs: &mut FcsState, s: &Sensed) -> Option<[f64; 3]> {
    if fs.recover == RecoverPhase::Idle {
        if !departed(s) {
            return None;
        }
        fs.recover = RecoverPhase::Levelling;
    }
    let stalled =
        ((s.alpha.abs() - EASE_ALPHA_RAD) / (RECOVERED_ALPHA_RAD - EASE_ALPHA_RAD)).clamp(0.0, 1.0);
    if stalled <= 0.0
        || (s.rates.z.abs() < RECOVERED_RATE_RPS && s.alpha.abs() < RECOVERED_ALPHA_RAD)
    {
        fs.recover = RecoverPhase::Idle;
        return None;
    }
    let a = &model.actuators;
    // Rudder against the yaw rate (a positive rudder yaws the nose left), stick towards the
    // unstalled side (forward when the angle of attack is positive: the trailing edge down),
    // ailerons neutral; all eased off as the stall goes.
    let rudder = if s.rates.z >= 0.0 {
        a.rudder.max
    } else {
        a.rudder.min
    };
    let elevator = if s.alpha >= 0.0 {
        a.elevator.max
    } else {
        a.elevator.min
    };
    Some([stalled * elevator, 0.0, stalled * rudder])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::Assist;
    use crate::test_content::kestrel;

    fn sensed(model: &AircraftModel) -> Sensed {
        let world = flight::env::SphereSea::open();
        let body = flight::body::RigidState {
            pos_bf: DVec3::new(0.0, 0.0, 5000.0),
            vel_bf: DVec3::new(0.0, 200.0, 0.0),
            att: flight::frames::attitude(DVec3::Z, flight::frames::Euler::default()),
            omega_body: DVec3::ZERO,
        };
        let ac = model.state(
            body,
            2000.0,
            flight::engine::EngineCommand::from_lever(0.6, 4),
        );
        let sense = model.sense(&ac, &world, 0.0);
        Sensed::of(model, &ac, &sense, &world)
    }

    #[test]
    fn departure_is_a_fast_yaw_or_a_high_alpha() {
        let (model, _) = kestrel();
        let mut s = sensed(&model);
        assert!(!departed(&s));
        s.rates.z = 0.6;
        assert!(departed(&s));
        s.rates.z = 0.0;
        s.alpha = 0.7;
        assert!(departed(&s));
    }

    #[test]
    fn the_anti_spin_controls_oppose_the_rotation_and_end_when_it_stops() {
        let (model, _) = kestrel();
        let mut fs = FcsState::new(Assist::Reduced, false);
        let mut s = sensed(&model);
        assert_eq!(
            anti_spin(&model, &mut fs, &s),
            None,
            "nothing to recover from"
        );
        s.rates.z = 0.9;
        s.alpha = 0.8;
        let a = &model.actuators;
        let c = anti_spin(&model, &mut fs, &s).expect("departed");
        assert_eq!(
            c,
            [a.elevator.max, 0.0, a.rudder.max],
            "a right spin: left rudder, stick forward"
        );
        s.rates.z = -0.9;
        assert_eq!(anti_spin(&model, &mut fs, &s).unwrap()[2], a.rudder.min);
        // Still holding while the aircraft is stalled, even with the rotation stopped.
        s.rates.z = 0.0;
        assert!(anti_spin(&model, &mut fs, &s).is_some());
        s.alpha = 0.3;
        assert_eq!(anti_spin(&model, &mut fs, &s), None);
        assert_eq!(fs.recover, RecoverPhase::Idle);
    }
}
