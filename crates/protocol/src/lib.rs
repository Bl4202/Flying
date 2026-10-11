//! Wire protocol (stub until v0.5).
//!
//! Exists now so the workspace graph matches GDD 61.3 and so `protocol` is the single place a
//! version bump has to happen (GDD 61.7: protocol changes require a version bump).
#![deny(missing_docs)]

/// Multiplayer protocol version. Bumped on any wire-format change; mismatches are refused
/// cleanly at the handshake (GDD 62.7).
pub const PROTOCOL_VERSION: u32 = 0;
