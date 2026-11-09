//! The straight-and-level test pilot (ADR-47): holds the height, heading and calibrated speed it
//! has at its first tick. The scripts HOLD and `HOLD_FIRE` fly the player's aircraft with it (the
//! gates AI1 and MS5), and the bandit's `HoldCourse` brain is the same pilot.

use flight::controls::PilotControls;
use flight::env::FlightEnv;
use simmath::dmath;

use crate::maneuver::{Airframe, Goal, climb_dir, fly_goal, heading_dir, height_gamma, hold_speed};

/// Height error per radian of flight-path angle, m. // ALGO: ADR-47 (gentle: 100 m gives 0.05 rad)
pub const HEIGHT_PER_RAD_M: f64 = 2_000.0;
/// The steepest climb or descent it uses to hold a height, rad. // ALGO: ADR-47
pub const HEIGHT_GAMMA_MAX: f64 = 0.15;
/// The load it pulls at most, g. // ALGO: ADR-47 (gentle corrections only)
pub const HOLD_N_CAP: f64 = 2.0;

/// Straight and level at what it found.
#[allow(clippy::struct_excessive_bools, reason = "independent flags")]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct HoldCourse {
    /// The height it holds, m above MSL.
    pub alt_msl_m: f64,
    /// The heading it holds, rad (0 north, clockwise).
    pub heading_rad: f64,
    /// The calibrated speed it holds, m/s.
    pub cas_mps: f64,
    /// The speed hold's integral.
    pub speed_i: f64,
    /// It has taken its references.
    pub set: bool,
    /// `HOLD_FIRE`: press the missile button once, at the first tick the seeker is locked.
    pub fire_once: bool,
    /// It has pressed it.
    pub fired: bool,
}

impl HoldCourse {
    /// A pilot that takes its references at its first tick.
    #[must_use]
    pub const fn new(fire_once: bool) -> Self {
        Self {
            alt_msl_m: 0.0,
            heading_rad: 0.0,
            cas_mps: 0.0,
            speed_i: 0.0,
            set: false,
            fire_once,
            fired: false,
        }
    }

    /// One tick: the controls that hold the references (taken now, the first time).
    pub fn step<E: FlightEnv>(&mut self, af: &Airframe<'_, E>) -> PilotControls {
        let s = af.sensed();
        let up = af.up();
        let height = af.env.height_msl_m(af.ac.body.pos_bf);
        if !self.set {
            let v = af.ac.body.vel_bf;
            self.alt_msl_m = height;
            self.heading_rad = dmath::atan2(v.x, v.y);
            self.cas_mps = s.cas;
            self.set = true;
        }
        let gamma = height_gamma(self.alt_msl_m, height, HEIGHT_PER_RAD_M, HEIGHT_GAMMA_MAX);
        let dir = climb_dir(heading_dir(self.heading_rad, up), up, gamma);
        let throttle = hold_speed(
            self.cas_mps,
            s.cas,
            &mut self.speed_i,
            af.fcs.tick_dt_s,
            1.0,
        );
        fly_goal(
            &Goal {
                dir_bf: dir,
                n_cap: HOLD_N_CAP,
                throttle,
            },
            af,
        )
    }

    /// `HOLD_FIRE`'s trigger: true once, at the first call with the seeker locked.
    pub fn fire(&mut self, locked: bool) -> bool {
        let press = self.fire_once && !self.fired && locked;
        self.fired |= press;
        press
    }

    /// Its numbers as words, for the state hash.
    #[must_use]
    pub fn words(&self) -> [u64; 5] {
        [
            self.alt_msl_m.to_bits(),
            self.heading_rad.to_bits(),
            self.cas_mps.to_bits(),
            self.speed_i.to_bits(),
            u64::from(self.set) | u64::from(self.fire_once) << 1 | u64::from(self.fired) << 2,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{Cockpit, DT};
    use fcs::Assist;
    use fcs::tester::Presses;
    use simmath::math::DVec3;

    #[test]
    fn it_holds_what_it_found_for_two_minutes() {
        // Heading 090 at 4 000 m and 200 m/s, a little disturbed: after 120 s it is within 30 m,
        // 1° and 3 m/s of where it began.
        for assist in [Assist::Standard, Assist::Manual] {
            let mut ck = Cockpit::air(
                assist,
                DVec3::new(0.0, 0.0, 4_000.0),
                core::f64::consts::FRAC_PI_2,
                200.0,
                10_260.0,
            );
            let mut hold = HoldCourse::new(false);
            let cas0 = ck.airframe().sensed().cas;
            for k in 0..(120.0 / DT) as u32 {
                let mut c = hold.step(&ck.airframe());
                if k < 30 {
                    c.roll = 0.3; // A gust of roll stick for half a second.
                }
                ck.step(&c, Presses::default());
            }
            assert!((hold.alt_msl_m - 4_000.0).abs() < 1.0 && hold.set);
            assert!(
                (ck.height() - 4_000.0).abs() < 30.0,
                "{assist:?}: {} m",
                ck.height()
            );
            assert!(
                (ck.track() - core::f64::consts::FRAC_PI_2).abs() < 1f64.to_radians(),
                "{assist:?}: {}",
                ck.track().to_degrees()
            );
            assert!(
                (ck.airframe().sensed().cas - cas0).abs() < 3.0,
                "{assist:?}"
            );
        }
    }

    #[test]
    fn hold_fire_presses_once_when_locked() {
        let mut h = HoldCourse::new(true);
        assert!(!h.fire(false));
        assert!(h.fire(true));
        assert!(!h.fire(true));
        let mut plain = HoldCourse::new(false);
        assert!(!plain.fire(true));
        let mut moved = h;
        moved.speed_i = 1.0;
        assert_ne!(h.words(), moved.words());
        assert_ne!(h.words(), plain.words());
    }
}
