//! The manoeuvre controller (GDD 19.12, ADR-47): from a goal (the direction the AI wants its
//! velocity to point, a load-factor cap, a throttle lever) to the same stick, pedals and throttle a
//! player gives.
//!
//! With `v̂` the velocity's direction, `û` local up and `θ` the angle to the goal direction `d̂`:
//! turn at `ω = min(K_TURN θ, ω_max)` towards `ê = normalize(d̂ − (d̂·v̂) v̂)`, which needs the
//! acceleration `a = V ω ê`. Lift must supply that and cancel gravity's part across the path:
//! `L = a − g⊥`. The load factor is `|L| / g0` within the cap and the bank puts the lift along
//! `L`: `φ = atan2(l̂·r̂, l̂·û⊥)` with `û⊥` up across the path and `r̂ = v̂ × û⊥` to the right.
//! `fcs::tester::sticks::air_n` turns the load factor and bank into sticks for the assist level.

use fcs::sense::Sensed;
use fcs::tester::geom::wrap;
use fcs::tester::sticks::{LoadDemand, air_n};
use fcs::{FcsModel, FcsState};
use flight::aircraft::{AircraftModel, AircraftState};
use flight::controls::PilotControls;
use flight::env::FlightEnv;
use flight::physical::G0;
use simmath::dmath;
use simmath::math::DVec3;

/// Turn rate asked for per radian of heading error, 1/s. // ALGO: ADR-47
pub const K_TURN: f64 = 0.5;
/// Angles below this count as on the goal, rad. // ALGO: ADR-47
const ON_GOAL_RAD: f64 = 1e-4;
/// Speed hold: throttle at zero error, per m/s of error, and per m/s s. // ALGO: ADR-47 (as the
/// virtual tester's speed loop)
pub const SPEED_FF: f64 = 0.5;
/// See [`SPEED_FF`].
pub const K_SPEED_P: f64 = 0.03;
/// See [`SPEED_FF`].
pub const K_SPEED_I: f64 = 0.004;
/// The speed hold's integral is kept within this, m/s s. // ALGO: as the tester's
const SPEED_I_MAX: f64 = 150.0;
/// The throttle lever at military power (the engine's detent). // DATA: `flight::engine::DETENT`
pub const MILITARY: f64 = 0.8;
/// The throttle lever in full afterburner.
pub const AFTERBURNER: f64 = 1.0;

/// What the AI wants the aircraft to do.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Goal {
    /// The direction its velocity should point, BF (any length).
    pub dir_bf: DVec3,
    /// The largest load factor it may pull, g.
    pub n_cap: f64,
    /// The throttle lever, 0 to 1 (0.8 military, 1 full afterburner).
    pub throttle: f64,
}

/// The aircraft the AI flies and the world it flies in.
pub struct Airframe<'a, E: FlightEnv> {
    /// The airframe's model.
    pub model: &'a AircraftModel,
    /// Its control system's model.
    pub fcs: &'a FcsModel,
    /// Its state.
    pub ac: &'a AircraftState,
    /// Its control system's state (the assist level is read from here).
    pub fs: &'a FcsState,
    /// The world.
    pub env: &'a E,
    /// Simulation time, s.
    pub t_s: f64,
}

impl<E: FlightEnv> Airframe<'_, E> {
    /// What the control laws see now.
    #[must_use]
    pub fn sensed(&self) -> Sensed {
        let sense = self.model.sense(self.ac, self.env, self.t_s);
        Sensed::of(self.model, self.ac, &sense, self.env)
    }

    /// Local up at the aircraft.
    #[must_use]
    pub fn up(&self) -> DVec3 {
        self.env.up_bf(self.ac.body.pos_bf)
    }
}

/// What a goal asks of the lift.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GoalCommand {
    /// The load factor, g (within the goal's cap).
    pub n: f64,
    /// The bank that points the lift where it is needed (wind axes), rad.
    pub bank: f64,
    /// The part of the needed lift that is up across the path, g (signed; not capped). Full reads
    /// the stick as a flight-path rate, which this gives at whatever bank Full allows.
    pub n_up: f64,
}

/// The load factor and bank (wind axes) that turn the velocity `vel_bf` towards `goal`, or `None`
/// when the goal or the velocity is not usable. `lift_bf` (the aircraft's lift direction, body
/// `−z`) breaks the tie when the goal is straight behind.
#[must_use]
pub fn goal_command(
    goal: &Goal,
    vel_bf: DVec3,
    up_bf: DVec3,
    gravity_bf: DVec3,
    lift_bf: DVec3,
) -> Option<GoalCommand> {
    let speed = vel_bf.length();
    let d = goal.dir_bf.normalized_or(DVec3::ZERO);
    if !(speed > 0.0 && speed.is_finite() && d.length_squared() > 0.5 && goal.n_cap.is_finite()) {
        return None;
    }
    let v_hat = vel_bf * (1.0 / speed);
    let across = |a: DVec3| a - v_hat * a.dot(v_hat);
    let up_perp = across(up_bf).normalized_or(DVec3::Z);
    let right = v_hat.cross(up_perp);
    let theta = dmath::acos(v_hat.dot(d).clamp(-1.0, 1.0));
    let n_cap = goal.n_cap.max(0.0);
    let omega_max = n_cap * G0 / speed.max(1.0);
    let omega = (K_TURN * theta).min(omega_max);
    let e = if theta < ON_GOAL_RAD {
        up_perp
    } else {
        let lateral = across(d);
        if lateral.length_squared() > 1e-12 {
            lateral.normalized_or(up_perp)
        } else {
            across(lift_bf).normalized_or(up_perp)
        }
    };
    let lift = e * (speed * omega) - across(gravity_bf);
    let len = lift.length();
    let n = (len / G0).clamp(0.0, n_cap);
    let l_hat = lift.normalized_or(up_perp);
    let bank = dmath::atan2(l_hat.dot(right), l_hat.dot(up_perp));
    let n_up = lift.dot(up_perp) / G0;
    (n.is_finite() && bank.is_finite() && n_up.is_finite()).then_some(GoalCommand { n, bank, n_up })
}

/// Full's bank-angle floor in the conversion of the vertical lift to a load factor (cos 78°).
/// // ALGO: ADR-47 (Full's own bank limit is lower)
const FULL_COS_MIN: f64 = 0.2;

/// The pilot's controls that fly `goal` (stick, pedals and throttle; nothing else is touched). A
/// goal that cannot be flown (not finite, no length) gives neutral sticks and its throttle.
#[must_use]
pub fn fly_goal<E: FlightEnv>(goal: &Goal, af: &Airframe<'_, E>) -> PilotControls {
    let throttle = if goal.throttle.is_finite() {
        goal.throttle.clamp(0.0, 1.0)
    } else {
        MILITARY
    };
    let mut c = PilotControls {
        throttle,
        ..PilotControls::default()
    };
    let body = &af.ac.body;
    let lift = body.att.rotate(DVec3::new(0.0, 0.0, -1.0));
    let Some(cmd) = goal_command(
        goal,
        body.vel_bf,
        af.up(),
        af.env.gravity_bf(body.pos_bf),
        lift,
    ) else {
        return c;
    };
    let sense = af.model.sense(af.ac, af.env, af.t_s);
    let s = Sensed::of(af.model, af.ac, &sense, af.env);
    // The bank the shorter way round from where the aircraft is.
    let bank = s.phi + wrap(cmd.bank - s.phi);
    // Full limits the bank and reads the stick as a flight-path rate: give it the load factor
    // whose vertical part, at the bank it has, is the one needed, so the path rate is right.
    let n = if af.fs.assist == fcs::Assist::Full {
        (cmd.n_up / dmath::cos(s.phi).max(FULL_COS_MIN)).clamp(0.0, goal.n_cap.max(0.0))
    } else {
        cmd.n
    };
    let st = air_n(
        af.fs.assist,
        af.model,
        af.fcs,
        af.ac,
        &sense,
        &s,
        &LoadDemand { n, bank },
    );
    if st.pitch.is_finite() && st.roll.is_finite() && st.yaw.is_finite() {
        c.pitch = st.pitch;
        c.roll = st.roll;
        c.yaw = st.yaw;
    }
    c
}

/// A proportional-integral speed hold (as the virtual tester's): the throttle lever for a
/// calibrated speed, at most `max`. The integral stops growing while the lever is against a stop
/// and the error would push it further. A non-finite speed gives [`SPEED_FF`] and leaves the
/// integral alone.
#[must_use]
pub fn hold_speed(cas_target: f64, cas: f64, integ: &mut f64, dt: f64, max: f64) -> f64 {
    let e = cas_target - cas;
    if !(e.is_finite() && integ.is_finite()) {
        return SPEED_FF.min(max);
    }
    let u = SPEED_FF + K_SPEED_P * e + K_SPEED_I * *integ;
    if (u > 0.0 && u < max) || (u >= max && e < 0.0) || (u <= 0.0 && e > 0.0) {
        *integ = (*integ + e * dt).clamp(-SPEED_I_MAX, SPEED_I_MAX);
    }
    (SPEED_FF + K_SPEED_P * e + K_SPEED_I * *integ).clamp(0.0, max)
}

/// `v` with its part along `up` removed, unit length (`fallback` if that leaves nothing).
#[must_use]
pub fn horizontal(v: DVec3, up: DVec3, fallback: DVec3) -> DVec3 {
    (v - up * v.dot(up)).normalized_or(fallback)
}

/// The direction `gamma` above the horizontal unit direction `horiz`.
#[must_use]
pub fn climb_dir(horiz: DVec3, up: DVec3, gamma: f64) -> DVec3 {
    horiz * dmath::cos(gamma) + up * dmath::sin(gamma)
}

/// The horizontal direction of a heading (0 north, clockwise positive) at a place with local up
/// `up`.
#[must_use]
pub fn heading_dir(heading_rad: f64, up: DVec3) -> DVec3 {
    horizontal(
        DVec3::new(dmath::sin(heading_rad), dmath::cos(heading_rad), 0.0),
        up,
        DVec3::Y,
    )
}

/// The flight-path angle that closes on a height: `(target − now) / per_rad_m`, within `±limit`.
#[must_use]
pub fn height_gamma(target_m: f64, now_m: f64, per_rad_m: f64, limit: f64) -> f64 {
    let g = (target_m - now_m) / per_rad_m;
    if g.is_finite() {
        g.clamp(-limit, limit)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests;
