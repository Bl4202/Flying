//! Integer random number generators (GDD 26.3, Appendix B.10).
//!
//! * `SplitMix64` seeds everything.
//! * `xoshiro256**` is the working generator.
//! * Named streams: `SplitMix64(master ^ fnv1a64(stream_name))` seeds each stream, so adding
//!   randomness to one subsystem never perturbs another.
//!
//! Float draws use the 53-bit shift method; the multiply by a power of two is exact.

const SPLITMIX_GAMMA: u64 = 0x9E37_79B9_7F4A_7C15;
const FNV_OFFSET: u64 = 0xCBF2_9CE4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01B3;

/// One step of `SplitMix64`. Advances `state` and returns the next output.
pub fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(SPLITMIX_GAMMA);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// 64-bit FNV-1a hash of `bytes`.
#[must_use]
pub fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut h = FNV_OFFSET;
    for &b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(FNV_PRIME);
    }
    h
}

/// The `xoshiro256**` generator.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Xoshiro256StarStar {
    s: [u64; 4],
}

impl Xoshiro256StarStar {
    /// Builds a generator from explicit state. The state must not be all zero.
    #[must_use]
    pub fn from_state(s: [u64; 4]) -> Self {
        debug_assert!(s != [0; 4], "xoshiro256** state must not be all zero");
        Self { s }
    }

    /// Seeds a generator by running `SplitMix64` from `seed` four times.
    #[must_use]
    pub fn from_seed(seed: u64) -> Self {
        let mut sm = seed;
        let s = [
            splitmix64(&mut sm),
            splitmix64(&mut sm),
            splitmix64(&mut sm),
            splitmix64(&mut sm),
        ];
        Self::from_state(s)
    }

    /// The raw state words, for hashing and serialisation.
    #[must_use]
    pub fn state(&self) -> [u64; 4] {
        self.s
    }

    /// Next 64 random bits.
    pub fn next_u64(&mut self) -> u64 {
        let result = self.s[1].wrapping_mul(5).rotate_left(7).wrapping_mul(9);
        let t = self.s[1] << 17;
        self.s[2] ^= self.s[0];
        self.s[3] ^= self.s[1];
        self.s[1] ^= self.s[2];
        self.s[0] ^= self.s[3];
        self.s[2] ^= t;
        self.s[3] = self.s[3].rotate_left(45);
        result
    }

    /// Uniform float in `[0, 1)` with 53 bits of precision.
    pub fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / 9_007_199_254_740_992.0)
    }

    /// Uniform float in `[lo, hi)`.
    pub fn range_f64(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.next_f64()
    }
}

/// Factory for named, independent random streams derived from one master seed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RngStreams {
    master: u64,
}

impl RngStreams {
    /// Creates the factory for `master`.
    #[must_use]
    pub fn new(master: u64) -> Self {
        Self { master }
    }

    /// The master seed.
    #[must_use]
    pub fn master(&self) -> u64 {
        self.master
    }

    /// A fresh generator for the stream called `name`.
    #[must_use]
    pub fn stream(&self, name: &str) -> Xoshiro256StarStar {
        Xoshiro256StarStar::from_seed(self.master ^ fnv1a64(name.as_bytes()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splitmix64_reference_vector_seed_zero() {
        let mut s = 0_u64;
        assert_eq!(splitmix64(&mut s), 0xE220_A839_7B1D_CDAF);
        assert_eq!(splitmix64(&mut s), 0x6E78_9E6A_A1B9_65F4);
        assert_eq!(splitmix64(&mut s), 0x06C4_5D18_8009_454F);
    }

    #[test]
    fn fnv1a64_reference_vectors() {
        assert_eq!(fnv1a64(b""), 0xCBF2_9CE4_8422_2325);
        assert_eq!(fnv1a64(b"a"), 0xAF63_DC4C_8601_EC8C);
        assert_eq!(fnv1a64(b"foobar"), 0x8594_4171_F739_67E8);
    }

    #[test]
    fn xoshiro_reference_vector_state_1234() {
        let mut r = Xoshiro256StarStar::from_state([1, 2, 3, 4]);
        assert_eq!(r.next_u64(), 11_520);
        assert_eq!(r.next_u64(), 0);
        assert_eq!(r.next_u64(), 1_509_978_240);
        assert_eq!(r.next_u64(), 1_215_971_899_390_074_240);
    }

    #[test]
    fn floats_are_in_unit_interval() {
        let mut r = Xoshiro256StarStar::from_seed(42);
        for _ in 0..10_000 {
            let x = r.next_f64();
            assert!((0.0..1.0).contains(&x));
        }
    }

    #[test]
    fn streams_are_independent_and_reproducible() {
        let a = RngStreams::new(7);
        let b = RngStreams::new(7);
        assert_eq!(a.stream("forces"), b.stream("forces"));
        assert_ne!(a.stream("forces"), a.stream("events"));
        assert_ne!(a.stream("forces"), RngStreams::new(8).stream("forces"));
    }
}
