//! Wire protocol: the version constant, the transport abstraction and its loopback
//! implementation (GDD 33.3). Messages and the handshake arrive with multiplayer (v0.5).
//!
//! `protocol` is the single place a version bump has to happen (GDD 61.7: protocol changes
//! require a version bump).
#![deny(missing_docs)]

pub mod transport;

/// Multiplayer protocol version. Bumped on any wire-format change; mismatches are refused
/// cleanly at the handshake (GDD 62.7).
pub const PROTOCOL_VERSION: u32 = 0;
