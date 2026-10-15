//! Control-surface actuators (GDD 11.7).
//!
//! Each surface follows its command through a first-order lag, then a rate limit and a range
//! limit. The lag is applied exactly for the fixed sub-step: `delta += k (cmd - delta)` with
//! `k = 1 - exp(-dt / tau)` computed once at load, so the response does not depend on how the
//! sub-step relates to `tau` (an explicit Euler step would overshoot for `dt > tau`).
//!
//! Angles are radians; the airbrake moves as a fraction 0 to 1.

use content::aircraft::{Actuator as ActuatorDef, Airbrake as AirbrakeDef, Fcs};
use simmath::dmath;

/// One actuator, with its update factor precomputed for a fixed step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Actuator {
    /// Lowest position.
    pub min: f64,
    /// Highest position.
    pub max: f64,
    /// Largest movement in one step (the rate limit times the step).
    pub max_step: f64,
    /// Fraction of the remaining error closed in one step, `1 - exp(-dt / tau)`.
    pub k: f64,
}

impl Actuator {
    /// An actuator with travel `[min, max]`, a rate limit (per second) and a time constant (s),
    /// stepped every `dt_s` seconds. A zero time constant follows the command at the rate limit.
    #[must_use]
    pub fn new(min: f64, max: f64, rate_per_s: f64, tau_s: f64, dt_s: f64) -> Self {
        let k = if tau_s > 0.0 {
            -dmath::exp_m1(-dt_s / tau_s)
        } else {
            1.0
        };
        Self {
            min,
            max,
            max_step: rate_per_s * dt_s,
            k,
        }
    }

    /// An angular actuator from content data (degrees), in radians.
    #[must_use]
    pub fn from_def(def: &ActuatorDef, dt_s: f64) -> Self {
        Self::new(
            f64::from(def.range_deg[0]).to_radians(),
            f64::from(def.range_deg[1]).to_radians(),
            f64::from(def.rate_dps).to_radians(),
            f64::from(def.tau_s),
            dt_s,
        )
    }

    /// The airbrake, which travels from 0 to 1.
    #[must_use]
    pub fn from_airbrake(def: &AirbrakeDef, dt_s: f64) -> Self {
        Self::new(
            0.0,
            1.0,
            f64::from(def.rate_per_s),
            f64::from(def.tau_s),
            dt_s,
        )
    }

    /// An actuator that never moves from zero (a surface the aircraft has no use for yet).
    #[must_use]
    pub const fn fixed() -> Self {
        Self {
            min: 0.0,
            max: 0.0,
            max_step: 0.0,
            k: 0.0,
        }
    }

    /// The position after one step towards `cmd` (clamped to the travel first).
    #[must_use]
    pub fn step(&self, pos: f64, cmd: f64) -> f64 {
        let target = cmd.clamp(self.min, self.max);
        let mv = (self.k * (target - pos)).clamp(-self.max_step, self.max_step);
        (pos + mv).clamp(self.min, self.max)
    }

    /// The command clamped to the travel.
    #[must_use]
    pub fn clamp(&self, value: f64) -> f64 {
        value.clamp(self.min, self.max)
    }
}

/// Surface positions or commands. Angles in radians, the airbrake and gear as fractions.
///
/// Signs (GDD 0.3.2, `docs/architecture/flight.md`): elevator positive trailing edge down (nose
/// down); aileron positive rolls right wing down; rudder positive trailing edge left (nose left);
/// flaps positive down.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Surfaces {
    /// Elevator (stabilator), rad.
    pub elevator_rad: f64,
    /// Aileron, rad.
    pub aileron_rad: f64,
    /// Rudder, rad.
    pub rudder_rad: f64,
    /// Leading-edge flaps, rad.
    pub lef_rad: f64,
    /// Trailing-edge flaps, rad.
    pub tef_rad: f64,
    /// Airbrake, 0 (stowed) to 1 (open).
    pub airbrake: f64,
    /// Gear, 0 (up) to 1 (down).
    pub gear: f64,
}

impl Surfaces {
    /// Everything neutral, stowed and up.
    pub const NEUTRAL: Self = Self {
        elevator_rad: 0.0,
        aileron_rad: 0.0,
        rudder_rad: 0.0,
        lef_rad: 0.0,
        tef_rad: 0.0,
        airbrake: 0.0,
        gear: 0.0,
    };

    /// True if every field is finite.
    #[must_use]
    pub fn is_finite(&self) -> bool {
        [
            self.elevator_rad,
            self.aileron_rad,
            self.rudder_rad,
            self.lef_rad,
            self.tef_rad,
            self.airbrake,
            self.gear,
        ]
        .iter()
        .all(|v| v.is_finite())
    }
}

/// The aircraft's actuators.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Actuators {
    /// Elevator.
    pub elevator: Actuator,
    /// Aileron.
    pub aileron: Actuator,
    /// Rudder.
    pub rudder: Actuator,
    /// Leading-edge flaps. Held at zero until the FCS schedules them (v0.1b).
    pub lef: Actuator,
    /// Trailing-edge flaps.
    pub tef: Actuator,
    /// Airbrake.
    pub airbrake: Actuator,
    /// Gear. Held up until the gear model exists (v0.1b).
    pub gear: Actuator,
}

impl Actuators {
    /// The actuators of an aircraft, stepped every `dt_s` seconds.
    #[must_use]
    pub fn from_fcs(fcs: &Fcs, dt_s: f64) -> Self {
        Self {
            elevator: Actuator::from_def(&fcs.stab, dt_s),
            aileron: Actuator::from_def(&fcs.aileron, dt_s),
            rudder: Actuator::from_def(&fcs.rudder, dt_s),
            lef: Actuator::fixed(),
            tef: Actuator::from_def(&fcs.tef, dt_s),
            airbrake: Actuator::from_airbrake(&fcs.airbrake, dt_s),
            gear: Actuator::fixed(),
        }
    }

    /// Moves every surface one step towards its command.
    #[must_use]
    pub fn step(&self, pos: &Surfaces, cmd: &Surfaces) -> Surfaces {
        Surfaces {
            elevator_rad: self.elevator.step(pos.elevator_rad, cmd.elevator_rad),
            aileron_rad: self.aileron.step(pos.aileron_rad, cmd.aileron_rad),
            rudder_rad: self.rudder.step(pos.rudder_rad, cmd.rudder_rad),
            lef_rad: self.lef.step(pos.lef_rad, cmd.lef_rad),
            tef_rad: self.tef.step(pos.tef_rad, cmd.tef_rad),
            airbrake: self.airbrake.step(pos.airbrake, cmd.airbrake),
            gear: self.gear.step(pos.gear, cmd.gear),
        }
    }

    /// Commands clamped to each surface's travel: where the surfaces settle if held.
    #[must_use]
    pub fn clamp(&self, cmd: &Surfaces) -> Surfaces {
        Surfaces {
            elevator_rad: self.elevator.clamp(cmd.elevator_rad),
            aileron_rad: self.aileron.clamp(cmd.aileron_rad),
            rudder_rad: self.rudder.clamp(cmd.rudder_rad),
            lef_rad: self.lef.clamp(cmd.lef_rad),
            tef_rad: self.tef.clamp(cmd.tef_rad),
            airbrake: self.airbrake.clamp(cmd.airbrake),
            gear: self.gear.clamp(cmd.gear),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f64 = 1.0 / 120.0;

    #[test]
    fn the_lag_is_exact_for_the_step() {
        // No rate limit in the way: after n steps the error is exp(-n dt / tau) of the start.
        let a = Actuator::new(-1.0, 1.0, 1e9, 0.05, DT);
        let mut pos = 0.0;
        for _ in 0..6 {
            pos = a.step(pos, 0.5);
        }
        let expected = 0.5 * (1.0 - dmath::exp(-6.0 * DT / 0.05));
        assert!((pos - expected).abs() < 1e-14, "{pos} vs {expected}");
        // After one time constant the response is 63.2 % of the step.
        let a = Actuator::new(-1.0, 1.0, 1e9, 6.0 * DT, DT);
        let mut pos = 0.0;
        for _ in 0..6 {
            pos = a.step(pos, 1.0);
        }
        assert!((pos - (1.0 - dmath::exp(-1.0))).abs() < 1e-14);
    }

    #[test]
    fn the_rate_and_the_travel_limit_the_motion() {
        // GDD 11.7 stabilator: 60 deg/s, so 0.5 deg per 1/120 s step at most.
        let stab = Actuator::new(
            -25f64.to_radians(),
            25f64.to_radians(),
            60f64.to_radians(),
            0.05,
            DT,
        );
        let one = stab.step(0.0, 25f64.to_radians());
        assert!((one - 0.5f64.to_radians()).abs() < 1e-15, "rate-limited");
        let mut pos = 0.0;
        for _ in 0..240 {
            pos = stab.step(pos, 40f64.to_radians());
        }
        // The lag approaches the end of travel and never passes it.
        assert!(
            pos <= stab.max && stab.max - pos < 1e-12,
            "held at the end of travel"
        );
        // A zero time constant moves at the rate limit and lands exactly on the command.
        let snap = Actuator::new(0.0, 1.0, 1.0, 0.0, DT);
        assert!((snap.step(0.0, 1.0) - DT).abs() < 1e-15);
        assert_eq!(snap.step(0.999, 1.0), 1.0);
        assert_eq!(Actuator::fixed().step(0.0, 1.0), 0.0);
    }

    #[test]
    fn content_actuators_convert_to_radians() {
        let def = ActuatorDef {
            range_deg: [-20.0, 20.0],
            rate_dps: 80.0,
            tau_s: 0.05,
        };
        let a = Actuator::from_def(&def, DT);
        assert!((a.max - 20f64.to_radians()).abs() < 1e-15);
        assert!((a.max_step - 80f64.to_radians() * DT).abs() < 1e-15);
        let b = Actuator::from_airbrake(
            &AirbrakeDef {
                rate_per_s: 1.0,
                tau_s: 0.3,
            },
            DT,
        );
        assert_eq!((b.min, b.max), (0.0, 1.0));
    }
}
