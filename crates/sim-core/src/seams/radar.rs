//! `RadarModel`: does a radar see a target? (GDD 16, 61.5). The first implementations are "sees
//! nothing" and a range/RCS model from the radar range equation; a full emitter model with
//! clutter, jamming and ESM replaces them in M4.

use crate::dmath;
use crate::math::DVec3;

/// One detection question.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RadarQuery {
    /// Radar position, BF.
    pub radar_pos: DVec3,
    /// Target position, BF.
    pub target_pos: DVec3,
    /// Target radar cross-section, square metres.
    pub target_rcs_m2: f64,
    /// Whether the line of sight is clear (from a [`crate::seams::los::LosService`]).
    pub line_of_sight: bool,
}

/// The answer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Detection {
    /// True if the target is seen.
    pub detected: bool,
    /// Distance between radar and target, metres (0 if the input was unusable).
    pub range_m: f64,
}

/// Radar detection of one target by one radar.
pub trait RadarModel: Send {
    /// Whether the radar sees the target.
    fn detect(&self, q: &RadarQuery) -> Detection;
}

/// A radar that never sees anything.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoRadar;

impl RadarModel for NoRadar {
    fn detect(&self, q: &RadarQuery) -> Detection {
        Detection {
            detected: false,
            range_m: sane_range(q),
        }
    }
}

/// Detection range scales with the fourth root of the cross-section (the radar range equation
/// with everything else fixed): `R = R_ref * sigma^(1/4)`, where `R_ref` is the range at 1 m2.
#[derive(Clone, Copy, Debug)]
pub struct RangeRcsRadar {
    /// Detection range against a 1 m2 target, metres.
    pub reference_range_m: f64,
}

impl RangeRcsRadar {
    /// Largest range at which a target of `rcs_m2` is seen, metres.
    #[must_use]
    pub fn detection_range_m(&self, rcs_m2: f64) -> f64 {
        if rcs_m2 > 0.0 && rcs_m2.is_finite() && self.reference_range_m.is_finite() {
            self.reference_range_m.max(0.0) * dmath::sqrt(dmath::sqrt(rcs_m2))
        } else {
            0.0
        }
    }
}

impl RadarModel for RangeRcsRadar {
    fn detect(&self, q: &RadarQuery) -> Detection {
        let range_m = sane_range(q);
        Detection {
            // An unusable position reports range 0, which must not read as "point blank".
            detected: positions_usable(q)
                && q.line_of_sight
                && range_m <= self.detection_range_m(q.target_rcs_m2),
            range_m,
        }
    }
}

fn positions_usable(q: &RadarQuery) -> bool {
    q.radar_pos.is_finite() && q.target_pos.is_finite()
}

/// Distance between the two points, or 0 when either is not finite.
fn sane_range(q: &RadarQuery) -> f64 {
    if positions_usable(q) {
        (q.target_pos - q.radar_pos).length()
    } else {
        0.0
    }
}

/// Behaviour every `RadarModel` must have (`AC-61.5`).
///
/// # Panics
/// Panics, with a message naming the property, if the implementation violates the contract.
pub fn contract(r: &dyn RadarModel) {
    let at = |range: f64, rcs: f64, los: bool| RadarQuery {
        radar_pos: DVec3::new(100.0, -50.0, 2_000.0),
        target_pos: DVec3::new(100.0 + range, -50.0, 2_000.0),
        target_rcs_m2: rcs,
        line_of_sight: los,
    };

    // No line of sight, no detection, whatever the range and size.
    for range in [1.0, 1_000.0, 50_000.0] {
        assert!(
            !r.detect(&at(range, 1.0e4, false)).detected,
            "a blocked target at {range} m is not seen"
        );
    }

    // Pure function of the query, and it reports the true range.
    let q = at(12_000.0, 3.0, true);
    assert_eq!(r.detect(&q), r.detect(&q), "detection is repeatable");
    assert!(
        (r.detect(&q).range_m - 12_000.0).abs() < 1e-6,
        "range is the distance"
    );

    // Seen at some range means seen at every shorter range; seen with some size means seen
    // with every larger size.
    for rcs in [0.01, 0.5, 5.0, 100.0] {
        let mut seen_before = true;
        for range in [100.0, 1_000.0, 10_000.0, 40_000.0, 150_000.0, 600_000.0] {
            let seen = r.detect(&at(range, rcs, true)).detected;
            assert!(
                seen_before || !seen,
                "monotone in range (rcs {rcs}, range {range})"
            );
            seen_before = seen;
        }
    }
    for range in [2_000.0, 20_000.0, 90_000.0] {
        let mut seen_before = false;
        for rcs in [0.01, 0.1, 1.0, 10.0, 100.0, 1_000.0] {
            let seen = r.detect(&at(range, rcs, true)).detected;
            assert!(
                !seen_before || seen,
                "monotone in rcs (range {range}, rcs {rcs})"
            );
            seen_before = seen;
        }
    }

    // Nonsense in, no detection out (and no panic).
    for rcs in [f64::NAN, f64::INFINITY, -1.0, 0.0] {
        assert!(
            !r.detect(&at(1_000.0, rcs, true)).detected,
            "rcs {rcs} is not seen"
        );
    }
    let bad = RadarQuery {
        target_pos: DVec3::new(f64::NAN, 0.0, 0.0),
        ..at(1_000.0, 1.0, true)
    };
    let d = r.detect(&bad);
    assert!(
        !d.detected && d.range_m == 0.0,
        "a non-finite position is not seen"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn query(range: f64, rcs: f64) -> RadarQuery {
        RadarQuery {
            radar_pos: DVec3::ZERO,
            target_pos: DVec3::new(0.0, range, 0.0),
            target_rcs_m2: rcs,
            line_of_sight: true,
        }
    }

    #[test]
    fn both_radars_pass_the_contract() {
        contract(&NoRadar);
        contract(&RangeRcsRadar {
            reference_range_m: 60_000.0,
        });
    }

    #[test]
    fn detection_range_follows_the_fourth_root_of_rcs() {
        let r = RangeRcsRadar {
            reference_range_m: 60_000.0,
        };
        assert!((r.detection_range_m(1.0) - 60_000.0).abs() < 1e-6);
        // 16 times the cross-section doubles the range.
        assert!((r.detection_range_m(16.0) - 120_000.0).abs() < 1e-6);
        // 1/16 of it halves the range.
        assert!((r.detection_range_m(1.0 / 16.0) - 30_000.0).abs() < 1e-6);
        assert!(r.detect(&query(119_000.0, 16.0)).detected);
        assert!(!r.detect(&query(121_000.0, 16.0)).detected);
    }

    #[test]
    fn the_null_radar_still_reports_range() {
        let d = NoRadar.detect(&query(5_000.0, 100.0));
        assert!(!d.detected);
        assert!((d.range_m - 5_000.0).abs() < 1e-9);
    }
}
