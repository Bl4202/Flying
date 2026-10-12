//! `PersistenceSink`: where progress is stored (GDD 34.6, 61.5). The first implementations
//! discard everything or keep it in memory; the OPFS-backed profile store replaces them in M2.

use std::collections::BTreeMap;

/// Largest value a sink must accept, bytes.
pub const MAX_VALUE_BYTES: usize = 1 << 20;
/// Longest key a sink must accept, bytes.
pub const MAX_KEY_BYTES: usize = 128;

/// Why a write was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PersistError {
    /// The key is empty, too long, or has characters outside `[a-z0-9._/-]`.
    InvalidKey,
    /// The value is larger than [`MAX_VALUE_BYTES`].
    TooLarge,
}

impl core::fmt::Display for PersistError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            Self::InvalidKey => "invalid persistence key",
            Self::TooLarge => "persistence value too large",
        })
    }
}

impl std::error::Error for PersistError {}

/// A key-value store for progress and settings.
pub trait PersistenceSink: Send {
    /// Stores `bytes` under `key`, replacing any earlier value.
    ///
    /// # Errors
    /// [`PersistError::InvalidKey`] or [`PersistError::TooLarge`]; nothing is stored.
    fn write(&mut self, key: &str, bytes: &[u8]) -> Result<(), PersistError>;

    /// The value stored under `key`, if this sink keeps values and has one.
    fn read(&self, key: &str) -> Option<Vec<u8>>;

    /// True if values written can be read back.
    fn retains(&self) -> bool;
}

/// Checks a key and value against the limits every sink shares.
///
/// # Errors
/// [`PersistError::InvalidKey`] or [`PersistError::TooLarge`].
pub fn validate(key: &str, bytes: &[u8]) -> Result<(), PersistError> {
    let ok_char = |c: u8| c.is_ascii_lowercase() || c.is_ascii_digit() || b"._/-".contains(&c);
    if key.is_empty() || key.len() > MAX_KEY_BYTES || !key.bytes().all(ok_char) {
        return Err(PersistError::InvalidKey);
    }
    if bytes.len() > MAX_VALUE_BYTES {
        return Err(PersistError::TooLarge);
    }
    Ok(())
}

/// Accepts valid writes and forgets them.
#[derive(Clone, Copy, Debug, Default)]
pub struct DiscardingSink;

impl PersistenceSink for DiscardingSink {
    fn write(&mut self, key: &str, bytes: &[u8]) -> Result<(), PersistError> {
        validate(key, bytes)
    }

    fn read(&self, _key: &str) -> Option<Vec<u8>> {
        None
    }

    fn retains(&self) -> bool {
        false
    }
}

/// Keeps values in memory, in key order.
#[derive(Clone, Debug, Default)]
pub struct InMemorySink {
    values: BTreeMap<String, Vec<u8>>,
}

impl InMemorySink {
    /// Number of keys stored.
    #[must_use]
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// True if nothing is stored.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

impl PersistenceSink for InMemorySink {
    fn write(&mut self, key: &str, bytes: &[u8]) -> Result<(), PersistError> {
        validate(key, bytes)?;
        self.values.insert(key.to_owned(), bytes.to_vec());
        Ok(())
    }

    fn read(&self, key: &str) -> Option<Vec<u8>> {
        self.values.get(key).cloned()
    }

    fn retains(&self) -> bool {
        true
    }
}

/// Behaviour every `PersistenceSink` must have (`AC-61.5`).
///
/// # Panics
/// Panics, with a message naming the property, if the implementation violates the contract.
pub fn contract(s: &mut dyn PersistenceSink) {
    assert_eq!(
        s.read("profile/never-written"),
        None,
        "an unwritten key reads as nothing"
    );

    for bad in [
        "",
        "Has Space",
        "UPPER",
        "semi;colon",
        &"k".repeat(MAX_KEY_BYTES + 1),
    ] {
        assert_eq!(
            s.write(bad, b"x"),
            Err(PersistError::InvalidKey),
            "key {bad:?} is refused"
        );
        assert_eq!(s.read(bad), None, "a refused write stores nothing");
    }
    let big = vec![0_u8; MAX_VALUE_BYTES + 1];
    assert_eq!(s.write("profile/big", &big), Err(PersistError::TooLarge));
    assert_eq!(s.read("profile/big"), None);

    assert_eq!(s.write("profile/a.bin", b"one"), Ok(()));
    assert_eq!(
        s.write("profile/b.bin", &[]),
        Ok(()),
        "an empty value is valid"
    );
    assert_eq!(
        s.write("profile/a.bin", b"two"),
        Ok(()),
        "a key can be overwritten"
    );
    assert_eq!(
        s.write("profile/max", &big[..MAX_VALUE_BYTES]),
        Ok(()),
        "the limit is inclusive"
    );

    if s.retains() {
        assert_eq!(
            s.read("profile/a.bin").as_deref(),
            Some(&b"two"[..]),
            "last write wins"
        );
        assert_eq!(s.read("profile/b.bin").as_deref(), Some(&[][..]));
        assert_eq!(
            s.read("profile/max").map(|v| v.len()),
            Some(MAX_VALUE_BYTES)
        );
    } else {
        assert_eq!(
            s.read("profile/a.bin"),
            None,
            "a non-retaining sink reads as nothing"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_sinks_pass_the_contract() {
        contract(&mut DiscardingSink);
        contract(&mut InMemorySink::default());
    }

    #[test]
    fn the_memory_sink_counts_keys() {
        let mut s = InMemorySink::default();
        assert!(s.is_empty());
        s.write("a", b"1").unwrap();
        s.write("b", b"2").unwrap();
        s.write("a", b"3").unwrap();
        assert_eq!(s.len(), 2);
    }
}
