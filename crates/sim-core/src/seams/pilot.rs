//! `PilotBrain`: the controls an AI pilot would give an aircraft (GDD 15, 61.5). The first
//! implementation holds a compass heading; behaviour trees and the tactical layer replace it in
//! M3.

use crate::dmath;
use crate::math::DVec3;

/// What a pilot can see of its own aircraft.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PilotView {
    /// Velocity, BF (x east, y north, z up), metres per second.
    pub vel_bf: DVec3,
    /// Bank angle, radians, positive right wing down.
    pub bank_rad: f64,
}

/// The controls the pilot asks for, each in its natural range.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PilotCommand {
    /// Roll command, `[-1, 1]`, positive rolls right.
    pub roll: f64,
    /// Pitch command, `[-1, 1]`, positive pulls up.
    pub pitch: f64,
    /// Throttle, `[0, 1]`.
    pub throttle: f64,
}

impl PilotCommand {
    /// Hands off, half throttle.
    pub const NEUTRAL: Self = Self {
        roll: 0.0,
        pitch: 0.0,
        throttle: 0.5,
    };
}

/// An AI pilot.
pub trait PilotBrain: Send {
    /// Decides the controls for the next tick. Must return a finite command inside the stated
    /// ranges for any input.
    fn decide(&mut self, view: &PilotView) -> PilotCommand;
}

/// Compass heading of a velocity: 0 north, clockwise, in `[0, 2*pi)`. Zero if there is no
/// horizontal motion.
#[must_use]
pub fn compass_heading_rad(vel_bf: DVec3) -> f64 {
    if !(vel_bf.is_finite() && (vel_bf.x != 0.0 || vel_bf.y != 0.0)) {
        return 0.0;
    }
    let h = dmath::atan2(vel_bf.x, vel_bf.y);
    if h < 0.0 {
        h + core::f64::consts::TAU
    } else {
        h
    }
}

/// Smallest signed angle from `from` to `to`, in `(-pi, pi]`.
#[must_use]
pub fn wrap_pi(mut a: f64) -> f64 {
    use core::f64::consts::{PI, TAU};
    if !a.is_finite() {
        return 0.0;
    }
    while a > PI {
        a -= TAU;
    }
    while a <= -PI {
        a += TAU;
    }
    a
}

/// Holds a compass heading by banking towards it, at a steady throttle.
#[derive(Clone, Copy, Debug)]
pub struct HoldCourse {
    /// Heading to hold, radians, 0 north, clockwise.
    pub heading_rad: f64,
    /// Throttle to hold, `[0, 1]`.
    pub throttle: f64,
    /// Roll command per radian of heading error.
    pub gain: f64,
}

impl HoldCourse {
    /// Holds `heading_rad` at half throttle.
    #[must_use]
    pub fn new(heading_rad: f64) -> Self {
        Self {
            heading_rad,
            throttle: 0.5,
            gain: 1.5,
        }
    }
}

impl PilotBrain for HoldCourse {
    fn decide(&mut self, view: &PilotView) -> PilotCommand {
        if !(view.vel_bf.is_finite() && view.bank_rad.is_finite() && self.heading_rad.is_finite()) {
            return PilotCommand::NEUTRAL;
        }
        // A target to the right (positive error) wants a right bank: positive roll.
        let error = wrap_pi(self.heading_rad - compass_heading_rad(view.vel_bf));
        PilotCommand {
            roll: (self.gain * error).clamp(-1.0, 1.0),
            pitch: 0.0,
            throttle: self.throttle.clamp(0.0, 1.0),
        }
    }
}

/// Behaviour every `PilotBrain` must have (`AC-61.5`). `fresh` makes a new pilot.
///
/// # Panics
/// Panics, with a message naming the property, if the implementation violates the contract.
pub fn contract(fresh: &dyn Fn() -> Box<dyn PilotBrain>) {
    let views: Vec<PilotView> = (0..64)
        .map(|i| {
            let a = f64::from(i) * 0.2;
            PilotView {
                vel_bf: DVec3::new(
                    250.0 * dmath::sin(a),
                    250.0 * dmath::cos(a),
                    5.0 * dmath::sin(3.0 * a),
                ),
                bank_rad: 0.4 * dmath::sin(a),
            }
        })
        .collect();

    let mut a = fresh();
    let mut b = fresh();
    for v in &views {
        let ca = a.decide(v);
        let cb = b.decide(v);
        assert_eq!(
            ca, cb,
            "two fresh pilots given the same views give the same commands"
        );
        assert!(
            ca.roll.is_finite() && ca.pitch.is_finite() && ca.throttle.is_finite(),
            "finite command"
        );
        assert!(
            (-1.0..=1.0).contains(&ca.roll),
            "roll in range: {}",
            ca.roll
        );
        assert!(
            (-1.0..=1.0).contains(&ca.pitch),
            "pitch in range: {}",
            ca.pitch
        );
        assert!(
            (0.0..=1.0).contains(&ca.throttle),
            "throttle in range: {}",
            ca.throttle
        );
    }

    // Garbage in, a safe command out.
    let mut p = fresh();
    for bad in [
        PilotView {
            vel_bf: DVec3::new(f64::NAN, 0.0, 0.0),
            bank_rad: 0.0,
        },
        PilotView {
            vel_bf: DVec3::ZERO,
            bank_rad: f64::INFINITY,
        },
        PilotView {
            vel_bf: DVec3::ZERO,
            bank_rad: 0.0,
        },
    ] {
        let c = p.decide(&bad);
        assert!(
            c.roll.is_finite()
                && (-1.0..=1.0).contains(&c.roll)
                && (0.0..=1.0).contains(&c.throttle),
            "safe command for {bad:?}: {c:?}"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::f64::consts::{FRAC_PI_2, PI};

    fn flying(heading: f64) -> PilotView {
        PilotView {
            vel_bf: DVec3::new(
                250.0 * dmath::sin(heading),
                250.0 * dmath::cos(heading),
                0.0,
            ),
            bank_rad: 0.0,
        }
    }

    #[test]
    fn compass_headings_follow_the_navigation_convention() {
        assert!(
            (compass_heading_rad(DVec3::new(0.0, 1.0, 0.0))).abs() < 1e-12,
            "north"
        );
        assert!(
            (compass_heading_rad(DVec3::new(1.0, 0.0, 0.0)) - FRAC_PI_2).abs() < 1e-12,
            "east"
        );
        assert!(
            (compass_heading_rad(DVec3::new(0.0, -1.0, 0.0)) - PI).abs() < 1e-12,
            "south"
        );
        assert!(
            (compass_heading_rad(DVec3::new(-1.0, 0.0, 0.0)) - 3.0 * FRAC_PI_2).abs() < 1e-12,
            "west"
        );
        assert_eq!(
            compass_heading_rad(DVec3::new(0.0, 0.0, 50.0)),
            0.0,
            "straight up has no heading"
        );
    }

    #[test]
    fn wrapping_takes_the_short_way_round() {
        assert!((wrap_pi(3.0 * FRAC_PI_2) + FRAC_PI_2).abs() < 1e-12);
        assert!((wrap_pi(-3.0 * FRAC_PI_2) - FRAC_PI_2).abs() < 1e-12);
        assert!((wrap_pi(PI) - PI).abs() < 1e-12, "+pi stays +pi");
        assert_eq!(wrap_pi(f64::NAN), 0.0);
    }

    #[test]
    fn hold_course_passes_the_contract() {
        contract(&|| Box::new(HoldCourse::new(1.0)));
    }

    #[test]
    fn it_banks_the_short_way_and_levels_on_course() {
        let mut p = HoldCourse::new(FRAC_PI_2); // east
        assert!(
            p.decide(&flying(0.0)).roll > 0.5,
            "north to east: right bank"
        );
        assert!(
            p.decide(&flying(PI)).roll > 0.0 || p.decide(&flying(PI)).roll < 0.0,
            "south: a turn either way"
        );
        assert!(p.decide(&flying(PI)).roll.abs() > 0.5);
        let mut west = HoldCourse::new(3.0 * FRAC_PI_2 + 0.1);
        assert!(
            west.decide(&flying(0.0)).roll < 0.0,
            "north to west-north-west: left bank"
        );
        assert!(
            p.decide(&flying(FRAC_PI_2)).roll.abs() < 1e-9,
            "on course: wings level"
        );
    }
}
