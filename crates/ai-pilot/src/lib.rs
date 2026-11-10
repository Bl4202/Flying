//! The pilot AI of v0.1d (GDD 19, ADR-47): engagement geometry, the manoeuvre controller that turns
//! goals into a player's stick and throttle, the skill levels, the bandit's state machine, the
//! straight-and-level test pilot, and the bot playtester that flies a whole sortie.
//!
//! Layering (ADR-29, ADR-47): this crate depends on `content`, `fcs`, `flight` and `simmath`. It
//! never touches the world: `sim-core` builds a [`percept::Percept`] (what the bandit knows, already
//! delayed and noised by the perception stub) and hands over the aircraft's state, and the brains
//! answer with [`flight::controls::PilotControls`] and button presses, which `sim-core` turns into
//! input frames, exactly as a player's (GDD 19.12). Random numbers come in as inputs
//! ([`percept::Dice`]), drawn by `sim-core` from its streams.
//!
//! The bot playtester ([`sortie::SortieTester`]) is an instrument, like the virtual tester it
//! wraps: it reads exact state.
#![deny(missing_docs)]

pub mod bandit;
pub mod geometry;
pub mod hold;
pub mod maneuver;
pub mod percept;
pub mod skill;
pub mod sortie;

pub use bandit::{BanditBrain, BanditState, BrainKind, BrainOut};
pub use hold::HoldCourse;
pub use maneuver::{Airframe, Goal};
pub use percept::{Contact, Dice, OwnView, Percept};
pub use skill::SkillParams;
pub use sortie::{BanditView, OwnInputs, SortieOut, SortiePhase, SortieTester};

#[cfg(test)]
pub(crate) mod test_support;
