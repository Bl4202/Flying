//! Skill levels (GDD 19.6, ADR-47): the `ai_skill/1` entry in SI units and radians.
//!
//! Skill changes behaviour only: never the aircraft's performance, its sensors or its weapons
//! (GDD 19.6). The load-factor cap is `g_frac × n_max` (GDD 19.12).

use content::SkillDef;
pub use content::skill::{
    TACTIC_BREAK_TURN, TACTIC_EXTEND, TACTIC_JINK, TACTIC_LAG_PURSUIT, TACTIC_LEAD_PURSUIT,
    TACTIC_PURE_PURSUIT,
};

/// One skill level's parameters.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SkillParams {
    /// 1 Recruit, 2 Regular, 3 Veteran.
    pub rank: u8,
    /// Whether it fires the gun at all (the Recruit does not: owner decision D3).
    pub fires_gun: bool,
    /// How old what it sees is, s.
    pub perception_delay_s: f64,
    /// Chance per decision of noticing a threat it could see.
    pub notice_prob: f64,
    /// The share of the airframe's load limit it will pull.
    pub g_frac: f64,
    /// The load factor it pulls at most, g.
    pub n_cap: f64,
    /// Standard deviation of its gun aim error, rad.
    pub gun_aim_sigma_rad: f64,
    /// Time before it reacts to a threat, s.
    pub defend_delay_s: f64,
    /// Standard deviation of the timing of its last-ditch break, s.
    pub break_timing_sigma_s: f64,
    /// The lowest it flies above the terrain on its own, m.
    pub terrain_min_agl_m: f64,
    /// How long it holds a decision before another (except the urgent ones), s.
    pub commitment_s: f64,
    /// It extends to regain energy below this calibrated speed (0: never), m/s.
    pub extend_below_cas_mps: f64,
    /// ... until it is this fast again, m/s.
    pub extend_until_cas_mps: f64,
    /// Its repertoire: a bit set of the `TACTIC_*` constants.
    pub tactics: u8,
}

impl SkillParams {
    /// The parameters of a skill entry for an airframe whose load limit is `n_max` g.
    #[must_use]
    pub fn from_def(d: &SkillDef, n_max: f64) -> Self {
        let f = f64::from;
        Self {
            rank: d.rank,
            fires_gun: d.fires_gun,
            perception_delay_s: f(d.perception_delay_s),
            notice_prob: f(d.notice_prob),
            g_frac: f(d.g_frac),
            n_cap: f(d.g_frac) * n_max,
            gun_aim_sigma_rad: f(d.gun_aim_sigma_mrad) * 1e-3, // PHYS: mrad to rad
            defend_delay_s: f(d.defend_delay_s),
            break_timing_sigma_s: f(d.break_timing_sigma_s),
            terrain_min_agl_m: f(d.terrain_min_agl_m),
            commitment_s: f(d.commitment_s),
            extend_below_cas_mps: f(d.extend_below_cas_mps),
            extend_until_cas_mps: f(d.extend_until_cas_mps),
            tactics: d.tactics,
        }
    }

    /// Whether a tactic is in its repertoire.
    #[must_use]
    pub const fn has(&self, tactic: u8) -> bool {
        self.tactics & tactic != 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::skill_def;

    #[test]
    fn the_three_skills_from_their_entries() {
        let n_max = 9.0;
        let [r, g, v] = [1, 2, 3].map(|k| SkillParams::from_def(skill_def(k), n_max));
        assert_eq!((r.rank, g.rank, v.rank), (1, 2, 3));
        assert_eq!((r.fires_gun, g.fires_gun, v.fires_gun), (false, true, true));
        assert!((r.n_cap - 0.65 * 9.0).abs() < 1e-6 && (v.n_cap - 0.9 * 9.0).abs() < 1e-6);
        assert!((g.gun_aim_sigma_rad - 8e-3).abs() < 1e-9);
        assert_eq!(
            (r.perception_delay_s, v.defend_delay_s),
            (1.5, f64::from(1.2f32))
        );
        assert!(
            r.has(TACTIC_PURE_PURSUIT) && r.has(TACTIC_BREAK_TURN) && !r.has(TACTIC_LEAD_PURSUIT)
        );
        assert!(g.has(TACTIC_LEAD_PURSUIT) && g.has(TACTIC_JINK) && !g.has(TACTIC_LAG_PURSUIT));
        assert!(v.has(TACTIC_LAG_PURSUIT) && v.has(TACTIC_EXTEND));
        assert_eq!(
            (
                r.extend_below_cas_mps,
                v.extend_below_cas_mps,
                v.extend_until_cas_mps
            ),
            (0.0, 140.0, 220.0)
        );
        assert_eq!(
            (
                r.terrain_min_agl_m,
                g.terrain_min_agl_m,
                v.terrain_min_agl_m
            ),
            (300.0, 250.0, 150.0)
        );
    }
}
