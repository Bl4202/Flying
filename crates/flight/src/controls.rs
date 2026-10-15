//! Pilot controls and control laws (GDD 11.11).
//!
//! v0.1a has one law, [`ManualLaw`]: the stick drives the surfaces directly around the held pitch
//! trim, with no augmentation. The FCS laws of v0.1b implement the same [`ControlLaw`] trait.

use crate::actuators::{Actuators, Surfaces};

/// Pitch trim rate, rad/s. // ALGO: 2 deg/s, a typical trim-motor rate (GDD 11.11)
pub const TRIM_RATE_RPS: f64 = 2.0 * core::f64::consts::PI / 180.0;

/// What the pilot (or a script, or the test pilot) is doing with the controls.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PilotControls {
    /// Pitch stick, -1 (forward, nose down) to +1 (back, nose up).
    pub pitch: f64,
    /// Roll stick, -1 (left) to +1 (right).
    pub roll: f64,
    /// Pedals, -1 (left, nose left) to +1 (right, nose right).
    pub yaw: f64,
    /// Throttle lever, 0 (idle) to 1 (full afterburner); the military detent is at 0.8.
    pub throttle: f64,
    /// Pitch trim nose up held.
    pub trim_nose_up: bool,
    /// Pitch trim nose down held.
    pub trim_nose_down: bool,
    /// Flap lever, 0 (up) to 1 (landing).
    pub flaps: f64,
    /// Airbrake lever, 0 (stowed) to 1 (open).
    pub airbrake: f64,
}

impl PilotControls {
    /// Every input clamped to its range, with NaN treated as neutral, so a bad input can never
    /// reach the model.
    #[must_use]
    pub fn sanitized(&self) -> Self {
        let c = |v: f64, lo: f64, hi: f64| if v.is_nan() { 0.0 } else { v.clamp(lo, hi) };
        Self {
            pitch: c(self.pitch, -1.0, 1.0),
            roll: c(self.roll, -1.0, 1.0),
            yaw: c(self.yaw, -1.0, 1.0),
            throttle: c(self.throttle, 0.0, 1.0),
            trim_nose_up: self.trim_nose_up,
            trim_nose_down: self.trim_nose_down,
            flaps: c(self.flaps, 0.0, 1.0),
            airbrake: c(self.airbrake, 0.0, 1.0),
        }
    }
}

/// What a control law sees.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LawInput {
    /// The pilot's controls, sanitized.
    pub controls: PilotControls,
    /// The held pitch trim, elevator radians.
    pub trim_pitch_rad: f64,
}

/// Turns pilot controls into surface commands.
pub trait ControlLaw {
    /// Surface commands for this tick.
    fn commands(&self, input: &LawInput, actuators: &Actuators) -> Surfaces;
}

/// Direct control: full stick is full surface travel, around the trim for pitch.
///
/// `elevator = trim - pitch * max`, `aileron = roll * max`, `rudder = -yaw * max`, where `max` is
/// the larger end of each surface's travel. Stick back (+pitch) is trailing edge up, which pitches
/// the nose up; right pedal is trailing edge right (negative), which yaws the nose right.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ManualLaw;

fn authority(min: f64, max: f64) -> f64 {
    min.abs().max(max.abs())
}

impl ControlLaw for ManualLaw {
    fn commands(&self, input: &LawInput, a: &Actuators) -> Surfaces {
        let c = &input.controls;
        Surfaces {
            elevator_rad: input.trim_pitch_rad
                - c.pitch * authority(a.elevator.min, a.elevator.max),
            aileron_rad: c.roll * authority(a.aileron.min, a.aileron.max),
            rudder_rad: -c.yaw * authority(a.rudder.min, a.rudder.max),
            lef_rad: 0.0,
            tef_rad: c.flaps * a.tef.max,
            airbrake: c.airbrake,
            gear: 0.0,
        }
    }
}

/// The trim after `dt_s` seconds of the trim switches, kept within the elevator's travel. Nose up
/// moves the elevator trailing edge up (negative).
#[must_use]
pub fn step_trim(
    trim_pitch_rad: f64,
    controls: &PilotControls,
    actuators: &Actuators,
    dt_s: f64,
) -> f64 {
    let dir = f64::from(i8::from(controls.trim_nose_down) - i8::from(controls.trim_nose_up));
    (trim_pitch_rad + dir * TRIM_RATE_RPS * dt_s)
        .clamp(actuators.elevator.min, actuators.elevator.max)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actuators::Actuator;

    fn actuators() -> Actuators {
        let dt = 1.0 / 120.0;
        let deg = |lo: f64, hi: f64, rate: f64| {
            Actuator::new(
                lo.to_radians(),
                hi.to_radians(),
                rate.to_radians(),
                0.05,
                dt,
            )
        };
        Actuators {
            elevator: deg(-25.0, 25.0, 60.0),
            aileron: deg(-20.0, 20.0, 80.0),
            rudder: deg(-30.0, 30.0, 120.0),
            lef: Actuator::fixed(),
            tef: deg(0.0, 30.0, 20.0),
            airbrake: Actuator::new(0.0, 1.0, 1.0, 0.3, dt),
            gear: Actuator::fixed(),
        }
    }

    #[test]
    fn manual_law_signs() {
        let a = actuators();
        let trim = -0.1;
        let cmd = |controls: PilotControls| {
            ManualLaw.commands(
                &LawInput {
                    controls,
                    trim_pitch_rad: trim,
                },
                &a,
            )
        };
        let neutral = cmd(PilotControls::default());
        assert_eq!(neutral.elevator_rad, trim, "hands off holds the trim");
        assert_eq!((neutral.aileron_rad, neutral.rudder_rad), (0.0, 0.0));
        // Stick back: trailing edge up, which is the nose-up direction (C_m,de < 0).
        let back = cmd(PilotControls {
            pitch: 1.0,
            ..PilotControls::default()
        });
        assert!((back.elevator_rad - (trim - 25f64.to_radians())).abs() < 1e-15);
        // Stick right: positive aileron, right wing down (C_l,da > 0).
        let right = cmd(PilotControls {
            roll: 0.5,
            ..PilotControls::default()
        });
        assert!((right.aileron_rad - 10f64.to_radians()).abs() < 1e-15);
        // Right pedal: trailing edge right (negative), nose right (C_n,dr < 0).
        let pedal = cmd(PilotControls {
            yaw: 1.0,
            ..PilotControls::default()
        });
        assert!((pedal.rudder_rad + 30f64.to_radians()).abs() < 1e-15);
        let flaps = cmd(PilotControls {
            flaps: 0.5,
            airbrake: 1.0,
            ..PilotControls::default()
        });
        assert!((flaps.tef_rad - 15f64.to_radians()).abs() < 1e-15);
        assert_eq!((flaps.airbrake, flaps.lef_rad, flaps.gear), (1.0, 0.0, 0.0));
    }

    #[test]
    fn trim_runs_at_two_degrees_per_second_within_the_travel() {
        let a = actuators();
        let up = PilotControls {
            trim_nose_up: true,
            ..PilotControls::default()
        };
        let mut trim = 0.0;
        for _ in 0..60 {
            trim = step_trim(trim, &up, &a, 1.0 / 60.0);
        }
        assert!(
            (trim + 2f64.to_radians()).abs() < 1e-12,
            "{}",
            trim.to_degrees()
        );
        let both = PilotControls {
            trim_nose_up: true,
            trim_nose_down: true,
            ..PilotControls::default()
        };
        assert_eq!(
            step_trim(trim, &both, &a, 1.0),
            trim,
            "both switches cancel"
        );
        let down = PilotControls {
            trim_nose_down: true,
            ..PilotControls::default()
        };
        assert_eq!(step_trim(0.0, &down, &a, 100.0), a.elevator.max);
    }

    #[test]
    fn sanitizing_clamps_and_neutralises_nan() {
        let wild = PilotControls {
            pitch: f64::NAN,
            roll: 3.0,
            yaw: -7.0,
            throttle: 1.5,
            flaps: -1.0,
            airbrake: f64::INFINITY,
            ..PilotControls::default()
        }
        .sanitized();
        assert_eq!(
            (
                wild.pitch,
                wild.roll,
                wild.yaw,
                wild.throttle,
                wild.flaps,
                wild.airbrake
            ),
            (0.0, 1.0, -1.0, 1.0, 0.0, 1.0)
        );
    }
}
