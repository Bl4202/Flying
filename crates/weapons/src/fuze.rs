//! The proximity fuze and the warhead (GDD 15.6, ADR-46).
//!
//! The fuze arms once the missile has flown `arm_dist_m` and `safe_sep_s` has passed. Each tick it
//! takes the missile's path relative to the target in the target's body axes and finds the closest
//! approach to the target's capsules (surface distance, 0 inside). It triggers at the first local
//! minimum of that distance within `trigger_radius_m`: when the closest point of the tick's path is
//! not its end (were it the end, the missile is still closing and the next tick comes closer).
//! The warhead then delivers `damage_j · (1 − d / lethal_radius_m)²` for a miss distance `d`.

use simmath::math::{DQuat, DVec3};

use crate::capsule::{Capsule, relative_segment, seg_seg_closest};
use crate::missile::{Missile, MissileParams};

/// Closest points this near the end of the tick's path count as the end. // ALGO: rounding
const END_EPS: f64 = 1e-9;

/// A detonation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Detonation {
    /// Where the missile was at the closest approach, BF, m.
    pub point_bf: DVec3,
    /// Its distance from the nearest capsule's surface there, m (0 inside one).
    pub miss_m: f64,
}

/// Whether the fuze is armed: flown at least `arm_dist_m` and at least `safe_sep_s` since launch.
#[must_use]
pub fn fuze_armed(m: &Missile, params: &MissileParams) -> bool {
    m.flown_m >= params.arm_dist_m && m.t_s >= params.launch.safe_sep_s
}

/// The fuze's check over one tick, for an armed fuze: the missile went from `m_prev` to `m_now`
/// while the target's reference point went from `t_prev` to `t_now`, ending at attitude `q_t`.
/// `None` if it does not trigger this tick.
#[must_use]
pub fn fuze_check(
    m_prev: DVec3,
    m_now: DVec3,
    t_prev: DVec3,
    t_now: DVec3,
    q_t: DQuat,
    caps: &[Capsule],
    params: &MissileParams,
) -> Option<Detonation> {
    let (s0, s1) = relative_segment(m_prev, m_now, t_prev, t_now, q_t);
    let mut best: Option<(f64, f64)> = None;
    for c in caps {
        let (s, _, dist) = seg_seg_closest(s0, s1, c.a, c.b);
        let d = (dist - c.radius).max(0.0);
        if best.is_none_or(|(bd, _)| d < bd) {
            best = Some((d, s));
        }
    }
    let (miss_m, s) = best?;
    let degenerate = (s1 - s0).length_squared() == 0.0;
    let at_minimum = degenerate || s < 1.0 - END_EPS;
    (miss_m.is_finite() && miss_m <= params.trigger_radius_m && at_minimum).then(|| Detonation {
        point_bf: m_prev.lerp(m_now, s),
        miss_m,
    })
}

/// The energy the warhead delivers to a target `miss_m` from it, J:
/// `damage_j · (1 − d / R)²` inside the lethal radius `R`, else 0.
#[must_use]
pub fn damage_energy(params: &MissileParams, miss_m: f64) -> f64 {
    if miss_m.is_nan() || miss_m >= params.lethal_radius_m {
        return 0.0;
    }
    let x = 1.0 - miss_m.max(0.0) / params.lethal_radius_m;
    params.damage_j * x * x
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::missile::{MissilePhase, launch};
    use crate::seeker::SeekerState;
    use crate::test_content;

    fn dart1() -> MissileParams {
        MissileParams::from_def(test_content::missile(), 1.0 / 60.0).expect("a missile")
    }

    fn kestrel_caps() -> Vec<Capsule> {
        let d = test_content::kestrel()
            .damage
            .as_ref()
            .expect("damage data");
        d.capsules.iter().map(Capsule::from_def).collect()
    }

    #[test]
    fn ms4_the_fuze_arms_after_300_m_and_half_a_second() {
        let p = dart1();
        let mut m = launch(
            &p,
            DVec3::ZERO,
            DVec3::new(600.0, 0.0, 0.0),
            SeekerState::searching(),
        );
        assert_eq!(m.phase, MissilePhase::Boost);
        for (flown, t, armed) in [
            (0.0, 0.0, false),
            (299.0, 2.0, false),
            (400.0, 0.49, false),
            (300.0, 0.5, true),
        ] {
            m.flown_m = flown;
            m.t_s = t;
            assert_eq!(fuze_armed(&m, &p), armed, "{flown} m, {t} s");
        }
    }

    #[test]
    fn ms4_detonation_at_the_swept_closest_approach() {
        let p = dart1();
        let caps = kestrel_caps();
        let q = DQuat::IDENTITY;
        // The target still at the origin (body = BF axes here); the missile crosses 6 m above the
        // fuselage's top (fuselage axis at z = 0.3, radius 1.1: its top is at z = −0.8, so the
        // path at z = −6.8 clears it by 6 m), from x = −20 to x = 20 in one tick.
        let d = fuze_check(
            DVec3::new(-20.0, 0.0, -6.8),
            DVec3::new(20.0, 0.0, -6.8),
            DVec3::ZERO,
            DVec3::ZERO,
            q,
            &caps,
            &p,
        )
        .expect("triggers");
        // The fin (from (−3.6, 0, −0.6) to (−5, 0, −2.2), radius 0.7) is the nearest: its top end
        // at z = −2.2 is 4.6 m below the path, less 0.7 m.
        assert!((d.miss_m - 3.9).abs() < 1e-6, "{}", d.miss_m);
        assert!(
            (d.point_bf - DVec3::new(-5.0, 0.0, -6.8)).length() < 1e-6,
            "{:?}",
            d.point_bf
        );
        // 8.3 m clear of the fin: no detonation.
        assert_eq!(
            fuze_check(
                DVec3::new(-20.0, 0.0, -11.2),
                DVec3::new(20.0, 0.0, -11.2),
                DVec3::ZERO,
                DVec3::ZERO,
                q,
                &caps,
                &p
            ),
            None
        );
        // Still closing at the end of the tick (the path stops short of the closest point): wait.
        assert_eq!(
            fuze_check(
                DVec3::new(-40.0, 0.0, -6.8),
                DVec3::new(-10.0, 0.0, -6.8),
                DVec3::ZERO,
                DVec3::ZERO,
                q,
                &caps,
                &p
            ),
            None
        );
        // The next tick contains the minimum.
        assert!(
            fuze_check(
                DVec3::new(-10.0, 0.0, -6.8),
                DVec3::new(20.0, 0.0, -6.8),
                DVec3::ZERO,
                DVec3::ZERO,
                q,
                &caps,
                &p
            )
            .is_some()
        );
        // Relative motion counts: a target moving with the missile is never approached.
        assert_eq!(
            fuze_check(
                DVec3::new(-40.0, 0.0, -6.8),
                DVec3::new(-10.0, 0.0, -6.8),
                DVec3::ZERO,
                DVec3::new(30.0, 0.0, 0.0),
                q,
                &caps,
                &p
            ),
            None
        );
        // Through the fuselage: a direct hit, miss 0.
        let hit = fuze_check(
            DVec3::new(0.0, -30.0, 0.3),
            DVec3::new(0.0, 30.0, 0.3),
            DVec3::ZERO,
            DVec3::ZERO,
            q,
            &caps,
            &p,
        )
        .expect("a hit");
        assert_eq!(hit.miss_m, 0.0);
    }

    #[test]
    fn ms4_warhead_damage_falls_with_the_square() {
        let p = dart1();
        assert_eq!(damage_energy(&p, 0.0), 2_548_000.0);
        assert_eq!(damage_energy(&p, 4.0), 637_000.0);
        assert_eq!(damage_energy(&p, 6.0), 159_250.0);
        assert_eq!(damage_energy(&p, 8.0), 0.0);
        assert_eq!(damage_energy(&p, 20.0), 0.0);
        assert_eq!(damage_energy(&p, f64::NAN), 0.0);
        // 400 hit points at 0 m against the Kestrel's 6 370 J per hit point.
        assert_eq!(damage_energy(&p, 0.0) / 6_370.0, 400.0);
    }
}
