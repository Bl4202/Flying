//! State hashing for determinism checks (ADR-10, ADR-23).
//!
//! FNV-1a 64 over canonical little-endian bytes. Floats hash through their IEEE-754 bit pattern;
//! every NaN collapses to one canonical pattern (NaN payloads may legitimately differ between
//! targets), while `-0.0` and `+0.0` stay distinct because they are distinguishable states.

const FNV_OFFSET: u64 = 0xCBF2_9CE4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01B3;
const CANONICAL_NAN_BITS: u64 = 0x7FF8_0000_0000_0000;

/// Incremental FNV-1a 64 hasher.
#[derive(Clone, Debug)]
pub struct StateHasher {
    h: u64,
}

impl Default for StateHasher {
    fn default() -> Self {
        Self::new()
    }
}

impl StateHasher {
    /// A fresh hasher.
    #[must_use]
    pub fn new() -> Self {
        Self { h: FNV_OFFSET }
    }

    /// Feeds raw bytes.
    pub fn write_bytes(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.h ^= u64::from(b);
            self.h = self.h.wrapping_mul(FNV_PRIME);
        }
    }

    /// Feeds a `u64` as 8 little-endian bytes.
    pub fn write_u64(&mut self, v: u64) {
        self.write_bytes(&v.to_le_bytes());
    }

    /// Feeds an `f64` by bit pattern, canonicalising NaN.
    pub fn write_f64(&mut self, v: f64) {
        let bits = if v.is_nan() {
            CANONICAL_NAN_BITS
        } else {
            v.to_bits()
        };
        self.write_u64(bits);
    }

    /// Feeds a slice of `f64`.
    pub fn write_f64s(&mut self, vs: &[f64]) {
        for &v in vs {
            self.write_f64(v);
        }
    }

    /// The hash so far.
    #[must_use]
    pub fn finish(&self) -> u64 {
        self.h
    }
}

/// The tag the combat column holds when the session has no combat: "NOCOMBAT" in ASCII, read as a
/// little-endian `u64` (ADR-45, DT1).
pub const NO_COMBAT_TAG: u64 = 0x5441_424D_4F43_4F4E;

/// The combat column of a session with no combat: the hash of [`NO_COMBAT_TAG`]. It is a constant,
/// so the free-flight goldens can show that adding combat changed nothing else (ADR-45, DT1).
#[must_use]
pub fn no_combat_hash() -> u64 {
    let mut h = StateHasher::new();
    h.write_u64(NO_COMBAT_TAG);
    h.finish()
}

/// Per-subsystem hashes plus the combined hash, so a mismatch can be localised.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SubsystemHashes {
    /// Tick counter, core state and master seed.
    pub tick: u64,
    /// The probe world's random stream states.
    pub rng: u64,
    /// Probe-world body state.
    pub bodies: u64,
    /// The ownship: the player's aircraft, or the spectator without aircraft content.
    pub ownship: u64,
    /// Validated input state, including the scripted pilot.
    pub input: u64,
    /// The loaded content pack's identity (zero-content packs hash too).
    pub content: u64,
    /// The combat state; [`no_combat_hash`] when the session has none. Holds every number of the
    /// combat state and nothing observational (events, counters).
    pub combat: u64,
    /// Hash of the seven above, in that order.
    pub combined: u64,
}

impl SubsystemHashes {
    /// Combines the subsystem hashes.
    #[must_use]
    pub fn new(
        tick: u64,
        rng: u64,
        bodies: u64,
        ownship: u64,
        input: u64,
        content: u64,
        combat: u64,
    ) -> Self {
        let mut h = StateHasher::new();
        h.write_u64(tick);
        h.write_u64(rng);
        h.write_u64(bodies);
        h.write_u64(ownship);
        h.write_u64(input);
        h.write_u64(content);
        h.write_u64(combat);
        Self {
            tick,
            rng,
            bodies,
            ownship,
            input,
            content,
            combat,
            combined: h.finish(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_plain_fnv_on_bytes() {
        let mut h = StateHasher::new();
        h.write_bytes(b"foobar");
        assert_eq!(h.finish(), 0x8594_4171_F739_67E8);
    }

    #[test]
    fn nan_payloads_collapse() {
        let a = f64::from_bits(0x7FF8_0000_0000_0001);
        let b = f64::from_bits(0xFFF8_0000_0000_0042);
        let mut ha = StateHasher::new();
        let mut hb = StateHasher::new();
        ha.write_f64(a);
        hb.write_f64(b);
        assert_eq!(ha.finish(), hb.finish());
    }

    #[test]
    fn signed_zero_is_distinct() {
        let mut ha = StateHasher::new();
        let mut hb = StateHasher::new();
        ha.write_f64(0.0);
        hb.write_f64(-0.0);
        assert_ne!(ha.finish(), hb.finish());
    }

    #[test]
    fn combined_depends_on_every_part() {
        let base = SubsystemHashes::new(1, 2, 3, 4, 5, 6, 7);
        for changed in [
            SubsystemHashes::new(9, 2, 3, 4, 5, 6, 7),
            SubsystemHashes::new(1, 9, 3, 4, 5, 6, 7),
            SubsystemHashes::new(1, 2, 9, 4, 5, 6, 7),
            SubsystemHashes::new(1, 2, 3, 9, 5, 6, 7),
            SubsystemHashes::new(1, 2, 3, 4, 9, 6, 7),
            SubsystemHashes::new(1, 2, 3, 4, 5, 9, 7),
            SubsystemHashes::new(1, 2, 3, 4, 5, 6, 9),
        ] {
            assert_ne!(base.combined, changed.combined);
        }
        assert_eq!(base.combat, 7);
    }

    #[test]
    fn the_no_combat_tag_spells_nocombat_and_its_hash_is_a_constant() {
        assert_eq!(&NO_COMBAT_TAG.to_le_bytes(), b"NOCOMBAT");
        // FNV-1a 64 of the eight tag bytes: pinned, because the free-flight goldens carry it.
        let mut h = StateHasher::new();
        h.write_bytes(b"NOCOMBAT");
        assert_eq!(no_combat_hash(), h.finish());
        assert_eq!(no_combat_hash(), no_combat_hash());
    }
}
