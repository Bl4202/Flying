//! The runway frame and the circuit the virtual tester flies (ADR-33).
//!
//! Positions are measured in the runway's own frame: `along` from the threshold in the landing
//! direction, `across` to the right of it. The circuit is a left-hand rectangle:
//!
//! ```text
//!                 crosswind
//!        P2 <------------------- P1          ^ along
//!        |                       ^           |
//! down-  |                       | climb-out +--> across
//! wind   v                       |
//!        P3 -------------------> P4 ---- final ----> threshold
//!                  base
//! ```
//!
//! Each corner is flown "by", not "over": the next leg starts when the distance to go on this one
//! falls below the turn's own radius times `tan(half the turn)`.

use simmath::dmath;
use simmath::math::DVec3;

/// The runway as the tester sees it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Runway {
    /// The centre of the threshold on the ground, BF m.
    pub threshold_bf: DVec3,
    /// The direction of landing and take-off, rad (0 is north, clockwise positive).
    pub heading_rad: f64,
    /// Length, m.
    pub length_m: f64,
    /// Width, m.
    pub width_m: f64,
}

/// A point in the runway frame, m.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RunwayPoint {
    /// Distance from the threshold along the runway (negative before it).
    pub along: f64,
    /// Distance to the right of the centreline.
    pub across: f64,
}

impl Runway {
    /// The runway frame coordinates of a BF position.
    #[must_use]
    pub fn locate(&self, p: DVec3) -> RunwayPoint {
        let (s, c) = (dmath::sin(self.heading_rad), dmath::cos(self.heading_rad));
        let (de, dn) = (p.x - self.threshold_bf.x, p.y - self.threshold_bf.y);
        RunwayPoint {
            along: de * s + dn * c,
            across: de * c - dn * s,
        }
    }

    /// The direction (as a heading, rad) of a vector given in the runway frame.
    #[must_use]
    pub fn heading_of(&self, along: f64, across: f64) -> f64 {
        self.heading_rad + dmath::atan2(across, along)
    }

    /// The rate of change of a position in the runway frame for a BF velocity: `(d along, d
    /// across)`, m/s.
    #[must_use]
    pub fn velocity(&self, v: DVec3) -> (f64, f64) {
        let (s, c) = (dmath::sin(self.heading_rad), dmath::cos(self.heading_rad));
        (v.x * s + v.y * c, v.x * c - v.y * s)
    }
}

/// An angle wrapped to `(-pi, pi]`.
#[must_use]
pub fn wrap(a: f64) -> f64 {
    let two_pi = 2.0 * core::f64::consts::PI;
    let mut x = a % two_pi;
    if x > core::f64::consts::PI {
        x -= two_pi;
    } else if x <= -core::f64::consts::PI {
        x += two_pi;
    }
    x
}

/// One straight leg of the circuit: from `from` to `to` in the runway frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Leg {
    /// Where the leg starts.
    pub from: RunwayPoint,
    /// Where it ends (the corner).
    pub to: RunwayPoint,
}

impl Leg {
    /// Its length, m.
    #[must_use]
    pub fn length(&self) -> f64 {
        let (da, dc) = (
            self.to.along - self.from.along,
            self.to.across - self.from.across,
        );
        dmath::sqrt(da * da + dc * dc)
    }

    /// Its unit direction in the runway frame, `(along, across)`.
    #[must_use]
    pub fn direction(&self) -> (f64, f64) {
        let l = self.length().max(1.0);
        (
            (self.to.along - self.from.along) / l,
            (self.to.across - self.from.across) / l,
        )
    }

    /// Where a position is against the leg: `(cross-track error, distance to go)`, m. The error
    /// is positive to the right of the direction of flight.
    #[must_use]
    pub fn track(&self, p: RunwayPoint) -> (f64, f64) {
        let (ua, uc) = self.direction();
        let (da, dc) = (p.along - self.from.along, p.across - self.from.across);
        // The right-hand normal of (ua, uc) in the frame (along north, across east) is
        // (-uc, ua) rotated: for a heading of 0 (along) the right is `across` positive.
        let err = -da * uc + dc * ua;
        let gone = da * ua + dc * uc;
        (err, self.length() - gone)
    }
}

/// The five legs of the circuit, left-hand: climb-out, crosswind, downwind, base and final.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Circuit {
    /// Where the climb-out turns onto the crosswind, along the runway, m.
    pub corner_along_m: f64,
    /// How far the downwind leg is from the runway, m.
    pub width_m: f64,
    /// How far before the threshold the base leg is, m.
    pub base_before_m: f64,
}

impl Circuit {
    /// The legs, in order.
    #[must_use]
    pub fn legs(&self) -> [Leg; 5] {
        let p = |along: f64, across: f64| RunwayPoint { along, across };
        let (a1, w, af) = (self.corner_along_m, self.width_m, self.base_before_m);
        [
            Leg {
                from: p(0.0, 0.0),
                to: p(a1, 0.0),
            },
            Leg {
                from: p(a1, 0.0),
                to: p(a1, -w),
            },
            Leg {
                from: p(a1, -w),
                to: p(-af, -w),
            },
            Leg {
                from: p(-af, -w),
                to: p(-af, 0.0),
            },
            Leg {
                from: p(-af, 0.0),
                to: p(0.0, 0.0),
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn runway() -> Runway {
        Runway {
            threshold_bf: DVec3::new(0.0, -6_000.0, 40.0),
            heading_rad: 0.0,
            length_m: 3_000.0,
            width_m: 45.0,
        }
    }

    #[test]
    fn the_runway_frame_is_along_north_and_across_east_for_a_northerly_runway() {
        let r = runway();
        let p = r.locate(DVec3::new(30.0, -5_000.0, 0.0));
        assert!((p.along - 1_000.0).abs() < 1e-9 && (p.across - 30.0).abs() < 1e-9);
        // The same place for a runway heading east: along is east, across is south.
        let e = Runway {
            heading_rad: core::f64::consts::FRAC_PI_2,
            threshold_bf: DVec3::ZERO,
            ..r
        };
        let p = e.locate(DVec3::new(500.0, -20.0, 0.0));
        assert!((p.along - 500.0).abs() < 1e-9 && (p.across - 20.0).abs() < 1e-9);
        let (da, dc) = e.velocity(DVec3::new(0.0, -10.0, 0.0));
        assert!((da - 0.0).abs() < 1e-9 && (dc - 10.0).abs() < 1e-9);
    }

    #[test]
    fn a_heading_in_the_runway_frame_is_a_bearing() {
        let r = runway();
        assert!(r.heading_of(1.0, 0.0).abs() < 1e-12, "along");
        assert!(
            (r.heading_of(0.0, 1.0) - core::f64::consts::FRAC_PI_2).abs() < 1e-12,
            "right"
        );
        assert!(
            (r.heading_of(-1.0, 0.0).abs() - core::f64::consts::PI).abs() < 1e-12,
            "back"
        );
    }

    #[test]
    fn angles_wrap_to_the_half_turn() {
        assert!((wrap(3.5 * core::f64::consts::PI) + 0.5 * core::f64::consts::PI).abs() < 1e-12);
        assert!((wrap(-3.5 * core::f64::consts::PI) - 0.5 * core::f64::consts::PI).abs() < 1e-12);
        assert!((wrap(core::f64::consts::PI) - core::f64::consts::PI).abs() < 1e-12);
    }

    #[test]
    fn the_cross_track_error_is_positive_to_the_right_of_the_flight_direction() {
        let c = Circuit {
            corner_along_m: 4_000.0,
            width_m: 3_000.0,
            base_before_m: 3_000.0,
        };
        let legs = c.legs();
        // Climb-out heading along: a point to the right (across > 0) is right of track.
        let (e, to_go) = legs[0].track(RunwayPoint {
            along: 1_000.0,
            across: 50.0,
        });
        assert!((e - 50.0).abs() < 1e-9 && (to_go - 3_000.0).abs() < 1e-9);
        // Crosswind heads to negative across: right of track is increasing along.
        let (e, to_go) = legs[1].track(RunwayPoint {
            along: 4_100.0,
            across: -1_000.0,
        });
        assert!((e - 100.0).abs() < 1e-9 && (to_go - 2_000.0).abs() < 1e-9);
        // Downwind heads to negative along: right of track is negative across (further out).
        let (e, _) = legs[2].track(RunwayPoint {
            along: 0.0,
            across: -3_100.0,
        });
        assert!(e > 0.0, "{e}");
        // Final: back to the start.
        let (e, to_go) = legs[4].track(RunwayPoint {
            along: -1_000.0,
            across: -20.0,
        });
        assert!((e + 20.0).abs() < 1e-9 && (to_go - 1_000.0).abs() < 1e-9);
    }
}
