//! Hit volumes: capsules (a segment with a radius) in an aircraft's body axes, and the swept tests
//! that rounds, fuzes and collisions use (GDD 15.4.2, 18.2, 18.8.2, `AC-15.8`, `AC-18.1`).
//!
//! Every test is swept: a round or a missile is the segment it travelled during the tick, taken
//! relative to the target (both ends in the target's body axes), so nothing fast can pass through
//! a capsule between two samples.

use content::aircraft::CapsuleDef;
use simmath::math::{DQuat, DVec3};

/// Segments shorter than this (squared length, m²) count as points. // ALGO: 1 µm
const DEGENERATE_M2: f64 = 1e-12;

/// A capsule: every point within `radius` of the segment from `a` to `b` (body axes, m).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Capsule {
    /// One end of the segment, m.
    pub a: DVec3,
    /// The other end, m.
    pub b: DVec3,
    /// Radius, m.
    pub radius: f64,
}

impl Capsule {
    /// A capsule from the content's hit volume.
    #[must_use]
    pub fn from_def(d: &CapsuleDef) -> Self {
        let v = |p: [f32; 3]| DVec3::new(f64::from(p[0]), f64::from(p[1]), f64::from(p[2]));
        Self {
            a: v(d.a_m),
            b: v(d.b_m),
            radius: f64::from(d.radius_m),
        }
    }
}

/// The closest points of two segments: `(s, t, distance)` with the points at `p0 + s (p1 − p0)`
/// and `q0 + t (q1 − q0)`, `s, t` in `[0, 1]` (Ericson, *Real-Time Collision Detection* 5.1.9).
/// Parallel segments give one of the closest pairs; a degenerate segment is its first point.
#[must_use]
pub fn seg_seg_closest(p0: DVec3, p1: DVec3, q0: DVec3, q1: DVec3) -> (f64, f64, f64) {
    let d1 = p1 - p0;
    let d2 = q1 - q0;
    let r = p0 - q0;
    let a = d1.length_squared();
    let e = d2.length_squared();
    let f = d2.dot(r);
    let (s, t) = if a <= DEGENERATE_M2 && e <= DEGENERATE_M2 {
        (0.0, 0.0)
    } else if a <= DEGENERATE_M2 {
        (0.0, (f / e).clamp(0.0, 1.0))
    } else {
        let c = d1.dot(r);
        if e <= DEGENERATE_M2 {
            ((-c / a).clamp(0.0, 1.0), 0.0)
        } else {
            let b = d1.dot(d2);
            let denom = a * e - b * b;
            // Parallel (denom 0): any s works; take 0 and let t pick the pair.
            let s = if denom > 0.0 {
                ((b * f - c * e) / denom).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let t = (b * s + f) / e;
            if t < 0.0 {
                ((-c / a).clamp(0.0, 1.0), 0.0)
            } else if t > 1.0 {
                (((b - c) / a).clamp(0.0, 1.0), 1.0)
            } else {
                (s, t)
            }
        }
    };
    let dist = ((p0 + d1 * s) - (q0 + d2 * t)).length();
    (s, t, dist)
}

/// The shortest distance between the segments `p0 p1` and `q0 q1`, m.
#[must_use]
pub fn seg_seg_distance(p0: DVec3, p1: DVec3, q0: DVec3, q1: DVec3) -> f64 {
    seg_seg_closest(p0, p1, q0, q1).2
}

/// The smaller root `t` of `|m + t d|² = r²` (`dd = d·d > 0`), if the line meets the sphere.
fn sphere_entry(m: DVec3, d: DVec3, dd: f64, r: f64) -> Option<f64> {
    let b = m.dot(d);
    let c = m.length_squared() - r * r;
    let disc = b * b - dd * c;
    (disc >= 0.0).then(|| (-b - simmath::dmath::sqrt(disc)) / dd)
}

/// Where the segment `s0 + t (s1 − s0)` first enters the capsule, as `t` in `[0, 1]`; `None` if it
/// stays outside. The capsule is the union of a finite cylinder and two spheres, so the first entry
/// is the earliest of the cylinder's side entry (inside its length) and the two spheres' entries.
fn capsule_entry(s0: DVec3, s1: DVec3, c: &Capsule) -> Option<f64> {
    let (s_close, _, dist) = seg_seg_closest(s0, s1, c.a, c.b);
    if dist > c.radius {
        return None;
    }
    let (_, _, d0) = seg_seg_closest(s0, s0, c.a, c.b);
    if d0 <= c.radius {
        return Some(0.0);
    }
    let d = s1 - s0;
    let dd = d.length_squared();
    if dd <= DEGENERATE_M2 {
        return Some(0.0);
    }
    let mut best = f64::INFINITY;
    for centre in [c.a, c.b] {
        if let Some(t) = sphere_entry(s0 - centre, d, dd, c.radius) {
            best = best.min(t);
        }
    }
    let axis = c.b - c.a;
    let len2 = axis.length_squared();
    if len2 > DEGENERATE_M2 {
        let u = axis * (1.0 / simmath::dmath::sqrt(len2));
        let m = s0 - c.a;
        let m_perp = m - u * m.dot(u);
        let d_perp = d - u * d.dot(u);
        let dp2 = d_perp.length_squared();
        if dp2 > DEGENERATE_M2 * dd
            && let Some(t) = sphere_entry(m_perp, d_perp, dp2, c.radius)
            && (0.0..=1.0).contains(&((m + d * t).dot(axis) / len2))
        {
            best = best.min(t);
        }
    }
    // The segment reaches the capsule (dist <= radius), so an entry exists in [0, 1]; rounding at a
    // tangent can put the computed root a hair outside, and then the closest approach stands in.
    Some(if (0.0..=1.0).contains(&best) {
        best
    } else {
        s_close
    })
}

/// The first capsule the segment `s0 s1` enters and the fraction along the segment where it does:
/// `(index, t)`, the smallest `t` in `[0, 1]` over every capsule (ties go to the lower index).
/// `None` if the segment touches none. All in the capsules' axes.
#[must_use]
pub fn segment_hits_capsules(s0: DVec3, s1: DVec3, caps: &[Capsule]) -> Option<(usize, f64)> {
    let mut hit: Option<(usize, f64)> = None;
    for (i, c) in caps.iter().enumerate() {
        if let Some(t) = capsule_entry(s0, s1, c)
            && hit.is_none_or(|(_, best)| t < best)
        {
            hit = Some((i, t));
        }
    }
    hit
}

/// A BF point in the body axes of a body at `origin_bf` with attitude `att` (body to BF).
#[must_use]
pub fn to_body(att: DQuat, origin_bf: DVec3, p_bf: DVec3) -> DVec3 {
    att.conjugate().rotate(p_bf - origin_bf)
}

/// The segment a point travelled during a tick, relative to a moving body, in the body's axes at
/// the end of the tick: `(Q⁻¹ (p0 − o0), Q⁻¹ (p1 − o1))`, where `p` is the point and `o` the body's
/// reference point at the start (0) and end (1) of the tick, and `Q` the body's attitude at the end.
#[must_use]
pub fn relative_segment(
    p0_bf: DVec3,
    p1_bf: DVec3,
    o0_bf: DVec3,
    o1_bf: DVec3,
    att1: DQuat,
) -> (DVec3, DVec3) {
    let inv = att1.conjugate();
    (inv.rotate(p0_bf - o0_bf), inv.rotate(p1_bf - o1_bf))
}

/// The clearance between two sets of capsules on two bodies (reference point `p`, attitude `q`):
/// the smallest surface-to-surface distance over every pair, m. Zero or negative when they
/// overlap (GDD 18.8.2).
#[must_use]
pub fn capsules_distance(
    a: &[Capsule],
    qa: DQuat,
    pa: DVec3,
    b: &[Capsule],
    qb: DQuat,
    pb: DVec3,
) -> f64 {
    let mut best = f64::INFINITY;
    for ca in a {
        let (a0, a1) = (pa + qa.rotate(ca.a), pa + qa.rotate(ca.b));
        for cb in b {
            let (b0, b1) = (pb + qb.rotate(cb.a), pb + qb.rotate(cb.b));
            best = best.min(seg_seg_distance(a0, a1, b0, b1) - ca.radius - cb.radius);
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(x: f64, y: f64, z: f64) -> DVec3 {
        DVec3::new(x, y, z)
    }

    /// A small deterministic generator for the tests (splitmix64 to a unit float).
    struct Dice(u64);

    impl Dice {
        fn next(&mut self) -> f64 {
            self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = self.0;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            ((z ^ (z >> 31)) >> 11) as f64 / (1u64 << 53) as f64
        }
    }

    #[test]
    fn segment_distances_worked_examples() {
        // Crossing at right angles, 1 m apart: x axis at z = 0 and y axis at z = 1.
        let (s, t, d) = seg_seg_closest(
            v(-1.0, 0.0, 0.0),
            v(1.0, 0.0, 0.0),
            v(0.0, -1.0, 1.0),
            v(0.0, 1.0, 1.0),
        );
        assert_eq!((s, t, d), (0.5, 0.5, 1.0));
        // Parallel and overlapping, 2 m apart.
        assert_eq!(
            seg_seg_distance(
                v(0.0, 0.0, 0.0),
                v(4.0, 0.0, 0.0),
                v(1.0, 2.0, 0.0),
                v(3.0, 2.0, 0.0)
            ),
            2.0
        );
        // Parallel, end to end along the same line with a 3 m gap.
        assert_eq!(
            seg_seg_distance(
                v(0.0, 0.0, 0.0),
                v(1.0, 0.0, 0.0),
                v(4.0, 0.0, 0.0),
                v(6.0, 0.0, 0.0)
            ),
            3.0
        );
        // Two points 5 m apart (3-4-5).
        assert_eq!(
            seg_seg_distance(
                v(0.0, 0.0, 0.0),
                v(0.0, 0.0, 0.0),
                v(3.0, 4.0, 0.0),
                v(3.0, 4.0, 0.0)
            ),
            5.0
        );
        // A point beside a segment's interior, and beyond its end.
        assert_eq!(
            seg_seg_distance(
                v(2.0, 3.0, 0.0),
                v(2.0, 3.0, 0.0),
                v(0.0, 0.0, 0.0),
                v(4.0, 0.0, 0.0)
            ),
            3.0
        );
        assert_eq!(
            seg_seg_distance(
                v(7.0, 4.0, 0.0),
                v(7.0, 4.0, 0.0),
                v(0.0, 0.0, 0.0),
                v(4.0, 0.0, 0.0)
            ),
            5.0
        );
        // Skew, closest points at the ends: (1, 0, 0) and (1, 1, 2) are 5^0.5 apart.
        let d = seg_seg_distance(
            v(0.0, 0.0, 0.0),
            v(1.0, 0.0, 0.0),
            v(1.0, 1.0, 2.0),
            v(1.0, 5.0, 2.0),
        );
        assert!((d - 5f64.sqrt()).abs() < 1e-15, "{d}");
        // Symmetric in its arguments.
        let (p0, p1, q0, q1) = (
            v(0.3, -2.0, 1.0),
            v(4.0, 1.0, -1.0),
            v(-1.0, 0.5, 2.0),
            v(2.0, 2.0, -3.0),
        );
        let d1 = seg_seg_distance(p0, p1, q0, q1);
        assert!((d1 - seg_seg_distance(q1, q0, p1, p0)).abs() < 1e-12);
    }

    #[test]
    fn the_closest_distance_agrees_with_a_dense_search() {
        let mut dice = Dice(7);
        let mut r = || {
            v(
                dice.next() * 10.0 - 5.0,
                dice.next() * 10.0 - 5.0,
                dice.next() * 10.0 - 5.0,
            )
        };
        for _ in 0..200 {
            let (p0, p1, q0, q1) = (r(), r(), r(), r());
            let d = seg_seg_distance(p0, p1, q0, q1);
            let mut brute = f64::INFINITY;
            for i in 0..=200 {
                let a = p0.lerp(p1, f64::from(i) / 200.0);
                for j in 0..=200 {
                    brute = brute.min((a - q0.lerp(q1, f64::from(j) / 200.0)).length());
                }
            }
            assert!(d <= brute + 1e-9 && brute - d < 0.06, "{d} vs {brute}");
        }
    }

    #[test]
    fn a_segment_enters_the_first_capsule_on_its_way() {
        let caps = [
            Capsule {
                a: v(10.0, 0.0, -1.0),
                b: v(10.0, 0.0, 1.0),
                radius: 1.0,
            },
            Capsule {
                a: v(5.0, 0.0, -1.0),
                b: v(5.0, 0.0, 1.0),
                radius: 0.5,
            },
        ];
        // Along x from 0 to 20: enters the second capsule's side at x = 4.5 (t 0.225), first.
        let (i, t) =
            segment_hits_capsules(v(0.0, 0.0, 0.0), v(20.0, 0.0, 0.0), &caps).expect("a hit");
        assert_eq!(i, 1);
        assert!((t - 0.225).abs() < 1e-12, "{t}");
        // Over the second capsule's end cap: at z = 1.6 it passes 0.6 m from the cap's centre
        // (radius 0.5), so the first capsule's cap (radius 1 about (10, 0, 1)) catches it where
        // (x - 10)^2 + 0.6^2 = 1, at x = 9.2 (t 0.46).
        let (i, t) =
            segment_hits_capsules(v(0.0, 0.0, 1.6), v(20.0, 0.0, 1.6), &caps).expect("a hit");
        assert_eq!(i, 0);
        assert!((t - 0.46).abs() < 1e-12, "{t}");
        // Starting inside: t = 0.
        assert_eq!(
            segment_hits_capsules(v(10.0, 0.2, 0.0), v(30.0, 0.0, 0.0), &caps),
            Some((0, 0.0))
        );
        // A point (zero-length segment) inside and outside.
        assert_eq!(
            segment_hits_capsules(v(5.0, 0.4, 0.0), v(5.0, 0.4, 0.0), &caps),
            Some((1, 0.0))
        );
        assert_eq!(
            segment_hits_capsules(v(5.0, 0.6, 0.0), v(5.0, 0.6, 0.0), &caps),
            None
        );
        // Stopping short and passing wide.
        assert_eq!(
            segment_hits_capsules(v(0.0, 0.0, 0.0), v(4.4, 0.0, 0.0), &caps),
            None
        );
        assert_eq!(
            segment_hits_capsules(v(0.0, 1.01, 0.0), v(20.0, 1.01, 0.0), &caps),
            None
        );
        // Along a capsule's own axis, from outside its end cap: enters the cap at z = -2.
        let (i, t) =
            segment_hits_capsules(v(10.0, 0.0, -5.0), v(10.0, 0.0, 5.0), &caps).expect("a hit");
        assert_eq!(i, 0);
        assert!((t - 0.3).abs() < 1e-12, "{t}");
    }

    #[test]
    fn gn4_ac_15_8_no_tunnelling_at_1000_mps() {
        // A 1 m-radius capsule (axis along z, 2 m long) and rounds along x at 1 000 m/s sampled at
        // 60 Hz (16.7 m a tick): 1 000 pass inside 0.9 m of its axis at random phases, 1 000 pass
        // at 1.2 m. The swept test finds every one of the first and none of the second; a test of
        // the sampled points alone would miss most of the first.
        let cap = [Capsule {
            a: v(0.0, 0.0, -1.0),
            b: v(0.0, 0.0, 1.0),
            radius: 1.0,
        }];
        let step = 1_000.0 / 60.0;
        // (swept hit, point hit) for one round passing `offset` from the axis.
        let fly = |dice: &mut Dice, offset: f64| {
            // Where the round is at the start of the tick before the one it crosses in.
            let mut x = -step * (1.0 + dice.next());
            let z = dice.next() * 2.0 - 1.0;
            let y = if dice.next() < 0.5 { -offset } else { offset };
            let (mut swept, mut point) = (false, false);
            for _ in 0..4 {
                let (s0, s1) = (v(x, y, z), v(x + step, y, z));
                swept |= segment_hits_capsules(s0, s1, &cap).is_some();
                point |= segment_hits_capsules(s1, s1, &cap).is_some();
                x += step;
            }
            (swept, point)
        };
        let mut dice = Dice(0x00C0_FFEE);
        let (mut caught, mut sampled) = (0, 0);
        for _ in 0..1_000 {
            let offset = 0.9 * dice.next();
            let (swept, point) = fly(&mut dice, offset);
            caught += u32::from(swept);
            sampled += u32::from(point);
        }
        let mut false_hits = 0;
        for _ in 0..1_000 {
            false_hits += u32::from(fly(&mut dice, 1.2).0);
        }
        assert_eq!((caught, false_hits), (1_000, 0));
        assert!(sampled < 300, "point samples caught {sampled}");
    }

    #[test]
    fn capsules_overlap_when_their_clearance_is_negative() {
        let one = [Capsule {
            a: v(-5.0, 0.0, 0.0),
            b: v(5.0, 0.0, 0.0),
            radius: 1.0,
        }];
        let id = DQuat::IDENTITY;
        // Side by side 3 m apart: 1 m of clearance.
        let d = capsules_distance(&one, id, DVec3::ZERO, &one, id, v(0.0, 3.0, 0.0));
        assert!((d - 1.0).abs() < 1e-12, "{d}");
        // The second turned 90° about z and moved 1.5 m up: it crosses the first, 0.5 m of overlap.
        let q = DQuat::from_axis_angle(DVec3::Z, core::f64::consts::FRAC_PI_2);
        let d = capsules_distance(&one, id, DVec3::ZERO, &one, q, v(0.0, 0.0, 1.5));
        assert!((d + 0.5).abs() < 1e-12, "{d}");
    }

    #[test]
    fn segments_relative_to_a_moving_body() {
        // A body moving 10 m east during the tick, with its nose turned to the north at the end,
        // and a point that kept 5 m north of it: relative to the body the point stood still, 5 m
        // ahead of the nose (BF north is body +x at the end).
        let q = DQuat::from_axis_angle(DVec3::Z, core::f64::consts::FRAC_PI_2);
        let (s0, s1) = relative_segment(
            v(0.0, 5.0, 0.0),
            v(10.0, 5.0, 0.0),
            DVec3::ZERO,
            v(10.0, 0.0, 0.0),
            q,
        );
        assert!((s0 - v(5.0, 0.0, 0.0)).length() < 1e-12, "{s0:?}");
        assert!((s1 - v(5.0, 0.0, 0.0)).length() < 1e-12, "{s1:?}");
        assert!(
            (to_body(q, v(10.0, 0.0, 0.0), v(10.0, 0.0, 3.0)) - v(0.0, 0.0, 3.0)).length() < 1e-12
        );
    }
}
