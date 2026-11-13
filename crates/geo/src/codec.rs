//! The height codec of a `GeoPack` tile (ADR-54): 16-bit quantisation between the tile's `h_min` and
//! `h_max`, and the row-delta coding of the stored payload.

use crate::tile::{POST_COUNT, POSTS};

/// The largest quantised value. // DATA: ADR-54, 16-bit posts
pub const Q_MAX: f64 = 65_535.0;

/// The quantisation bounds of a tile whose posts span [`min`, `max`] (f64): `h_min` is the largest
/// f32 ≤ `min` and `h_max` the smallest f32 ≥ `max`, so the decoder reproduces both exactly.
#[must_use]
pub fn bounds_f32(min: f64, max: f64) -> (f32, f32) {
    let mut lo = min as f32;
    if f64::from(lo) > min {
        lo = next_down(lo);
    }
    let mut hi = max as f32;
    if f64::from(hi) < max {
        hi = next_up(hi);
    }
    (lo, hi)
}

/// The next f32 towards +∞ (finite input).
fn next_up(x: f32) -> f32 {
    if x == 0.0 {
        return f32::from_bits(1);
    }
    let b = x.to_bits();
    f32::from_bits(if x > 0.0 { b + 1 } else { b - 1 })
}

/// The next f32 towards −∞ (finite input).
fn next_down(x: f32) -> f32 {
    -next_up(-x)
}

/// A height to its 16-bit code: `round((h − h_min) 65535 / (h_max − h_min))` clamped to
/// [0, 65535]; 0 when `h_max == h_min`.
#[must_use]
pub fn quantise(h: f64, h_min: f32, h_max: f32) -> u16 {
    let (lo, hi) = (f64::from(h_min), f64::from(h_max));
    if hi <= lo {
        return 0;
    }
    ((h - lo) * Q_MAX / (hi - lo)).round().clamp(0.0, Q_MAX) as u16
}

/// A 16-bit code to its height: `h_min + q (h_max − h_min) / 65535`.
#[must_use]
pub fn dequantise(q: f64, h_min: f32, h_max: f32) -> f64 {
    let lo = f64::from(h_min);
    lo + q * (f64::from(h_max) - lo) / Q_MAX
}

/// Row-delta coding in place: row `r` becomes `q[r] − q[r − 1]` (wrapping), row 0 stays raw.
pub fn delta_encode_rows(posts: &mut [u16; POST_COUNT]) {
    for r in (1..POSTS).rev() {
        for c in 0..POSTS {
            posts[r * POSTS + c] = posts[r * POSTS + c].wrapping_sub(posts[(r - 1) * POSTS + c]);
        }
    }
}

/// The inverse of [`delta_encode_rows`], in place.
pub fn delta_decode_rows(posts: &mut [u16; POST_COUNT]) {
    for r in 1..POSTS {
        for c in 0..POSTS {
            posts[r * POSTS + c] = posts[r * POSTS + c].wrapping_add(posts[(r - 1) * POSTS + c]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::Rng;

    #[test]
    fn heights_round_trip_within_half_a_step() {
        let mut rng = Rng::new(11);
        for _ in 0..10_000 {
            let a = rng.uniform(-500.0, 9_000.0);
            let b = a + rng.uniform(0.0, 5_000.0);
            let (lo, hi) = bounds_f32(a, b);
            assert!(f64::from(lo) <= a && f64::from(hi) >= b);
            let h = rng.uniform(a, b);
            let back = dequantise(f64::from(quantise(h, lo, hi)), lo, hi);
            let tol = (f64::from(hi) - f64::from(lo)) / 131_070.0 + 1e-9;
            assert!((back - h).abs() <= tol, "{h} -> {back} (tol {tol})");
        }
    }

    #[test]
    fn a_flat_tile_codes_to_zero() {
        let (lo, hi) = bounds_f32(553.5, 553.5);
        assert_eq!((lo, hi), (553.5, 553.5));
        assert_eq!(quantise(553.5, lo, hi), 0);
        assert_eq!(dequantise(0.0, lo, hi), 553.5);
    }

    #[test]
    fn bounds_step_outwards_when_the_cast_rounds_inwards() {
        let min = 0.1_f64; // 0.1 is not an f32; its nearest f32 is above it
        let (lo, hi) = bounds_f32(min, min);
        assert!(f64::from(lo) <= min && f64::from(hi) >= min);
        assert!(lo < hi);
        assert_eq!(next_down(next_up(1.5)), 1.5);
        assert_eq!(next_up(-f32::from_bits(1)), -0.0);
    }

    #[test]
    fn delta_coding_round_trips() {
        let mut rng = Rng::new(12);
        let mut p: Box<[u16; POST_COUNT]> = vec![0u16; POST_COUNT].try_into().unwrap();
        for v in p.as_mut_slice() {
            *v = (rng.next_u64() & 0xFFFF) as u16;
        }
        let orig = p.clone();
        delta_encode_rows(&mut p);
        assert_eq!(p[..POSTS], orig[..POSTS]);
        assert_ne!(p, orig);
        delta_decode_rows(&mut p);
        assert_eq!(p, orig);
    }
}
