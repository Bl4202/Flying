//! The AI skill entry (kind 6): how well a computer pilot flies and fights (GDD 19.6, ADR-47).
//!
//! The simulation finds a skill by its `rank` (1 for the lowest), never by id, so a pack may add
//! or rename levels without a code change. Floats are `f32`.

use crate::codec::{Reader, Writer};
use crate::cpack::CpackError;

/// The pilot flies the nose to the target.
pub const TACTIC_PURE_PURSUIT: u8 = 1;
/// The pilot aims ahead of the target (the gun solution).
pub const TACTIC_LEAD_PURSUIT: u8 = 2;
/// The pilot flies behind the target's turn to keep closure under control.
pub const TACTIC_LAG_PURSUIT: u8 = 4;
/// The pilot can turn hard into an attacker.
pub const TACTIC_BREAK_TURN: u8 = 8;
/// The pilot can jink to spoil a gun solution.
pub const TACTIC_JINK: u8 = 16;
/// The pilot can disengage and extend for speed.
pub const TACTIC_EXTEND: u8 = 32;
/// Every tactic bit in use.
pub const TACTICS_ALL: u8 = TACTIC_PURE_PURSUIT
    | TACTIC_LEAD_PURSUIT
    | TACTIC_LAG_PURSUIT
    | TACTIC_BREAK_TURN
    | TACTIC_JINK
    | TACTIC_EXTEND;

/// What the sim knows about one skill level.
#[derive(Clone, Debug, PartialEq)]
pub struct SkillDef {
    /// Locale key of the display name.
    pub name_key: String,
    /// Position in the ladder, 1 for the lowest. Ranks in a pack are `1..=n`, each once.
    pub rank: u8,
    /// True if the pilot fires its gun.
    pub fires_gun: bool,
    /// Delay between something becoming visible and the pilot acting on it, s.
    pub perception_delay_s: f32,
    /// Probability per decision that the pilot notices an incoming missile it can see.
    pub notice_prob: f32,
    /// Fraction of the aircraft's load-factor limit the pilot will pull.
    pub g_frac: f32,
    /// Standard deviation of the pilot's gun aim error, mrad.
    pub gun_aim_sigma_mrad: f32,
    /// Delay before the pilot reacts to being fired at, s.
    pub defend_delay_s: f32,
    /// Standard deviation of the timing of a defensive break, s.
    pub break_timing_sigma_s: f32,
    /// Lowest height above the ground the pilot will fly at, m.
    pub terrain_min_agl_m: f32,
    /// Shortest time the pilot keeps a decision before changing it, s.
    pub commitment_s: f32,
    /// Calibrated airspeed below which the pilot extends, m/s (0 for never).
    pub extend_below_cas_mps: f32,
    /// Calibrated airspeed at which the pilot stops extending, m/s.
    pub extend_until_cas_mps: f32,
    /// The tactics the pilot knows, as a bit mask of the `TACTIC_*` constants.
    pub tactics: u8,
}

impl SkillDef {
    pub(crate) fn encode(&self, w: &mut Writer<'_>) -> Result<(), CpackError> {
        self.check()?;
        w.str(&self.name_key)?;
        w.u8(self.rank);
        w.bool(self.fires_gun);
        w.u8(self.tactics);
        w.f32s(&[
            self.perception_delay_s,
            self.notice_prob,
            self.g_frac,
            self.gun_aim_sigma_mrad,
            self.defend_delay_s,
            self.break_timing_sigma_s,
            self.terrain_min_agl_m,
            self.commitment_s,
            self.extend_below_cas_mps,
            self.extend_until_cas_mps,
        ]);
        Ok(())
    }

    pub(crate) fn decode(r: &mut Reader<'_>) -> Result<Self, CpackError> {
        let name_key = r.str()?;
        let rank = r.u8()?;
        let fires_gun = r.bool()?;
        let tactics = r.u8()?;
        let def = Self {
            name_key,
            rank,
            fires_gun,
            tactics,
            perception_delay_s: r.f32()?,
            notice_prob: r.f32()?,
            g_frac: r.f32()?,
            gun_aim_sigma_mrad: r.f32()?,
            defend_delay_s: r.f32()?,
            break_timing_sigma_s: r.f32()?,
            terrain_min_agl_m: r.f32()?,
            commitment_s: r.f32()?,
            extend_below_cas_mps: r.f32()?,
            extend_until_cas_mps: r.f32()?,
        };
        def.check()?;
        Ok(def)
    }

    /// What the format itself requires; the baker checks the rest.
    fn check(&self) -> Result<(), CpackError> {
        if self.rank == 0 {
            return Err(CpackError::Record("a skill's rank is zero"));
        }
        if self.tactics & !TACTICS_ALL != 0 {
            return Err(CpackError::Record("a skill has an unknown tactic bit"));
        }
        Ok(())
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::codec::Strings;

    /// A middle skill, like the Regular's.
    pub(crate) fn sample(rank: u8) -> SkillDef {
        SkillDef {
            name_key: format!("skl.rank{rank}.name"),
            rank,
            fires_gun: rank > 1,
            perception_delay_s: 1.0,
            notice_prob: 0.7,
            g_frac: 0.8,
            gun_aim_sigma_mrad: 8.0,
            defend_delay_s: 2.0,
            break_timing_sigma_s: 1.0,
            terrain_min_agl_m: 250.0,
            commitment_s: 2.5,
            extend_below_cas_mps: 0.0,
            extend_until_cas_mps: 0.0,
            tactics: TACTIC_PURE_PURSUIT | TACTIC_LEAD_PURSUIT | TACTIC_BREAK_TURN | TACTIC_JINK,
        }
    }

    fn encode(def: &SkillDef) -> Result<(Vec<u8>, String), CpackError> {
        let mut strings = Strings::default();
        let mut w = Writer::new(&mut strings);
        def.encode(&mut w)?;
        Ok((w.finish(), String::from_utf8(strings.bytes).unwrap()))
    }

    #[test]
    fn a_skill_round_trips() {
        let def = sample(2);
        let (rec, text) = encode(&def).unwrap();
        let mut r = Reader::new(&rec, &text);
        assert_eq!(SkillDef::decode(&mut r).unwrap(), def);
        r.finish().unwrap();
    }

    #[test]
    fn rank_zero_and_unknown_tactic_bits_are_refused() {
        let mut zero = sample(1);
        zero.rank = 0;
        assert!(encode(&zero).is_err());
        let mut odd = sample(1);
        odd.tactics = 64;
        assert!(encode(&odd).is_err());

        // The reader refuses what the writer would have: rank is the byte after the name-key
        // reference, tactics two bytes later.
        let (rec, text) = encode(&sample(2)).unwrap();
        for (at, value) in [(8, 0_u8), (10, 64), (9, 2)] {
            let mut bad = rec.clone();
            bad[at] = value;
            let mut r = Reader::new(&bad, &text);
            assert!(SkillDef::decode(&mut r).is_err(), "byte {at} = {value}");
        }
    }

    #[test]
    fn the_tactic_bits_are_distinct_and_cover_the_mask() {
        let bits = [
            TACTIC_PURE_PURSUIT,
            TACTIC_LEAD_PURSUIT,
            TACTIC_LAG_PURSUIT,
            TACTIC_BREAK_TURN,
            TACTIC_JINK,
            TACTIC_EXTEND,
        ];
        let mut seen = 0_u8;
        for b in bits {
            assert_eq!(b.count_ones(), 1);
            assert_eq!(seen & b, 0);
            seen |= b;
        }
        assert_eq!(seen, TACTICS_ALL);
    }
}
