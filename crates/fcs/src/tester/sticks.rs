//! From what the tester wants the aircraft to do to where it puts the stick (ADR-33).
//!
//! The tester thinks in flight-path rate and bank angle. Each assist level reads the stick as
//! something else, so each has its own inverse: a load factor (Standard), a flight-path rate and a
//! bank angle (Full), a pitch rate and a roll rate (Reduced), a surface angle (Manual). These are
//! the inverses of the laws in `standard`, `full` and `reduced`, and the Manual one is the
//! incremental inversion the laws themselves use.

use flight::aircraft::{AircraftModel, AircraftState, Sense};
use flight::physical::G0;
use simmath::dmath;
use simmath::math::DVec3;

use crate::inverse::{self, Wanted};
use crate::model::FcsModel;
use crate::sense::Sensed;
use crate::standard;
use crate::state::Assist;

/// Bank loop: roll rate asked for per radian of bank error, 1/s. // ALGO
pub const K_BANK: f64 = 2.0;
/// The largest roll rate the tester asks for, rad/s. // ALGO
pub const MAX_ROLL_RATE_RPS: f64 = 1.2;
/// The Reduced law's pedals: pedal per radian of sideslip. // ALGO
pub const K_REDUCED_BETA: f64 = 4.0;

/// What the tester wants in the air.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AirDemand {
    /// The flight-path rate, rad/s.
    pub gamma_dot: f64,
    /// The bank angle, rad.
    pub bank: f64,
}

/// Stick, roll stick and pedals, each -1 to 1.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Sticks {
    /// Pitch, +1 back.
    pub pitch: f64,
    /// Roll, +1 right.
    pub roll: f64,
    /// Pedals, +1 nose right.
    pub yaw: f64,
}

/// The stick that asks Standard for the load factor `n`.
#[must_use]
pub fn stick_for_n(n: f64, lo: f64, hi: f64) -> f64 {
    let s = if n >= 1.0 {
        (n - 1.0) / (hi - 1.0).max(1e-6)
    } else {
        (n - 1.0) / (1.0 - lo).max(1e-6)
    };
    s.clamp(-1.0, 1.0)
}

fn authority(min: f64, max: f64) -> f64 {
    min.abs().max(max.abs())
}

/// The load factor that gives the flight-path rate `gamma_dot` at the bank the aircraft is at.
#[must_use]
pub fn load_for_path(s: &Sensed, gamma_dot: f64) -> f64 {
    let cos_bank = dmath::cos(s.bank_w.abs().min(1.047)).max(0.5);
    (dmath::cos(s.gamma) + s.v / G0 * gamma_dot) / cos_bank
}

/// The sticks that give `d` under `assist`.
#[must_use]
pub fn air(
    assist: Assist,
    model: &AircraftModel,
    fcs: &FcsModel,
    ac: &AircraftState,
    sense: &Sense,
    s: &Sensed,
    d: &AirDemand,
) -> Sticks {
    let n_des = load_for_path(s, d.gamma_dot);
    let q_des = s.g_over_v() * (n_des - s.turn_load);
    let p_des = (K_BANK * (d.bank - s.phi)).clamp(-MAX_ROLL_RATE_RPS, MAX_ROLL_RATE_RPS);
    match assist {
        Assist::Standard => {
            let (lo, hi) = standard::n_limits(fcs, s);
            let p_lim = standard::roll_rate_limit(fcs, s, true).min(standard::coupling_rate_limit(
                model,
                sense,
                s,
                q_des.abs().max(s.rates.y.abs()),
            ));
            Sticks {
                pitch: stick_for_n(n_des, lo, hi),
                roll: (p_des / p_lim.max(1e-3)).clamp(-1.0, 1.0),
                yaw: 0.0,
            }
        }
        Assist::Full => Sticks {
            pitch: (d.gamma_dot / fcs.full.gamma_rate_max).clamp(-1.0, 1.0),
            roll: (d.bank / fcs.full.bank_max_rad).clamp(-1.0, 1.0),
            yaw: 0.0,
        },
        Assist::Reduced => {
            let q_max = fcs.pitch.q_max.eval(s.qbar_kpa()).max(1e-3);
            let p_lim = standard::roll_rate_limit(fcs, s, false).max(1e-3);
            Sticks {
                pitch: (q_des / q_max).clamp(-1.0, 1.0),
                roll: (p_des / p_lim).clamp(-1.0, 1.0),
                yaw: (K_REDUCED_BETA * s.beta).clamp(-1.0, 1.0),
            }
        }
        Assist::Manual => manual(model, fcs, ac, sense, s, q_des, p_des),
    }
}

/// The Manual sticks: the angular accelerations that follow the demand, the surfaces that give
/// them, and the stick that holds each surface.
fn manual(
    model: &AircraftModel,
    fcs: &FcsModel,
    ac: &AircraftState,
    sense: &Sense,
    s: &Sensed,
    q_des: f64,
    p_des: f64,
) -> Sticks {
    let r_s = standard::coordinated_yaw_rate(fcs, s, 0.0);
    let (p_b, r_b) = standard::stability_to_body(p_des, r_s, s.alpha);
    let wanted = Wanted {
        nu: DVec3::new(
            fcs.roll.k_p * (p_b - s.rates.x),
            fcs.pitch.k_q * (q_des - s.rates.y),
            fcs.yaw.k_r * (r_b - s.rates.z),
        ),
        rudder_fixed: None,
    };
    let [de, da, dr] = inverse::surfaces(model, sense, &ac.surfaces, &wanted);
    let a = &model.actuators;
    Sticks {
        pitch: ((ac.trim_pitch_rad - de) / authority(a.elevator.min, a.elevator.max))
            .clamp(-1.0, 1.0),
        roll: (da / authority(a.aileron.min, a.aileron.max)).clamp(-1.0, 1.0),
        yaw: (-dr / authority(a.rudder.min, a.rudder.max)).clamp(-1.0, 1.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stick_for_a_load_factor_inverts_the_laws_mapping() {
        for (lo, hi) in [(-3.0, 9.0), (-1.0, 4.0)] {
            for n in [-0.5, 0.0, 0.7, 1.0, 1.5, 3.0, 3.9] {
                let s = stick_for_n(n, lo, hi);
                let back = standard::n_for_stick(s, lo, hi);
                assert!((back - n).abs() < 1e-12, "n {n}: stick {s} gives {back}");
            }
        }
        assert_eq!(
            stick_for_n(20.0, -3.0, 9.0),
            1.0,
            "beyond the limit: full back"
        );
        assert_eq!(stick_for_n(-20.0, -3.0, 9.0), -1.0);
    }
}
