//! What the bandit knows (GDD 19.9, ADR-47's perception stub), built every tick into an
//! `ai_pilot::Percept`.
//!
//! - **The track ring**: the player's state every six ticks, 32 entries (3.2 s), so the bandit
//!   can see the player as it was its skill's perception delay ago.
//! - **GCI**: once the bandit is active, every 4 s the player's position with 300 m of noise on
//!   each axis and its velocity; the percept dead-reckons it to now.
//! - **The eye**: within `R_vis = min(L_eff / θ_min, ln(C0 / C_min) V_meteo / 3.912)` with a clear
//!   terrain line of sight (checked every six ticks), the delayed state.
//! - **Missiles**: a flying missile within the eye's range for its plume (or body, coasting) and in
//!   sight is noticed with the skill's probability at each decision, and stays noticed.

use ai_pilot::{Contact, Dice, OwnView, Percept};
use flight::aircraft::AircraftModel;
use weapons::missile::visual_l_eff_m;
use weapons::sight::{Mover, gun_solution};

use super::models::CombatModels;
use super::player::{Kin, angle_between};
use super::{
    ARENA_RADIUS_M, CAP_ALT_MSL_M, CAP_RADIUS_M, Combat, DECISION_EVERY_TICKS, DECISION_PHASE,
    GCI_SIGMA_M, GCI_UPDATE_S, GUN_RANGING_CONE_RAD, GUN_RANGING_M, PERCEPTION_EVERY_TICKS,
    PERCEPTION_RING, SEEKER_LOS_EVERY_TICKS, THETA_MIN_RAD, V_METEO_M,
};
use crate::layout::combat_actor;
use crate::math::DVec3;
use crate::rng::gaussian_pair;
use crate::services::Services;

/// The contrast of an aircraft against the sky, and the least the eye detects. // PHYS: GDD 16.6.2
pub const CONTRAST_C0: f64 = 0.5;
/// See [`CONTRAST_C0`].
pub const CONTRAST_MIN: f64 = 0.05;
/// Koschmieder's constant, `ln 50`. // PHYS: GDD 16.6.2
pub const KOSCHMIEDER: f64 = 3.912;
/// The terrain clearance ahead is sampled this far apart along the path, s ... // ALGO: ADR-47
pub const CLEARANCE_STEP_S: f64 = 1.5;
/// ... this many steps ahead (7.5 s). // ALGO: ADR-47
pub const CLEARANCE_STEPS: u32 = 5;

/// One entry of the track ring.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Seen {
    /// The tick it was taken.
    pub tick: u64,
    /// The player's reference point, BF.
    pub pos_bf: DVec3,
    /// Its velocity, BF.
    pub vel_bf: DVec3,
    /// Its nose, BF.
    pub nose_bf: DVec3,
}

/// A GCI report.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Gci {
    /// When, s.
    pub t_s: f64,
    /// The reported position, BF.
    pub pos_bf: DVec3,
    /// The reported velocity, BF.
    pub vel_bf: DVec3,
}

/// The bandit's knowledge.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Perception {
    /// The track ring.
    pub ring: [Seen; PERCEPTION_RING],
    /// Where the next entry goes.
    pub head: u8,
    /// Entries filled.
    pub len: u8,
    /// The last GCI report.
    pub gci: Option<Gci>,
    /// The terrain line of sight to the player was clear at the last check.
    pub los_player: bool,
    /// The least terrain clearance ahead at the last decision, m.
    pub clearance_min_m: f64,
    /// It perceived the player at the last tick.
    pub sees: bool,
}

impl Default for Perception {
    fn default() -> Self {
        Self::new()
    }
}

impl Perception {
    /// Nothing known.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            ring: [Seen {
                tick: 0,
                pos_bf: DVec3::ZERO,
                vel_bf: DVec3::ZERO,
                nose_bf: DVec3::ZERO,
            }; PERCEPTION_RING],
            head: 0,
            len: 0,
            gci: None,
            los_player: false,
            clearance_min_m: f64::MAX,
            sees: false,
        }
    }

    /// A new bandit's eyes (the track ring and the GCI report are the controller's and stay).
    pub fn reset(&mut self) {
        self.los_player = false;
        self.clearance_min_m = f64::MAX;
        self.sees = false;
    }

    /// The newest entry taken at or before `tick`.
    #[must_use]
    pub fn at_or_before(&self, tick: u64) -> Option<&Seen> {
        let n = PERCEPTION_RING;
        (0..usize::from(self.len))
            .map(|k| &self.ring[(usize::from(self.head) + n - 1 - k) % n])
            .find(|s| s.tick <= tick)
    }
}

/// The eye's range for an object of effective size `l_eff_m`, m (GDD 16.6.2).
#[must_use]
pub fn visual_range_m(l_eff_m: f64) -> f64 {
    let contrast = crate::dmath::ln(CONTRAST_C0 / CONTRAST_MIN) * V_METEO_M / KOSCHMIEDER;
    (l_eff_m / THETA_MIN_RAD).min(contrast)
}

/// The least height above the terrain along the straight path ahead, m.
#[must_use]
pub fn clearance_ahead(svc: &Services, pos_bf: DVec3, vel_bf: DVec3) -> f64 {
    let mut least = f64::INFINITY;
    for k in 0..=CLEARANCE_STEPS {
        let p = pos_bf + vel_bf * (f64::from(k) * CLEARANCE_STEP_S);
        least = least.min(svc.geo.height_msl_m(p) - svc.geo.ground_height_msl_m(p));
    }
    least
}

/// The perception's work at the start of the tick: the track ring, the GCI report, the line of
/// sight, the terrain ahead and the missiles noticed (at decisions).
pub(crate) fn update(
    c: &mut Combat,
    models: &CombatModels,
    svc: &Services,
    player: Option<&Kin>,
    bandit: &Kin,
    tick: u64,
) {
    let t = crate::time::sim_time(tick);
    let p = &mut c.perception;
    if let Some(pk) = player {
        if tick.is_multiple_of(PERCEPTION_EVERY_TICKS) {
            let n = PERCEPTION_RING;
            p.ring[usize::from(p.head) % n] = Seen {
                tick,
                pos_bf: pk.ref_bf,
                vel_bf: pk.vel_bf,
                nose_bf: pk.nose_bf(),
            };
            p.head = ((usize::from(p.head) + 1) % n) as u8;
            p.len = (usize::from(p.len) + 1).min(n) as u8;
        }
        let every = u64::from(super::ticks(GCI_UPDATE_S));
        if c.activated && c.gci && tick.is_multiple_of(every) {
            let (n1, n2) = gaussian_pair(&mut c.rng.bandit);
            let (n3, _) = gaussian_pair(&mut c.rng.bandit);
            p.gci = Some(Gci {
                t_s: t,
                pos_bf: pk.ref_bf + DVec3::new(n1, n2, n3) * GCI_SIGMA_M,
                vel_bf: pk.vel_bf,
            });
        }
        if tick.is_multiple_of(SEEKER_LOS_EVERY_TICKS) {
            p.los_player = svc.geo.los(bandit.ref_bf, pk.ref_bf, 1.0).visible;
        }
    } else {
        p.los_player = false;
    }
    let deciding = tick % DECISION_EVERY_TICKS == DECISION_PHASE;
    if deciding || p.clearance_min_m >= f64::MAX {
        p.clearance_min_m = clearance_ahead(svc, bandit.ref_bf, bandit.vel_bf);
    }
    if deciding {
        notice_missiles(c, models, svc, bandit);
    }
}

/// At a decision: each flying missile not yet noticed, in sight and in the eye's range, is
/// noticed with the skill's probability.
fn notice_missiles(c: &mut Combat, models: &CombatModels, svc: &Services, bandit: &Kin) {
    let prob = c.skill.notice_prob;
    for s in c.missiles.iter_mut().filter(|s| s.flying() && !s.noticed) {
        let range = (s.m.pos - bandit.ref_bf).length();
        let seen = s.owner == combat_actor::PLAYER as u8
            && range <= visual_range_m(visual_l_eff_m(&s.m, &models.missile))
            && svc.geo.los(bandit.ref_bf, s.m.pos, 1.0).visible;
        if seen && c.rng.bandit.next_f64() < prob {
            s.noticed = true;
        }
    }
}

/// The percept for this tick. `own` is the bandit's air data (CAS, TAS, height above the ground).
#[allow(clippy::too_many_arguments)] // ALGO: both aircraft, the models and the world
pub(crate) fn percept(
    c: &mut Combat,
    models: &CombatModels,
    model: &AircraftModel,
    svc: &Services,
    bandit: &Kin,
    own: (f64, f64, f64),
    player: Option<&Kin>,
    tick: u64,
) -> Percept {
    let t = crate::time::sim_time(tick);
    let (cas, tas, agl) = own;
    let up = svc.geo.up_bf(bandit.ref_bf);
    let contact = if c.activated {
        player.and_then(|pk| contact(c, models, bandit, pk, tick, t))
    } else {
        None
    };
    c.perception.sees = contact.is_some();
    let threat_ata_rad = contact
        .filter(|k| k.visual)
        .and_then(|_| delayed(c, tick))
        .map(|s| ai_pilot::geometry::ata_rad(s.nose_bf, s.pos_bf, bandit.ref_bf));
    let missile = c
        .missiles
        .iter()
        .filter(|s| s.flying() && s.noticed)
        .min_by(|a, b| {
            let da = (a.m.pos - bandit.ref_bf).length_squared();
            let db = (b.m.pos - bandit.ref_bf).length_squared();
            da.total_cmp(&db)
        })
        .map(|s| Contact {
            pos_bf: s.m.pos,
            vel_bf: s.m.vel,
            visual: true,
            age_s: 0.0,
        });
    let gun_aim_point_bf = contact.filter(|k| k.visual).and(player).and_then(|pk| {
        let rel = pk.ref_bf - bandit.ref_bf;
        (rel.length() <= GUN_RANGING_M
            && angle_between(bandit.nose_bf(), rel) <= GUN_RANGING_CONE_RAD)
            .then(|| {
                gun_solution(
                    &models.gun,
                    &models.mount,
                    &bandit.shooter(),
                    &Mover {
                        pos_bf: pk.ref_bf,
                        vel_bf: pk.vel_bf,
                    },
                    &svc.flight_env(),
                    model.atmosphere(),
                    t,
                )
            })
            .flatten()
            .map(|s| s.aim_point_bf)
    });
    let r = &mut c.rng.bandit;
    let (g0, g1) = gaussian_pair(r);
    let (g2, g3) = gaussian_pair(r);
    let dice = Dice {
        u: [r.next_f64(), r.next_f64(), r.next_f64(), r.next_f64()],
        g: [g0, g1, g2, g3],
    };
    let home = DVec3::ZERO;
    Percept {
        t_s: t,
        own: OwnView {
            pos_bf: bandit.ref_bf,
            vel_bf: bandit.vel_bf,
            att: bandit.att,
            cas_mps: cas,
            tas_mps: tas,
            agl_m: agl,
            alt_msl_m: svc.geo.height_msl_m(bandit.ref_bf),
            up_bf: up,
            n_max: model.limits.n_max,
        },
        contact,
        threat_ata_rad,
        missile,
        gun_aim_point_bf,
        clearance_min_m: c.perception.clearance_min_m,
        home_bf: home,
        arena_radius_m: ARENA_RADIUS_M,
        from_home_m: DVec3::new(bandit.ref_bf.x, bandit.ref_bf.y, 0.0).length(),
        cap_centre_bf: c.cap_centre_bf,
        cap_radius_m: CAP_RADIUS_M,
        cap_alt_msl_m: CAP_ALT_MSL_M,
        dice,
    }
}

/// The player as the bandit saw it a perception delay ago.
fn delayed(c: &Combat, tick: u64) -> Option<Seen> {
    let delay = u64::from(super::ticks(c.skill.perception_delay_s));
    c.perception.at_or_before(tick.checked_sub(delay)?).copied()
}

/// The player by eye (the delayed state, in sight and in range), else by GCI (dead-reckoned).
fn contact(
    c: &Combat,
    models: &CombatModels,
    bandit: &Kin,
    _player: &Kin,
    tick: u64,
    t: f64,
) -> Option<Contact> {
    let eye = delayed(c, tick).filter(|s| {
        c.perception.los_player
            && (s.pos_bf - bandit.ref_bf).length() <= visual_range_m(models.visual_l_eff_m)
    });
    if let Some(s) = eye {
        return Some(Contact {
            pos_bf: s.pos_bf,
            vel_bf: s.vel_bf,
            visual: true,
            age_s: c.skill.perception_delay_s,
        });
    }
    let g = c.perception.gci.filter(|_| c.gci)?;
    let age = (t - g.t_s).max(0.0);
    Some(Contact {
        pos_bf: g.pos_bf + g.vel_bf * age,
        vel_bf: g.vel_bf,
        visual: false,
        age_s: age,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_eye_sees_the_kestrel_to_7_8_km_and_a_plume_to_3_3_km() {
        // 7.0 m / 0.9 mrad = 7 778 m; 3 m = 3 333 m; the contrast limit ln 10 · 40 km / 3.912 =
        // 23 544 m binds only for objects over 21 m.
        assert!((visual_range_m(7.0) - 7_777.777_777_777_777).abs() < 1e-6);
        assert!((visual_range_m(3.0) - 3_333.333_333_333_333).abs() < 1e-6);
        assert!((visual_range_m(100.0) - 23_543.6).abs() < 1.0);
    }

    #[test]
    fn the_ring_gives_the_newest_entry_at_or_before_a_tick() {
        let mut p = Perception::new();
        assert!(p.at_or_before(100).is_none());
        for k in 0..40_u64 {
            let n = PERCEPTION_RING;
            p.ring[usize::from(p.head) % n] = Seen {
                tick: k * 6,
                ..Seen::default()
            };
            p.head = ((usize::from(p.head) + 1) % n) as u8;
            p.len = (usize::from(p.len) + 1).min(n) as u8;
        }
        // Ticks 48 to 234 are kept (the last 32 of 0 to 234).
        assert_eq!(p.at_or_before(1_000).map(|s| s.tick), Some(234));
        assert_eq!(p.at_or_before(100).map(|s| s.tick), Some(96));
        assert_eq!(p.at_or_before(48).map(|s| s.tick), Some(48));
        assert!(p.at_or_before(47).is_none());
    }
}
