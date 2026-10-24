//! The loops of the Standard law and the pieces the Full and Reduced laws reuse (GDD 11.11.2,
//! ADR-34).
//!
//! Pitch is a load-factor loop made of the kinematics of a pull-up and an inversion of the
//! airframe's lift slope: with `alpha_dot = q - (g / V)(n - cos(gamma) cos(phi))` and
//! `n_dot = n_alpha alpha_dot`, asking for `n_dot = k_n (n_cmd - n)` gives
//! `q_des = (g / V)(n - cos(gamma) cos(phi)) + (k_n / n_alpha)(e_n + k_ni int e_n)`, so the load
//! factor closes on its command as a lag of `1 / k_n` at every dynamic pressure. The angle-of-
//! attack limiter caps `q_des` near the limit; the g limiter clamps `n_cmd` to the load limit of
//! the configuration; the overspeed wall adds a pull above the onset.
//!
//! Roll commands a stability-axis roll rate, shrunk with dynamic pressure, angle of attack and
//! load factor; yaw coordinates the turn and cancels the side force, with the pedals commanding a
//! sideslip.

use flight::aircraft::AircraftModel;
use flight::physical::G0;
use simmath::dmath;

use crate::inverse::n_alpha;
use crate::model::FcsModel;
use crate::sense::Sensed;
use crate::state::{FcsState, Limiting};

/// The load-factor loop is clamped so the integral cannot ask for more than this, g.
const N_INT_MAX_G: f64 = 2.0;
/// The load-factor integral only collects error this small, g. It is there to remove a steady
/// offset; collecting the large errors of a manoeuvre would only leave a slow tail behind it.
/// // ALGO: conditional integration
const N_INT_BAND_G: f64 = 0.25;
/// The load-factor loop bandwidth is `k_n` at this dynamic pressure and follows its square root,
/// like the short-period frequency the loop has to stay below, Pa. // ALGO: ADR-34
const Q_REF_PA: f64 = 20_000.0;
/// The bandwidth schedule stays between these multiples of `k_n`. // ALGO: ADR-34
const BW_SCALE_MIN: f64 = 0.6;
/// See [`BW_SCALE_MIN`].
const BW_SCALE_MAX: f64 = 1.8;
/// The angle-of-attack limiter looks this far ahead along the present rate of change, s: about
/// the lag of the pitch-rate loop, so the aircraft is slowed before it reaches the limit instead
/// of passing through it. // ALGO: ADR-34
const ALPHA_LEAD_S: f64 = 0.4;
/// Share of the elevator's pitch-acceleration power the angle-of-attack limiter counts on to
/// stop the pull, and the slowest approach it always allows, rad/s. The load-factor loop is
/// kept to the rate from which the aircraft could still be stopped at the limit
/// (`alpha_dot <= sqrt(2 a room)`), which depends on the control power at the time. // ALGO: ADR-34
const BRAKE_SHARE: f64 = 0.4;
/// See [`BRAKE_SHARE`].
const ALPHA_RATE_FLOOR_RPS: f64 = 0.05;
/// Delay before the elevator starts to stop the pull: the pitch-rate loop, the command's lead
/// and the actuator, s. // ALGO: ADR-34
const STOP_DELAY_S: f64 = 0.3;
/// The roll-rate integral is clamped to this, rad.
const P_INT_MAX_RAD: f64 = 0.5;
/// Share of the rudder's yaw acceleration that the inertia coupling of a rolling pull may use
/// before the roll rate is held back. // ALGO: ADR-34
const COUPLING_SHARE: f64 = 0.5;
/// Pitch rate below which the coupling does not limit the roll rate, rad/s. // ALGO: ADR-34
const COUPLING_Q_FLOOR_RPS: f64 = 0.1;

/// The angular acceleration the elevator can give in pitch at full travel now, rad/s^2.
#[must_use]
pub fn pitch_power(model: &AircraftModel, sense: &flight::aircraft::Sense) -> f64 {
    let a = &model.actuators.elevator;
    crate::inverse::effectiveness(sense)[0].y.abs() * a.max.min(-a.min)
}

/// The fastest rate from which a deceleration `a` that starts [`STOP_DELAY_S`] late still stops
/// within `room`: `v T + v^2 / 2a = room`.
#[must_use]
pub fn stoppable_rate(a_rps2: f64, room_rad: f64) -> f64 {
    if a_rps2 <= 1e-9 {
        return 0.0;
    }
    a_rps2 * (dmath::sqrt(STOP_DELAY_S * STOP_DELAY_S + 2.0 * room_rad / a_rps2) - STOP_DELAY_S)
}

/// A first-order filter step of time constant `tau` over `dt`.
#[must_use]
pub fn lag(prev: f64, target: f64, tau_s: f64, dt_s: f64) -> f64 {
    if tau_s <= 0.0 {
        return target;
    }
    prev + (1.0 - dmath::exp(-dt_s / tau_s)) * (target - prev)
}

/// The load limits for the configuration the aircraft is in, `(lowest, highest)`, g.
#[must_use]
pub fn n_limits(fcs: &FcsModel, s: &Sensed) -> (f64, f64) {
    let out = s.gear_pos > flight::gear::GEAR_UP_MAX
        || s.flap_fraction > flight::structure::FLAPS_OUT_MIN;
    let lo = if out {
        fcs.limits.n_min.max(fcs.n_min_config)
    } else {
        fcs.limits.n_min
    };
    (lo, fcs.limits.n_max_for(s.gear_pos, s.flap_fraction))
}

/// The load factor a pitch stick position asks for: neutral is 1 g (read literally from GDD
/// 11.11.2), full back is the limit, full forward the negative limit.
#[must_use]
pub fn n_for_stick(stick: f64, lo: f64, hi: f64) -> f64 {
    if stick >= 0.0 {
        1.0 + stick * (hi - 1.0)
    } else {
        1.0 + stick * (1.0 - lo)
    }
}

/// How far past the onset of the overspeed wall the aircraft is (0 inside it).
#[must_use]
pub fn overspeed_excess(fcs: &FcsModel, s: &Sensed) -> f64 {
    let by_cas = s.cas / fcs.limits.vmo_mps - fcs.overspeed_onset;
    let by_mach = s.mach / fcs.limits.mmo - fcs.overspeed_onset;
    by_cas.max(by_mach).max(0.0)
}

/// The bandwidth of the load-factor loop, 1/s: the gain, grown with dynamic pressure about the
/// reference.
#[must_use]
pub fn n_bandwidth(fcs: &FcsModel, s: &Sensed) -> f64 {
    fcs.pitch.k_n * dmath::sqrt(s.qbar_pa / Q_REF_PA).clamp(BW_SCALE_MIN, BW_SCALE_MAX)
}

/// The pitch rate the load-factor loop asks for, rad/s, for the load factor `n_raw` the pilot (or
/// the Full law) wants. `alpha_limit_rad` is the limit this law holds. Updates the filter and
/// integrator in `fs` and the limiter flags.
#[allow(
    clippy::too_many_arguments,
    reason = "one loop: command, limit, state, flags"
)]
pub fn pitch_n_loop(
    model: &AircraftModel,
    fcs: &FcsModel,
    fs: &mut FcsState,
    s: &Sensed,
    n_raw: f64,
    alpha_limit_rad: f64,
    alpha_limiter: bool,
    pitch_power: f64,
    lim: &mut Limiting,
) -> f64 {
    let dt = fcs.tick_dt_s;
    let (lo, hi) = n_limits(fcs, s);
    let excess = overspeed_excess(fcs, s);
    lim.overspeed = excess > 0.0;
    let wanted = n_raw + fcs.overspeed_k_pull * excess;
    lim.g = wanted > hi || wanted < lo;
    let n_cmd = wanted.clamp(lo, hi);
    fs.n_cmd = n_cmd;
    fs.n_filt = lag(fs.n_filt, n_cmd, fcs.pitch.tau_cmd_s, dt);

    let k = &fcs.pitch;
    let e = fs.n_filt - s.n_w;
    // At a fixed angle of attack the load factor follows the dynamic pressure, so a climb, a dive
    // or a speed change moves it with no help from the pilot; the loop takes that out of the
    // angle-of-attack rate it asks for instead of waiting for the error.
    let dlnq = if fs.qbar_prev > 0.0 && s.qbar_pa > 0.0 {
        ((s.qbar_pa - fs.qbar_prev) / (s.qbar_pa * dt)).clamp(-2.0, 2.0)
    } else {
        0.0
    };
    fs.qbar_prev = s.qbar_pa;
    let bandwidth = n_bandwidth(fcs, s);
    let mut alpha_dot = (bandwidth * (e + k.k_ni * fs.n_int) - s.n_w * dlnq) / n_alpha(model, s);
    if alpha_limiter {
        // Stay within the rate from which the elevator can still stop the pull at the limit.
        let a_stop = BRAKE_SHARE * pitch_power;
        let up_room = (alpha_limit_rad - s.alpha).max(0.0);
        let down_room = (s.alpha - k.alpha_min_rad).max(0.0);
        alpha_dot = alpha_dot
            .min(stoppable_rate(a_stop, up_room) + ALPHA_RATE_FLOOR_RPS)
            .max(-stoppable_rate(a_stop, down_room) - ALPHA_RATE_FLOOR_RPS);
    }
    let q_hold = s.g_over_v() * (s.n_w - s.turn_load);
    let mut q_des = q_hold + alpha_dot;

    // The angle-of-attack limiter: inside the band, looked at a moment ahead, the allowed pull
    // falls to what holds alpha and then to what brings it back.
    let mut limited = false;
    if alpha_limiter {
        // Here the real rate of change counts, the roll coupling included.
        let q_keep = s.q_hold();
        let alpha_dot_now = s.rates.y - q_keep;
        let alpha_ahead = s.alpha + ALPHA_LEAD_S * alpha_dot_now;
        if alpha_ahead > alpha_limit_rad - k.alpha_band_rad {
            let q_cap = q_keep + k.k_alpha * (alpha_limit_rad - alpha_ahead);
            if q_des > q_cap {
                q_des = q_cap;
                limited = true;
            }
        }
        if alpha_ahead < k.alpha_min_rad + k.alpha_band_rad {
            let q_floor = q_keep + k.k_alpha * (k.alpha_min_rad - alpha_ahead);
            if q_des < q_floor {
                q_des = q_floor;
                limited = true;
            }
        }
    }
    lim.alpha = limited;
    // The integrator waits while a limiter is holding the command back.
    if !limited && k.k_ni > 0.0 && e.abs() <= N_INT_BAND_G {
        let max = N_INT_MAX_G / k.k_ni;
        fs.n_int = (fs.n_int + e * dt).clamp(-max, max);
    }
    q_des
}

/// The largest roll rate available now, rad/s: the airframe's maximum shrunk with dynamic
/// pressure, angle of attack and load factor.
#[must_use]
pub fn roll_rate_limit(fcs: &FcsModel, s: &Sensed, shrink: bool) -> f64 {
    let r = &fcs.roll;
    let mut f = r.p_max_qbar.eval(s.qbar_kpa());
    if shrink {
        f *= r.p_max_alpha.eval(s.alpha.abs().to_degrees()) * r.p_max_nz.eval(s.n_w);
    }
    fcs.limits.roll_rate_max_rps * f
}

/// The roll rate at which the inertia coupling of rolling while pitching (`(I_x - I_y) / I_z p q`
/// in yaw) uses [`COUPLING_SHARE`] of the yaw acceleration the rudder has, at the pitch rate
/// `q_rps` now or asked for: faster than that and the nose is thrown sideways faster than the
/// rudder can hold it. The pitch rate that turns the flight path under gravity in a bank at 1 g is
/// not counted, so that a plain roll is as fast upside down as the right way up.
#[must_use]
pub fn coupling_rate_limit(
    model: &AircraftModel,
    sense: &flight::aircraft::Sense,
    s: &Sensed,
    q_rps: f64,
) -> f64 {
    let q_rps = (q_rps.abs() - s.g_over_v() * (1.0 - s.turn_load)).max(0.0);
    let i = &sense.mass.inertia_kgm2.m;
    let gain = ((i[0][0] - i[1][1]) / i[2][2]).abs();
    if gain < 1e-9 {
        return f64::INFINITY;
    }
    let r = &model.actuators.rudder;
    let power = crate::inverse::effectiveness(sense)[2].z.abs() * r.max.min(-r.min);
    COUPLING_SHARE * power / (gain * q_rps.abs().max(COUPLING_Q_FLOOR_RPS))
}

/// The body rates `(p, r)` for stability-axis rates `(p_s, r_s)`.
#[must_use]
pub fn stability_to_body(p_s: f64, r_s: f64, alpha: f64) -> (f64, f64) {
    let (sa, ca) = (dmath::sin(alpha), dmath::cos(alpha));
    (p_s * ca - r_s * sa, p_s * sa + r_s * ca)
}

/// Roll-rate command with the prefilter and its integral: the body roll acceleration to ask for,
/// rad/s^2, for a stability-axis rate command `p_s_cmd` and the body-axis rate `p_cmd_body`.
pub fn roll_loop(fcs: &FcsModel, fs: &mut FcsState, s: &Sensed, p_cmd_body: f64) -> f64 {
    let k = &fcs.roll;
    let e = p_cmd_body - s.rates.x;
    fs.p_int = (fs.p_int + e * fcs.tick_dt_s).clamp(-P_INT_MAX_RAD, P_INT_MAX_RAD);
    k.k_p * (e + k.k_pi * fs.p_int)
}

/// The stability-axis yaw rate that coordinates the turn and drives the sideslip to `beta_cmd`.
#[must_use]
pub fn coordinated_yaw_rate(fcs: &FcsModel, s: &Sensed, beta_cmd: f64) -> f64 {
    G0 / s.v * dmath::sin(s.phi) * dmath::cos(s.theta)
        + s.side_rate
        + fcs.yaw.k_beta * (s.beta - beta_cmd)
}

/// The sideslip the pedals command, rad (pedal right asks for the nose right of the flow, which
/// is negative sideslip).
#[must_use]
pub fn beta_for_pedals(fcs: &FcsModel, s: &Sensed, yaw: f64) -> f64 {
    -yaw * fcs.yaw.beta_max.eval(s.qbar_kpa())
}

/// Ask for the angular acceleration the Standard law wants from the stick and pedals.
pub fn wanted(
    model: &AircraftModel,
    fcs: &FcsModel,
    fs: &mut FcsState,
    s: &Sensed,
    sense: &flight::aircraft::Sense,
    c: &flight::controls::PilotControls,
    lim: &mut Limiting,
) -> crate::inverse::Wanted {
    let (lo, hi) = n_limits(fcs, s);
    let n_raw = n_for_stick(c.pitch, lo, hi);
    let q_des = pitch_n_loop(
        model,
        fcs,
        fs,
        s,
        n_raw,
        fcs.limits.alpha_limit_rad,
        true,
        pitch_power(model, sense),
        lim,
    );
    let p_lim = roll_rate_limit(fcs, s, true).min(coupling_rate_limit(
        model,
        sense,
        s,
        q_des.abs().max(s.rates.y.abs()),
    ));
    let p_raw = c.roll * p_lim;
    fs.p_filt = lag(fs.p_filt, p_raw, fcs.roll.tau_cmd_s, fcs.tick_dt_s);
    let r_s = coordinated_yaw_rate(fcs, s, beta_for_pedals(fcs, s, c.yaw));
    let (p_b, r_b) = stability_to_body(fs.p_filt, r_s, s.alpha);
    let nu_x = roll_loop(fcs, fs, s, p_b);
    crate::inverse::Wanted {
        nu: simmath::math::DVec3::new(
            nu_x,
            fcs.pitch.k_q * (q_des - s.rates.y),
            fcs.yaw.k_r * (r_b - s.rates.z),
        ),
        rudder_fixed: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stick_maps_to_load_factor_with_one_g_at_the_centre() {
        // GDD 11.11.2: n_cmd = 1 + s (n_max - 1) back, 1 + s (1 - n_min) forward.
        assert_eq!(n_for_stick(0.0, -3.0, 9.0), 1.0);
        assert_eq!(n_for_stick(1.0, -3.0, 9.0), 9.0);
        assert_eq!(n_for_stick(0.5, -3.0, 9.0), 5.0);
        assert_eq!(n_for_stick(-1.0, -3.0, 9.0), -3.0);
        assert_eq!(n_for_stick(-0.5, -3.0, 9.0), -1.0);
        // The gear or flaps out lowers the top.
        assert_eq!(n_for_stick(1.0, -1.0, 4.0), 4.0);
    }

    #[test]
    fn stability_and_body_rates_are_a_rotation_by_alpha() {
        let (p, r) = stability_to_body(1.0, 0.0, 0.0);
        assert_eq!((p, r), (1.0, 0.0));
        let a = 20f64.to_radians();
        let (p, r) = stability_to_body(1.0, 0.0, a);
        assert!((p - dmath::cos(a)).abs() < 1e-15 && (r - dmath::sin(a)).abs() < 1e-15);
        // The rotation preserves the length.
        let (p, r) = stability_to_body(0.3, -0.2, a);
        assert!((p * p + r * r - (0.3_f64 * 0.3 + 0.2_f64 * 0.2)).abs() < 1e-15);
    }

    #[test]
    fn the_stoppable_rate_solves_the_braking_distance() {
        // v T + v^2 / (2 a) = room with a = 2 rad/s^2, T = 0.25 s, room 0.3 rad: v = 0.6325...
        let (a, room) = (2.0, 0.3);
        let v = stoppable_rate(a, room);
        assert!(
            (v * STOP_DELAY_S + v * v / (2.0 * a) - room).abs() < 1e-12,
            "{v}"
        );
        assert_eq!(stoppable_rate(a, 0.0), 0.0);
        assert_eq!(stoppable_rate(0.0, 1.0), 0.0);
        // More power, more rate; more room, more rate.
        assert!(stoppable_rate(4.0, room) > v && stoppable_rate(a, 2.0 * room) > v);
    }

    #[test]
    fn a_first_order_lag_closes_the_gap_by_the_exact_fraction() {
        let x = lag(0.0, 1.0, 0.1, 0.1);
        assert!((x - (1.0 - dmath::exp(-1.0))).abs() < 1e-15);
        assert_eq!(lag(3.0, 5.0, 0.0, 0.016), 5.0, "no time constant: no lag");
        assert_eq!(lag(2.0, 2.0, 0.5, 0.016), 2.0);
    }
}
