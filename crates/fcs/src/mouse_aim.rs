//! Mouse-aim, "fly to cursor" (GDD 58.4, ADR-34): turns a pointing direction into stick commands.
//!
//! The error is taken in body axes. In Standard the nose is to turn toward the target at a rate
//! proportional to the error, looked ahead along the rate it already closes at; the lift that does
//! it (the turn plus the lift that holds the flight path against gravity) is a vector in the body
//! y-z plane whose direction is the roll stick and whose part along the lift axis, once that points
//! the right way, is the pitch stick. In Full, whose sticks are a bank and a flight-path rate, it
//! is a heading error to a bank and an elevation error to a rate. A small cap on the commands'
//! rate of change and a soft zone round the target keep a small error from chattering. Full and
//! Standard assist only; the pedals stay with the law (auto coordination).

use flight::aircraft::AircraftModel;
use flight::frames::{Euler, attitude, euler};
use flight::physical::G0;
use simmath::dmath;
use simmath::math::{DQuat, DVec3};

use crate::full::wrap;
use crate::model::FcsModel;
use crate::sense::Sensed;
use crate::state::{Assist, FcsState};

/// Where the pilot wants to point, in the local level frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Aim {
    /// Azimuth, rad, clockwise from local north.
    pub azimuth_rad: f64,
    /// Elevation, rad, positive up.
    pub elevation_rad: f64,
}

/// How aggressively each law uses the aim. // ALGO: GDD 58.4 "more aggressive in Full"
fn scale(assist: Assist) -> f64 {
    match assist {
        Assist::Full => 1.0,
        _ => 0.85,
    }
}

/// The aim error in body axes: the unit vector to the target.
#[must_use]
pub fn target_body(att: DQuat, up: DVec3, aim: Aim) -> DVec3 {
    let dir_bf = attitude(
        up,
        Euler {
            heading_rad: aim.azimuth_rad,
            pitch_rad: aim.elevation_rad,
            roll_rad: 0.0,
        },
    )
    .rotate(DVec3::X);
    att.conjugate().rotate(dir_bf)
}

/// The angle between the nose and the target, rad.
#[must_use]
pub fn error_rad(target_body: DVec3) -> f64 {
    dmath::atan2(
        dmath::sqrt(target_body.y * target_body.y + target_body.z * target_body.z),
        target_body.x,
    )
}

/// Full holds the wings level and the flight path where the stick is centred, and its sticks are
/// a bank and a flight-path rate; it points by heading and elevation instead, with this rate of
/// closing on each error, 1/s. // ALGO: ADR-34
const FULL_RATE_GAIN: f64 = 1.2;
/// The least share of the roll rate the bank loop is scaled for, so that a nearly dead roll
/// control does not ask for the moon. // ALGO: ADR-34
const ROLL_SHARE_MIN: f64 = 0.2;
/// How much more of the load-factor error the pitch stick asks for than the pitch-rate inversion
/// alone would (the loop answers a step late). // ALGO: ADR-34
const INVERSION_GAIN: f64 = 1.2;
/// Below this error a target under the nose is reached by pushing, rad; above three times it, by
/// rolling over; in between by both. // ALGO: ADR-34
const PUSH_ONLY_RAD: f64 = 0.074_2;
/// The width of the blend from pushing to rolling over, in multiples of [`PUSH_ONLY_RAD`].
/// // ALGO: ADR-34
const ROLL_OVER_BLEND: f64 = 2.0;
/// While the target is within the push-only range the lift is not counted as pointing anywhere
/// useful below this, g, so a small push is not turned into a roll-over. // ALGO: ADR-34
const ALONG_FLOOR_G: f64 = 3.9;

/// Pitch and roll sticks that fly toward `aim`, ramp limited.
///
/// The nose is to turn toward the target at a rate proportional to the error, capped, with the
/// rate it already closes at looked ahead by `lead_s` so the pull is let go before the target. The
/// lift that does it, in g, is the turn `V w / g` toward the target plus the lift that holds the
/// flight path against gravity: a vector in the body y-z plane. The roll stick takes its direction
/// (a smooth blend from "wings level" for a small error to "roll to the target" for a large one,
/// with nothing singular at zero) and the pitch stick the part of it along the lift axis, once the
/// lift axis points that way. A target under the nose is pushed down to while it is near and
/// reached by rolling over when it is not.
///
/// The pitch stick is not the load factor itself but the one that makes the pitch rate follow it:
/// the load-factor loop turns a load-factor error into an angle-of-attack rate through the lift
/// slope and its bandwidth, so the part of the error that is asked for, rather than the whole of
/// it, gives the pitch rate that the lift would give once it had caught up. Asking for the whole
/// error overshoots the rate by the ratio of the two. In Full the sticks are a bank and a
/// flight-path rate, so the same numbers are scaled to those.
#[must_use]
pub fn sticks(
    model: &AircraftModel,
    fcs: &FcsModel,
    fs: &mut FcsState,
    s: &Sensed,
    att: DQuat,
    up: DVec3,
    aim: Aim,
) -> (f64, f64) {
    if fs.assist == Assist::Full {
        return full_sticks(fcs, fs, s, att, up, aim);
    }
    let k = &fcs.mouse_aim;
    let t = target_body(att, up, aim);
    let e = error_rad(t);
    let (lo, hi) = crate::standard::n_limits(fcs, s);
    // The direction of the target around the nose, in the body y-z plane: (right, down).
    let perp = dmath::sqrt(t.y * t.y + t.z * t.z);
    let (dy, dz) = if perp > 1e-9 {
        (t.y / perp, t.z / perp)
    } else {
        (0.0, -1.0)
    };
    let v_over_g = s.v / G0;
    // The roll rate the stick gives is what the schedules leave of the airframe's maximum: the
    // bank loop keeps the same bandwidth when it is less.
    let roll_max = fcs.limits.roll_rate_max_rps;
    let roll_share =
        (crate::standard::roll_rate_limit(fcs, s, true) / roll_max).max(ROLL_SHARE_MIN);
    // The rate the nose now closes at on the target, rad/s, and the turn rate to ask for.
    let closing = s.rates.y * -dz + s.rates.z * dy;
    let soft = if e < k.deadzone_rad {
        e / k.deadzone_rad
    } else {
        1.0
    };
    let w_raw = scale(fs.assist) * k.k_pitch * (e * soft - k.lead_s * closing);
    let w_total = w_raw.clamp(-k.rate_cap_rps, k.rate_cap_rps);
    let w_pull = w_total.max(0.0);
    // What holds the flight path against gravity, along the lift axis and sideways in the wind
    // axes, g: banked to the right, the lift must be given back to the left.
    let side = -dmath::cos(s.gamma) * dmath::sin(s.bank_w) + v_over_g * w_pull * dy;
    let along_pull = s.turn_load - v_over_g * w_pull * dz;
    let along = s.turn_load - v_over_g * w_total * dz;
    // Pushing or rolling over for a target under the nose.
    let over = ((e - PUSH_ONLY_RAD) / (ROLL_OVER_BLEND * PUSH_ONLY_RAD)).clamp(0.0, 1.0);
    let phi_push = dmath::atan2(side, along_pull.max(ALONG_FLOOR_G));
    let phi_over = dmath::atan2(side, along_pull);
    let phi = phi_push + over * wrap(phi_over - phi_push);
    // Pull only as far as the lift axis points at where the lift is wanted.
    let gate = dmath::cos(phi).max(0.0);
    let flat = dmath::sqrt(side * side + along * along);
    let wanted = if along >= 0.0 {
        flat
    } else {
        along * (1.0 - over) + flat * over
    };
    let n_req = 1.0 + (wanted - 1.0) * gate;
    // The load factor to ask the loop for so that the pitch rate is the one `n_req` gives.
    let per_g = INVERSION_GAIN * crate::inverse::n_alpha(model, s)
        / crate::standard::n_bandwidth(fcs, s)
        / v_over_g;
    let n_cmd = s.n_w + per_g.min(1.0) * (n_req - s.n_w);
    let pitch = if n_cmd >= 1.0 {
        (n_cmd - 1.0) / (hi - 1.0)
    } else {
        (n_cmd - 1.0) / (1.0 - lo)
    };
    // The bank loop's gain grows from the near value to the far one with the blend, and the roll
    // rate about the velocity vector is taken off as damping.
    let k_bank = k.k_bank + (k.k_bank_far - k.k_bank) * over;
    let p_s = s.rates.x * dmath::cos(s.alpha) + s.rates.z * dmath::sin(s.alpha);
    let pitch = pitch.clamp(-1.0, k.pitch_stick_max);
    let roll = (k_bank * phi / roll_share - k.k_roll_rate * p_s / (roll_share * roll_max))
        .clamp(-1.0, 1.0);
    let step = k.ramp_per_s * fcs.tick_dt_s;
    fs.aim_pitch += (pitch - fs.aim_pitch).clamp(-step, step);
    fs.aim_roll += (roll - fs.aim_roll).clamp(-step, step);
    (fs.aim_pitch, fs.aim_roll)
}

/// The Full law's sticks toward `aim`: a bank that turns the heading toward it and a flight-path
/// rate that raises the nose to it, each in proportion to its own error. The law holds the flight
/// path and the wings where the sticks are centred, so a small error is a small, steady correction
/// and nothing hunts; it is slower than Standard because its flight-path rate is limited.
fn full_sticks(
    fcs: &FcsModel,
    fs: &mut FcsState,
    s: &Sensed,
    att: DQuat,
    up: DVec3,
    aim: Aim,
) -> (f64, f64) {
    let k = &fcs.mouse_aim;
    let nose = euler(up, att);
    let heading_error = wrap(aim.azimuth_rad - nose.heading_rad);
    let elevation_error = aim.elevation_rad - nose.pitch_rad;
    let soft = |e: f64| {
        if e.abs() < k.deadzone_rad {
            e.abs() / k.deadzone_rad
        } else {
            1.0
        }
    };
    // The bank that turns the heading at the rate asked for: tan(bank) = V w / g.
    let turn = FULL_RATE_GAIN * heading_error * soft(heading_error);
    let bank = dmath::atan2(s.v / G0 * turn, 1.0);
    let climb = FULL_RATE_GAIN * elevation_error * soft(elevation_error);
    let pitch = (climb / fcs.full.gamma_rate_max).clamp(-1.0, 1.0);
    let roll = (bank / fcs.full.bank_max_rad).clamp(-1.0, 1.0);
    let step = k.ramp_per_s * fcs.tick_dt_s;
    fs.aim_pitch += (pitch - fs.aim_pitch).clamp(-step, step);
    fs.aim_roll += (roll - fs.aim_roll).clamp(-step, step);
    (fs.aim_pitch, fs.aim_roll)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_content::kestrel;
    use flight::aircraft::AircraftModel;

    fn level(heading: f64) -> DQuat {
        attitude(
            DVec3::Z,
            Euler {
                heading_rad: heading,
                ..Euler::default()
            },
        )
    }

    #[test]
    fn the_target_in_body_axes_and_the_error() {
        // Heading north, target straight ahead: no error. Target east: to the right (+y).
        let t = target_body(level(0.0), DVec3::Z, Aim::default());
        assert!((t - DVec3::X).length() < 1e-12 && error_rad(t) < 1e-12);
        let east = target_body(
            level(0.0),
            DVec3::Z,
            Aim {
                azimuth_rad: core::f64::consts::FRAC_PI_2,
                elevation_rad: 0.0,
            },
        );
        assert!((east - DVec3::Y).length() < 1e-12);
        assert!((error_rad(east) - core::f64::consts::FRAC_PI_2).abs() < 1e-12);
        // Above the nose: body -z.
        let above = target_body(
            level(0.0),
            DVec3::Z,
            Aim {
                azimuth_rad: 0.0,
                elevation_rad: 0.5,
            },
        );
        assert!(above.z < 0.0 && above.y.abs() < 1e-12);
        // Behind: more than a quarter turn.
        let behind = target_body(
            level(0.0),
            DVec3::Z,
            Aim {
                azimuth_rad: core::f64::consts::PI,
                elevation_rad: 0.0,
            },
        );
        assert!(error_rad(behind) > 3.0);
    }

    /// The airframe level at 200 m/s, 5 km, heading north, and what the control system senses.
    fn flying(model: &AircraftModel) -> Sensed {
        let world = flight::env::SphereSea::open();
        let body = flight::body::RigidState {
            pos_bf: DVec3::new(0.0, 0.0, 5000.0),
            vel_bf: DVec3::new(0.0, 200.0, 0.0),
            att: level(0.0),
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

    /// The sticks after the ramp has run its course, for a target `az` right of north and `el`
    /// above the horizon, with the load factor `n` (the weight if `None`).
    fn settled_at(assist: Assist, az: f64, el: f64, n: Option<f64>) -> (f64, f64) {
        let (model, fcs) = kestrel();
        let mut s = flying(&model);
        s.n_w = n.unwrap_or(s.turn_load);
        let mut fs = FcsState::new(assist, false);
        let mut out = (0.0, 0.0);
        for _ in 0..30 {
            out = sticks(
                &model,
                &fcs,
                &mut fs,
                &s,
                level(0.0),
                DVec3::Z,
                Aim {
                    azimuth_rad: az,
                    elevation_rad: el,
                },
            );
        }
        out
    }

    fn settled(assist: Assist, az: f64, el: f64) -> (f64, f64) {
        settled_at(assist, az, el, None)
    }

    #[test]
    fn a_target_above_the_nose_is_pulled_to_and_one_beside_it_is_rolled_to() {
        // Above: pull, no roll.
        let (p, r) = settled(Assist::Standard, 0.0, 0.5);
        assert!(p > 0.3 && r.abs() < 1e-9, "{p} {r}");
        // To the right at the same height: roll right, and pull less than for the one above
        // (the lift vector does not point at it yet).
        let (pr, rr) = settled(Assist::Standard, 0.5, 0.0);
        assert!(rr > 0.2, "{rr}");
        assert!(pr < p, "{pr} {p}");
        // To the left it is the mirror image.
        let (pl, rl) = settled(Assist::Standard, -0.5, 0.0);
        assert!(
            (rl + rr).abs() < 1e-9 && (pl - pr).abs() < 1e-9,
            "{rl} {pl}"
        );
    }

    #[test]
    fn a_target_far_below_is_rolled_over_for_and_a_near_one_pushed_to() {
        // 90 deg below the nose: roll (either way), no pull yet.
        let (p, r) = settled(Assist::Standard, 0.0, -1.5);
        assert!(r.abs() > 0.2 && p < 0.1, "{p} {r}");
        // 2 deg below: push, no roll.
        let (p, r) = settled(Assist::Standard, 0.0, -0.035);
        assert!(p < 0.0 && r.abs() < 0.05, "{p} {r}");
    }

    #[test]
    fn nothing_is_asked_for_on_the_target() {
        let (p, r) = settled(Assist::Standard, 0.0, 0.0);
        // Level at 1 g is what the aircraft already does; the pull is the gravity feed-forward.
        assert!(p.abs() < 0.02 && r.abs() < 1e-9, "{p} {r}");
    }

    #[test]
    fn the_pull_never_goes_past_the_stick_limit() {
        let (_, fcs) = kestrel();
        let (p, _) = settled_at(Assist::Standard, 0.0, 1.2, Some(5.0));
        assert!((p - fcs.mouse_aim.pitch_stick_max).abs() < 1e-9, "{p}");
    }

    #[test]
    fn full_banks_toward_a_heading_error_and_raises_the_nose_to_an_elevation_error() {
        let (p, r) = settled(Assist::Full, 0.0, 0.3);
        assert!(p > 0.5 && r.abs() < 1e-9, "{p} {r}");
        let (p, r) = settled(Assist::Full, 0.4, 0.0);
        assert!(r > 0.5 && p.abs() < 1e-9, "{p} {r}");
        assert!(settled(Assist::Full, -0.4, 0.0).1 < -0.5);
        assert!(settled(Assist::Full, 0.0, -0.3).0 < -0.5);
    }

    #[test]
    fn the_sticks_are_rate_limited() {
        let (model, mut fcs) = kestrel();
        fcs.mouse_aim.ramp_per_s = 3.0;
        let s = flying(&model);
        let mut fs = FcsState::new(Assist::Standard, false);
        let aim = Aim {
            azimuth_rad: 1.0,
            elevation_rad: 0.0,
        };
        let (_, r1) = sticks(&model, &fcs, &mut fs, &s, level(0.0), DVec3::Z, aim);
        // 3 per second at 60 Hz: 0.05 per tick.
        assert!((r1 - 0.05).abs() < 1e-12, "{r1}");
        let (_, r2) = sticks(&model, &fcs, &mut fs, &s, level(0.0), DVec3::Z, aim);
        assert!((r2 - 0.10).abs() < 1e-12);
    }
}
