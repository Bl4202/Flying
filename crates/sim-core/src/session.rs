//! The session configuration: what kind of sortie the player starts (GDD 6.3 `session_config/1`,
//! ADR-36).
//!
//! The host sets it once, while the core is idle, through `fg_configure`. Changes during the
//! sortie travel in the input frame instead (an assist request, a spawn request), so they are
//! deterministic and a replay reproduces them.

use core::fmt;

use crate::layout::{assist_level, g_effects, spawn_kind};

/// Where the player's aircraft starts (and respawns by default).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Spawn {
    /// Lined up on the runway, engine idling, brakes on (GDD 48.2 "runway hot").
    #[default]
    Runway,
    /// Airborne and trimmed.
    Air,
}

/// How the pilot's stick becomes control-surface commands (GDD 11.11.3).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Assist {
    /// Flight-path-rate and bank-angle commands, every protection.
    Full,
    /// Load-factor and roll-rate commands with the alpha, g and overspeed limiters.
    #[default]
    Standard,
    /// Pitch-rate and roll-rate commands, warnings instead of limits.
    Reduced,
    /// Direct surface commands.
    Manual,
}

impl Assist {
    /// The number carried over the ABI (see [`crate::layout::assist_level`]).
    #[must_use]
    pub const fn code(self) -> u8 {
        match self {
            Self::Full => assist_level::FULL as u8,
            Self::Standard => assist_level::STANDARD as u8,
            Self::Reduced => assist_level::REDUCED as u8,
            Self::Manual => assist_level::MANUAL as u8,
        }
    }

    /// The control system's level of the same number.
    #[must_use]
    pub fn to_fcs(self) -> fcs::Assist {
        match self {
            Self::Full => fcs::Assist::Full,
            Self::Standard => fcs::Assist::Standard,
            Self::Reduced => fcs::Assist::Reduced,
            Self::Manual => fcs::Assist::Manual,
        }
    }

    /// The level for an ABI number, or `None` if it is not one.
    #[must_use]
    pub const fn from_code(code: u32) -> Option<Self> {
        match code as usize {
            assist_level::FULL => Some(Self::Full),
            assist_level::STANDARD => Some(Self::Standard),
            assist_level::REDUCED => Some(Self::Reduced),
            assist_level::MANUAL => Some(Self::Manual),
            _ => None,
        }
    }
}

/// How strongly sustained g affects the pilot (GDD 11.14).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GEffects {
    /// The pilot state is computed and reported; nothing is applied.
    Off,
    /// Visual effects only, with a higher tolerance.
    Reduced,
    /// Blackout delays the controls and G-LOC zeroes them.
    #[default]
    Full,
}

impl GEffects {
    /// The flight model's dial of the same name.
    #[must_use]
    pub fn to_flight(self) -> flight::physio::GEffects {
        match self {
            Self::Off => flight::physio::GEffects::Off,
            Self::Reduced => flight::physio::GEffects::Reduced,
            Self::Full => flight::physio::GEffects::Full,
        }
    }
}

/// The session: spawn, initial assist level and g-effects.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SessionConfig {
    /// Where the aircraft starts.
    pub spawn: Spawn,
    /// The assist level in force at the start.
    pub assist: Assist,
    /// The g-effects dial.
    pub g_effects: GEffects,
}

/// Why a session was not accepted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfigError {
    /// An argument is not one of the layout's codes.
    Invalid,
    /// The core has started; the session must be set first.
    NotIdle,
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid => write!(f, "a session argument is out of range"),
            Self::NotIdle => write!(
                f,
                "the session can only be configured while the core is idle"
            ),
        }
    }
}

impl std::error::Error for ConfigError {}

impl SessionConfig {
    /// The three settings as one number, for the state hash: the `spawn_kind`, `assist_level` and
    /// `g_effects` codes in a byte each.
    #[must_use]
    pub fn hash_word(&self) -> u64 {
        let spawn = match self.spawn {
            Spawn::Runway => spawn_kind::RUNWAY,
            Spawn::Air => spawn_kind::AIR,
        };
        let g = match self.g_effects {
            GEffects::Off => g_effects::OFF,
            GEffects::Reduced => g_effects::REDUCED,
            GEffects::Full => g_effects::FULL,
        };
        spawn as u64 | u64::from(self.assist.code()) << 8 | (g as u64) << 16
    }

    /// Builds a session from the numbers of `fg_configure` (`spawn_kind`, `assist_level`,
    /// `g_effects`).
    ///
    /// # Errors
    /// [`ConfigError::Invalid`] if any argument is not a code of its group.
    pub fn from_codes(spawn: u32, assist: u32, g: u32) -> Result<Self, ConfigError> {
        let spawn = match spawn as usize {
            spawn_kind::RUNWAY => Spawn::Runway,
            spawn_kind::AIR => Spawn::Air,
            _ => return Err(ConfigError::Invalid),
        };
        let assist = Assist::from_code(assist).ok_or(ConfigError::Invalid)?;
        let g_effects = match g as usize {
            g_effects::OFF => GEffects::Off,
            g_effects::REDUCED => GEffects::Reduced,
            g_effects::FULL => GEffects::Full,
            _ => return Err(ConfigError::Invalid),
        };
        Ok(Self {
            spawn,
            assist,
            g_effects,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_session_is_the_runway_start_under_standard_assist() {
        let s = SessionConfig::default();
        assert_eq!(s.spawn, Spawn::Runway);
        assert_eq!(s.assist, Assist::Standard);
        assert_eq!(s.g_effects, GEffects::Full);
    }

    #[test]
    fn codes_round_trip_and_unknown_codes_are_refused() {
        for (code, want) in [
            (1, Assist::Full),
            (2, Assist::Standard),
            (3, Assist::Reduced),
            (4, Assist::Manual),
        ] {
            assert_eq!(Assist::from_code(code), Some(want));
            assert_eq!(u32::from(want.code()), code);
        }
        assert_eq!(Assist::from_code(0), None, "0 means keep, not a level");
        assert_eq!(Assist::from_code(5), None);
        let ok = SessionConfig::from_codes(1, 4, 0).unwrap();
        assert_eq!(
            (ok.spawn, ok.assist, ok.g_effects),
            (Spawn::Air, Assist::Manual, GEffects::Off)
        );
        assert_eq!(
            SessionConfig::from_codes(2, 2, 2),
            Err(ConfigError::Invalid)
        );
        assert_eq!(
            SessionConfig::from_codes(0, 0, 2),
            Err(ConfigError::Invalid)
        );
        assert_eq!(
            SessionConfig::from_codes(0, 2, 3),
            Err(ConfigError::Invalid)
        );
    }

    #[test]
    fn the_levels_map_onto_the_control_systems_and_the_flight_models() {
        for level in [
            Assist::Full,
            Assist::Standard,
            Assist::Reduced,
            Assist::Manual,
        ] {
            assert_eq!(level.to_fcs().code(), level.code());
        }
        assert_eq!(GEffects::Off.to_flight(), flight::physio::GEffects::Off);
        assert_eq!(
            GEffects::Reduced.to_flight(),
            flight::physio::GEffects::Reduced
        );
        assert_eq!(GEffects::Full.to_flight(), flight::physio::GEffects::Full);
    }

    #[test]
    fn every_session_hashes_to_its_own_word() {
        let mut seen = Vec::new();
        for spawn in [Spawn::Runway, Spawn::Air] {
            for assist in [
                Assist::Full,
                Assist::Standard,
                Assist::Reduced,
                Assist::Manual,
            ] {
                for g_effects in [GEffects::Off, GEffects::Reduced, GEffects::Full] {
                    let w = SessionConfig {
                        spawn,
                        assist,
                        g_effects,
                    }
                    .hash_word();
                    assert!(!seen.contains(&w));
                    seen.push(w);
                }
            }
        }
    }
}
