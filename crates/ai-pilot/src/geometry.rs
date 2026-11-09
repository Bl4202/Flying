//! Engagement geometry (GDD 19.4.1, B.5): range, closure, the antenna train and aspect angles, the
//! lead-collision point and the classification of a geometry.
//!
//! ATA: the angle between a shooter's nose and its line of sight to the target. AA: the angle
//! between the target's tail and the line of sight from the target to the shooter (0 = the shooter
//! is dead astern, π = head on).

use simmath::dmath;
use simmath::math::DVec3;

/// The angle between two directions, rad; 0 when either has no length.
#[must_use]
pub fn angle_between(a: DVec3, b: DVec3) -> f64 {
    let (la, lb) = (a.length(), b.length());
    if !(la > 0.0 && lb > 0.0) {
        return 0.0;
    }
    dmath::acos((a.dot(b) / (la * lb)).clamp(-1.0, 1.0))
}

/// The distance between two points, m.
#[must_use]
pub fn range_m(own_pos: DVec3, tgt_pos: DVec3) -> f64 {
    (tgt_pos - own_pos).length()
}

/// The closing speed, m/s: `−Ṙ = −r · v_rel / |r|` with `r` from us to the target and `v_rel`
/// its velocity less ours. Positive while the range shrinks.
#[must_use]
pub fn closure_mps(own_pos: DVec3, own_vel: DVec3, tgt_pos: DVec3, tgt_vel: DVec3) -> f64 {
    let r = tgt_pos - own_pos;
    let len = r.length();
    if len.is_nan() || len <= 0.0 {
        return 0.0;
    }
    -r.dot(tgt_vel - own_vel) / len
}

/// The antenna train angle: between our nose and the line of sight to the target, rad.
#[must_use]
pub fn ata_rad(own_nose_bf: DVec3, own_pos: DVec3, tgt_pos: DVec3) -> f64 {
    angle_between(own_nose_bf, tgt_pos - own_pos)
}

/// The aspect angle of a target with nose (or velocity) `tgt_dir` seen from `own_pos`, rad:
/// between its tail and the line of sight from it to us.
#[must_use]
pub fn aa_rad(tgt_dir_bf: DVec3, tgt_pos: DVec3, own_pos: DVec3) -> f64 {
    angle_between(-tgt_dir_bf, own_pos - tgt_pos)
}

/// The lead-collision point (B.5): where a target at `tgt_pos` moving at `tgt_vel` will be when we,
/// flying straight at `own_speed` from `own_pos`, can meet it, and the time to it. Solves
/// `|r + v_t t| = V t` for the smallest positive `t`; `None` when there is no positive root (a
/// target too fast to catch).
#[must_use]
pub fn lead_point(
    own_pos: DVec3,
    own_speed: f64,
    tgt_pos: DVec3,
    tgt_vel: DVec3,
) -> Option<(DVec3, f64)> {
    let r = tgt_pos - own_pos;
    // a t² − 2 (r·v_t) t − r·r = 0 with a = V² − |v_t|².
    let a = own_speed * own_speed - tgt_vel.length_squared();
    let b = r.dot(tgt_vel);
    let c = r.length_squared();
    let t = if a.abs() < 1e-9 {
        // Equal speeds: one root, positive only when the target comes towards us.
        if b < 0.0 { -c / (2.0 * b) } else { -1.0 }
    } else {
        let disc = b * b + a * c;
        if disc < 0.0 {
            return None;
        }
        let root = dmath::sqrt(disc);
        let (t1, t2) = ((b - root) / a, (b + root) / a);
        match (t1 > 0.0, t2 > 0.0) {
            (true, true) => t1.min(t2),
            (true, false) => t1,
            (false, true) => t2,
            (false, false) => -1.0,
        }
    };
    (t > 0.0 && t.is_finite()).then(|| (tgt_pos + tgt_vel * t, t))
}

/// The geometry of an engagement as one side sees it (GDD 19.4.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Geometry {
    /// The target is inside our nose cone and we are behind it, in range.
    Offensive,
    /// The same, the other way round.
    Defensive,
    /// Nose to nose.
    HeadOn,
    /// Anything else.
    Neutral,
}

/// GDD 19.4.1's rule: offensive if our ATA < 60°, our AA < 90° and the range is inside `r_wez`;
/// defensive if the target meets the same on us; head-on if our AA > 150° and our ATA < 30°;
/// otherwise neutral. `own_ata` and `own_aa` are ours on the target, `tgt_ata` and `tgt_aa` the
/// target's on us.
#[must_use]
pub fn classify(
    own_ata: f64,
    own_aa: f64,
    tgt_ata: f64,
    tgt_aa: f64,
    range: f64,
    r_wez: f64,
) -> Geometry {
    // ALGO: GDD 19.4.1's thresholds, 60°, 90°, 150° and 30°.
    let (deg60, deg90) = (core::f64::consts::FRAC_PI_3, core::f64::consts::FRAC_PI_2);
    let (deg150, deg30) = (
        5.0 * core::f64::consts::FRAC_PI_6,
        core::f64::consts::FRAC_PI_6,
    );
    if own_ata < deg60 && own_aa < deg90 && range < r_wez {
        Geometry::Offensive
    } else if tgt_ata < deg60 && tgt_aa < deg90 && range < r_wez {
        Geometry::Defensive
    } else if own_aa > deg150 && own_ata < deg30 {
        Geometry::HeadOn
    } else {
        Geometry::Neutral
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(x: f64, y: f64, z: f64) -> DVec3 {
        DVec3::new(x, y, z)
    }

    #[test]
    fn range_closure_and_angles_worked_examples() {
        // We fly north at 200 m/s; the target is 1 000 m north and 1 000 m east, flying west at
        // 100 m/s.
        let (own, own_v) = (v(0.0, 0.0, 0.0), v(0.0, 200.0, 0.0));
        let (tgt, tgt_v) = (v(1_000.0, 1_000.0, 0.0), v(-100.0, 0.0, 0.0));
        assert!((range_m(own, tgt) - 1_414.213_562_373_095).abs() < 1e-9);
        // r = (1000, 1000), v_rel = (−100, −200): −r·v_rel / |r| = 300 000 / 1 414.2 = 212.1.
        assert!((closure_mps(own, own_v, tgt, tgt_v) - 212.132_034_355_964_27).abs() < 1e-9);
        // The target is 45° off our nose. Its tail points east and the line from it to us points
        // south-west, so AA = 135° (we are ahead of it and to its left).
        let deg = |x: f64| x.to_radians();
        assert!((ata_rad(v(0.0, 1.0, 0.0), own, tgt) - deg(45.0)).abs() < 1e-12);
        assert!((aa_rad(v(-1.0, 0.0, 0.0), tgt, own) - deg(135.0)).abs() < 1e-12);
        // Dead astern: AA 0; head on: AA 180.
        assert!(aa_rad(v(0.0, 1.0, 0.0), v(0.0, 500.0, 0.0), own).abs() < 1e-12);
        assert!((aa_rad(v(0.0, -1.0, 0.0), v(0.0, 500.0, 0.0), own) - deg(180.0)).abs() < 1e-12);
        assert_eq!(closure_mps(own, own_v, own, tgt_v), 0.0);
        assert_eq!(angle_between(DVec3::ZERO, v(1.0, 0.0, 0.0)), 0.0);
    }

    #[test]
    fn the_lead_point_worked_examples() {
        // A target 3 000 m east crossing north at 150 m/s; we fly at 250 m/s: a 3-4-5 triangle,
        // t = 3000 / 200 = 15 s (|(3000, 150 t)| = 250 t gives t² (62 500 − 22 500) = 9e6).
        let (p, t) = lead_point(DVec3::ZERO, 250.0, v(3_000.0, 0.0, 0.0), v(0.0, 150.0, 0.0))
            .expect("a lead point");
        assert!((t - 15.0).abs() < 1e-9, "{t}");
        assert!((p - v(3_000.0, 2_250.0, 0.0)).length() < 1e-6);
        // Head on at equal speeds: meet in the middle.
        let (p, t) = lead_point(
            DVec3::ZERO,
            200.0,
            v(0.0, 4_000.0, 0.0),
            v(0.0, -200.0, 0.0),
        )
        .expect("a meeting");
        assert!((t - 10.0).abs() < 1e-9 && (p - v(0.0, 2_000.0, 0.0)).length() < 1e-6);
        // A faster target running away: none; a slower one running away: caught.
        assert!(lead_point(DVec3::ZERO, 200.0, v(0.0, 1_000.0, 0.0), v(0.0, 300.0, 0.0)).is_none());
        let (_, t) = lead_point(DVec3::ZERO, 300.0, v(0.0, 1_000.0, 0.0), v(0.0, 200.0, 0.0))
            .expect("caught");
        assert!((t - 10.0).abs() < 1e-9, "{t}");
        // Equal speeds, the target running: none.
        assert!(lead_point(DVec3::ZERO, 200.0, v(0.0, 1_000.0, 0.0), v(0.0, 200.0, 0.0)).is_none());
    }

    #[test]
    fn geometry_classes() {
        let deg = |x: f64| x.to_radians();
        let wez = 2_000.0;
        assert_eq!(
            classify(deg(10.0), deg(20.0), deg(170.0), deg(160.0), 800.0, wez),
            Geometry::Offensive
        );
        assert_eq!(
            classify(deg(10.0), deg(20.0), deg(170.0), deg(160.0), 2_500.0, wez),
            Geometry::Neutral
        );
        assert_eq!(
            classify(deg(170.0), deg(160.0), deg(10.0), deg(20.0), 800.0, wez),
            Geometry::Defensive
        );
        assert_eq!(
            classify(deg(10.0), deg(170.0), deg(10.0), deg(170.0), 5_000.0, wez),
            Geometry::HeadOn
        );
        assert_eq!(
            classify(deg(90.0), deg(90.0), deg(90.0), deg(90.0), 800.0, wez),
            Geometry::Neutral
        );
    }
}
