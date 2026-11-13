//! Deterministic random points for the gate tests (ADR-51) and the selftests that repeat them in
//! WebAssembly. The geodesy itself takes no randomness; only tests draw from here.
//!
//! [`Rng`] is `xoshiro256**` (Blackman and Vigna) seeded through `SplitMix64`, so a seed gives the same
//! sequence on every platform.

use simmath::dmath;
use simmath::math::DVec3;

use crate::angles::RAD;
use crate::geodetic::Geodetic;

/// `xoshiro256**`, seeded through `SplitMix64`.
#[derive(Clone, Debug)]
pub struct Rng {
    s: [u64; 4],
}

impl Rng {
    /// A generator from a seed.
    #[must_use]
    pub fn new(seed: u64) -> Self {
        let mut z = seed;
        let mut s = [0u64; 4];
        for w in &mut s {
            // ALGO: `SplitMix64` (Vigna), the recommended seeder for xoshiro
            z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut x = z;
            x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            *w = x ^ (x >> 31);
        }
        Self { s }
    }

    /// The next 64 random bits.
    pub fn next_u64(&mut self) -> u64 {
        let r = self.s[1].wrapping_mul(5).rotate_left(7).wrapping_mul(9);
        let t = self.s[1] << 17;
        self.s[2] ^= self.s[0];
        self.s[3] ^= self.s[1];
        self.s[1] ^= self.s[2];
        self.s[0] ^= self.s[3];
        self.s[2] ^= t;
        self.s[3] = self.s[3].rotate_left(45);
        r
    }

    /// Uniform in [0, 1), 53 bits.
    pub fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }

    /// Uniform in [`a`, `b`).
    pub fn uniform(&mut self, a: f64, b: f64) -> f64 {
        a + (b - a) * self.next_f64()
    }

    /// A longitude uniform in (−π, π].
    pub fn lon(&mut self) -> f64 {
        core::f64::consts::PI - core::f64::consts::TAU * self.next_f64()
    }
}

/// GE2's points (seed 1): φ uniform in [−89.9°, 89.9°], λ in (−180°, 180°], h in [−500, 30 000] m.
pub fn ge2_points(seed: u64) -> impl Iterator<Item = Geodetic> {
    let mut r = Rng::new(seed);
    core::iter::repeat_with(move || {
        let lat = r.uniform(-89.9, 89.9) * RAD;
        let lon = r.lon();
        let h = r.uniform(-500.0, 30_000.0);
        Geodetic::new(lat, lon, h)
    })
}

/// A GE3 anchor (φ in [−85°, 85°], λ in (−180°, 180°]) as (lat, lon), rad.
pub fn ge3_anchor(r: &mut Rng) -> (f64, f64) {
    (r.uniform(-85.0, 85.0) * RAD, r.lon())
}

/// A GE3 BF point: (x, y) uniform in a disc of 1 000 km, z in [−20 000, 30 000] m.
pub fn ge3_point(r: &mut Rng) -> DVec3 {
    let rad = 1_000_000.0 * r.next_f64().sqrt();
    let th = core::f64::consts::TAU * r.next_f64();
    DVec3::new(
        rad * dmath::cos(th),
        rad * dmath::sin(th),
        r.uniform(-20_000.0, 30_000.0),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_generator_is_reproducible_and_in_range() {
        let a: Vec<u64> = (0..4)
            .map({
                let mut r = Rng::new(1);
                move |_| r.next_u64()
            })
            .collect();
        let b: Vec<u64> = (0..4)
            .map({
                let mut r = Rng::new(1);
                move |_| r.next_u64()
            })
            .collect();
        assert_eq!(a, b);
        let mut r = Rng::new(9);
        for _ in 0..10_000 {
            let x = r.next_f64();
            assert!((0.0..1.0).contains(&x));
            let l = r.lon();
            assert!(l > -core::f64::consts::PI && l <= core::f64::consts::PI);
        }
    }
}
