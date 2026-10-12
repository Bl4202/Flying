//! `MissileModel`: how a missile steers and when it fuzes (GDD 17, 61.5). The first
//! implementation is true proportional navigation with an acceleration limit and a swept
//! closest-approach fuze; seeker physics, motor curves and countermeasure response replace the
//! stubs around it in M3.

use crate::math::DVec3;

/// Position and velocity of a body, BF.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Kinematics {
    /// Position, metres.
    pub pos: DVec3,
    /// Velocity, metres per second.
    pub vel: DVec3,
}

/// Guidance and fuzing for one missile.
pub trait MissileModel: Send {
    /// Commanded acceleration, BF metres per second squared, never longer than
    /// [`MissileModel::max_accel_mps2`]. Must be finite for any input.
    fn guide(&self, missile: &Kinematics, target: &Kinematics) -> DVec3;

    /// Largest acceleration the airframe can pull, metres per second squared.
    fn max_accel_mps2(&self) -> f64;

    /// True if, during the next `dt` seconds on straight-line motion, the missile comes within
    /// its lethal radius of the target. Must not be fooled by tunnelling at high closing speed.
    fn fuze(&self, missile: &Kinematics, target: &Kinematics, dt: f64) -> bool;
}

/// True proportional navigation: `a = N * Vc * (omega x u_los)`, where `omega` is the
/// line-of-sight rotation rate, `Vc` the closing speed and `u_los` the line-of-sight unit
/// vector. The command is perpendicular to the line of sight and zero on a collision course.
#[derive(Clone, Copy, Debug)]
pub struct ProportionalNav {
    /// Navigation constant, usually 3 to 5.
    pub nav_constant: f64,
    /// Acceleration limit, metres per second squared.
    pub max_accel_mps2: f64,
    /// Fuze radius, metres.
    pub lethal_radius_m: f64,
}

impl ProportionalNav {
    /// A short-range missile: N = 4, 40 g, 9 m lethal radius.
    pub const SHORT_RANGE: Self = Self {
        nav_constant: 4.0,
        max_accel_mps2: 392.4,
        lethal_radius_m: 9.0,
    };
}

impl MissileModel for ProportionalNav {
    fn guide(&self, missile: &Kinematics, target: &Kinematics) -> DVec3 {
        let r = target.pos - missile.pos;
        let v = target.vel - missile.vel;
        let r2 = r.length_squared();
        if !(r2 > 1e-12 && r2.is_finite() && v.is_finite()) {
            return DVec3::ZERO;
        }
        let range = r.length();
        let los = r * (1.0 / range);
        let omega = r.cross(v) * (1.0 / r2);
        let closing = (-r.dot(v) / range).max(0.0);
        let a = omega.cross(los) * (self.nav_constant * closing);
        // `omega x los` points the way the line of sight is turning; PN accelerates that way.
        // (Sign check: a target crossing to the left of the line of sight gives omega up and
        // `omega x los` to the left.)
        limit(a, self.max_accel_mps2)
    }

    fn max_accel_mps2(&self) -> f64 {
        self.max_accel_mps2
    }

    fn fuze(&self, missile: &Kinematics, target: &Kinematics, dt: f64) -> bool {
        let r = target.pos - missile.pos;
        let v = target.vel - missile.vel;
        if !(r.is_finite() && v.is_finite() && dt >= 0.0 && dt.is_finite()) {
            return false;
        }
        // Closest approach of the relative motion over [0, dt].
        let v2 = v.length_squared();
        let s = if v2 > 0.0 {
            (-r.dot(v) / v2).clamp(0.0, dt)
        } else {
            0.0
        };
        (r + v * s).length() <= self.lethal_radius_m
    }
}

/// Scales `a` down to at most `max` long; zero if `a` is not finite.
fn limit(a: DVec3, max: f64) -> DVec3 {
    if !a.is_finite() {
        return DVec3::ZERO;
    }
    let len = a.length();
    if len > max { a * (max / len) } else { a }
}

/// Behaviour every `MissileModel` must have (`AC-61.5`).
///
/// # Panics
/// Panics, with a message naming the property, if the implementation violates the contract.
pub fn contract(m: &dyn MissileModel) {
    let k = |x: f64, y: f64, z: f64, vx: f64, vy: f64, vz: f64| Kinematics {
        pos: DVec3::new(x, y, z),
        vel: DVec3::new(vx, vy, vz),
    };
    let max = m.max_accel_mps2();
    assert!(
        max > 0.0 && max.is_finite(),
        "a positive acceleration limit"
    );

    // The command is finite, repeatable and within the limit, for crossing, tail-chase and
    // head-on geometries.
    let cases = [
        (
            k(0.0, 0.0, 3_000.0, 0.0, 600.0, 0.0),
            k(2_000.0, 6_000.0, 3_200.0, -200.0, 0.0, 0.0),
        ),
        (
            k(0.0, 0.0, 3_000.0, 0.0, 600.0, 0.0),
            k(0.0, 4_000.0, 3_000.0, 0.0, 250.0, 0.0),
        ),
        (
            k(0.0, 0.0, 3_000.0, 0.0, 600.0, 0.0),
            k(100.0, 9_000.0, 3_000.0, 0.0, -250.0, 0.0),
        ),
    ];
    for (mis, tgt) in &cases {
        let a = m.guide(mis, tgt);
        assert!(a.is_finite(), "the command is finite");
        assert!(
            a.length() <= max * (1.0 + 1e-12),
            "the command respects the limit"
        );
        assert_eq!(
            a,
            m.guide(mis, tgt),
            "guidance is a pure function of the state"
        );
    }

    // Degenerate input must not produce NaN: coincident bodies, non-finite state.
    let same = k(5.0, 5.0, 5.0, 1.0, 2.0, 3.0);
    assert!(m.guide(&same, &same).is_finite(), "coincident bodies");
    let bad = k(f64::NAN, 0.0, 0.0, 0.0, 0.0, 0.0);
    assert!(
        m.guide(&bad, &cases[0].1).is_finite(),
        "non-finite missile state"
    );
    assert!(
        m.guide(&cases[0].0, &bad).is_finite(),
        "non-finite target state"
    );
    assert!(
        !m.fuze(&bad, &cases[0].1, 1.0 / 60.0),
        "a non-finite state never fuzes"
    );

    // The fuze sees a pass-through that a point-in-time distance check would miss: 3 000 m/s
    // closing speed moves 50 m per tick, far more than any plausible lethal radius.
    let fast = k(0.0, 0.0, 0.0, 0.0, 1_500.0, 0.0);
    let through = k(0.0, 25.0, 0.0, 0.0, -1_500.0, 0.0);
    assert!(
        m.fuze(&fast, &through, 1.0 / 60.0),
        "no tunnelling at high closing speed"
    );
    let far = k(10_000.0, 25.0, 0.0, 0.0, -1_500.0, 0.0);
    assert!(
        !m.fuze(&fast, &far, 1.0 / 60.0),
        "a distant target does not fuze"
    );
    assert!(!m.fuze(&fast, &through, -1.0), "negative time never fuzes");

    // Steering towards a stationary target on a collision course needs no correction.
    let course = m.guide(
        &k(0.0, 0.0, 0.0, 0.0, 500.0, 0.0),
        &k(0.0, 8_000.0, 0.0, 0.0, 0.0, 0.0),
    );
    assert!(course.length() < 1e-6, "on course: {course:?}");
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f64 = 1.0 / 60.0;

    /// Flies a constant-speed missile against a straight-flying target; returns the time of the
    /// fuze, or `None` if it never fires within `limit_s`.
    fn engage(
        m: &ProportionalNav,
        mut mis: Kinematics,
        mut tgt: Kinematics,
        limit_s: f64,
    ) -> Option<f64> {
        let speed = mis.vel.length();
        let mut t = 0.0;
        while t < limit_s {
            if m.fuze(&mis, &tgt, DT) {
                return Some(t);
            }
            let a = m.guide(&mis, &tgt);
            let v = mis.vel + a * DT;
            mis.vel = v * (speed / v.length());
            mis.pos = mis.pos + mis.vel * DT;
            tgt.pos = tgt.pos + tgt.vel * DT;
            t += DT;
        }
        None
    }

    #[test]
    fn proportional_navigation_passes_the_contract() {
        contract(&ProportionalNav::SHORT_RANGE);
    }

    #[test]
    fn the_command_is_perpendicular_to_the_line_of_sight() {
        let m = ProportionalNav::SHORT_RANGE;
        let mis = Kinematics {
            pos: DVec3::ZERO,
            vel: DVec3::new(0.0, 600.0, 0.0),
        };
        let tgt = Kinematics {
            pos: DVec3::new(1_500.0, 5_000.0, 400.0),
            vel: DVec3::new(-220.0, 40.0, 0.0),
        };
        let a = m.guide(&mis, &tgt);
        let los = (tgt.pos - mis.pos).normalized_or(DVec3::Y);
        assert!(a.length() > 1.0, "a crossing target needs steering");
        assert!(
            a.dot(los).abs() < 1e-9 * a.length(),
            "a . los = {}",
            a.dot(los)
        );
    }

    #[test]
    fn it_steers_towards_where_the_target_is_going() {
        // The target crosses from the right of the nose to the left (towards -x): the missile
        // must turn left.
        let m = ProportionalNav::SHORT_RANGE;
        let mis = Kinematics {
            pos: DVec3::ZERO,
            vel: DVec3::new(0.0, 600.0, 0.0),
        };
        let tgt = Kinematics {
            pos: DVec3::new(300.0, 6_000.0, 0.0),
            vel: DVec3::new(-250.0, 0.0, 0.0),
        };
        assert!(m.guide(&mis, &tgt).x < 0.0);
    }

    #[test]
    fn it_intercepts_crossing_and_tail_chase_targets() {
        let m = ProportionalNav::SHORT_RANGE;
        let mis = Kinematics {
            pos: DVec3::new(0.0, 0.0, 3_000.0),
            vel: DVec3::new(0.0, 600.0, 0.0),
        };
        let crossing = Kinematics {
            pos: DVec3::new(4_000.0, 7_000.0, 3_200.0),
            vel: DVec3::new(-250.0, 0.0, 0.0),
        };
        let chase = Kinematics {
            pos: DVec3::new(500.0, 5_000.0, 3_000.0),
            vel: DVec3::new(120.0, 220.0, 20.0),
        };
        let t = engage(&m, mis, crossing, 40.0).expect("intercepts a crossing target");
        assert!((5.0..20.0).contains(&t), "time to go {t}");
        assert!(
            engage(&m, mis, chase, 40.0).is_some(),
            "intercepts a fleeing target"
        );
    }

    #[test]
    fn a_target_that_outruns_the_missile_is_never_caught() {
        let m = ProportionalNav::SHORT_RANGE;
        let mis = Kinematics {
            pos: DVec3::ZERO,
            vel: DVec3::new(0.0, 300.0, 0.0),
        };
        let tgt = Kinematics {
            pos: DVec3::new(0.0, 3_000.0, 0.0),
            vel: DVec3::new(0.0, 400.0, 0.0),
        };
        assert!(engage(&m, mis, tgt, 20.0).is_none());
    }

    #[test]
    fn the_limit_is_enforced() {
        let m = ProportionalNav {
            max_accel_mps2: 10.0,
            ..ProportionalNav::SHORT_RANGE
        };
        let mis = Kinematics {
            pos: DVec3::ZERO,
            vel: DVec3::new(0.0, 900.0, 0.0),
        };
        let tgt = Kinematics {
            pos: DVec3::new(100.0, 800.0, 0.0),
            vel: DVec3::new(-300.0, 0.0, 0.0),
        };
        assert!((m.guide(&mis, &tgt).length() - 10.0).abs() < 1e-9);
    }
}
