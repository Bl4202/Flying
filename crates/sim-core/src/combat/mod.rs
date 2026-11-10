//! Combat in the core (ADR-48): one bandit, the player's gun and missiles, the bandit's gun, the
//! rounds and missiles in flight, hits, kills and deaths, and the events the host reads.
//!
//! [`Combat`] is everything combat adds to the world. It exists only in a duel session, is
//! allocated once in `SimCore::start` (fixed arrays, boxed) and is reset by dropping it. The
//! systems in [`systems`] run in the Ai, Dynamics, Weapons, Collision and Systems stages, and each
//! returns at once when there is no combat. The random numbers come from five named streams of
//! the core's master seed; the new crates (`weapons`, `ai-pilot`) get them as inputs.
//!
//! What is hashed: every number of the state (the bandit's slot, input and brain, both health
//! bars, the weapons, the rounds, the missiles, the explosions, the perception, the random
//! streams, the cues the bot reads, the timers). What is not: the event ring and the counters,
//! which are observations ([`hash`]).

pub mod bandit;
pub mod events;
pub mod hash;
pub mod hits;
pub mod missiles;
pub mod models;
pub mod perception;
pub mod player;
pub mod report;
pub mod rounds;
pub mod sortie;
pub(crate) mod systems;
#[cfg(test)]
mod tests;
pub mod view;

use ai_pilot::{BanditBrain, SkillParams};
use flight::aircraft::AircraftModel;
use simmath::dmath;

use crate::layout::combat_actor;
use crate::layout::combat_event;
use crate::math::DVec3;
use crate::ownship::AircraftSlot;
use crate::rng::{RngStreams, Xoshiro256StarStar};
use crate::services::Services;
use crate::session::{Scenario, SessionConfig};
use crate::time::SIM_DT;

pub use ai_pilot::{BanditState, BrainKind, SortiePhase};
pub use bandit::{Bandit, BanditLife};
pub use events::{CombatEvent, EventRing};
pub use missiles::MissileSlot;
pub use models::CombatModels;
pub use perception::Perception;
pub use player::{Cues, GunState, PlayerWeapons};
pub use rounds::{RoundPool, RoundSlot};
pub use view::CombatView;

/// The CAP orbit's centre, BF east, m. // DATA: ADR-47 (test scenario)
pub const CAP_CENTRE_EAST_M: f64 = 0.0;
/// The CAP orbit's centre, BF north, m. // DATA: ADR-47 (test scenario)
pub const CAP_CENTRE_NORTH_M: f64 = 15_000.0;
/// The CAP orbit's radius, m. // DATA: ADR-47 (test scenario)
pub const CAP_RADIUS_M: f64 = 5_000.0;
/// The CAP orbit's height, m above MSL. // DATA: ADR-47 (test scenario)
pub const CAP_ALT_MSL_M: f64 = 4_500.0;
/// The bandit's true airspeed at a spawn, m/s. // DATA: ADR-47 (test scenario)
pub const BANDIT_SPAWN_TAS_MPS: f64 = 200.0;
/// The bandit's patrol speed, m/s CAS (the brain's `PATROL_CAS_MPS`). // DATA: ADR-47 (test scenario)
pub const PATROL_CAS_MPS: f64 = ai_pilot::bandit::PATROL_CAS_MPS;
/// The bandit is told of the player once the player has been airborne this long, s.
/// // DATA: ADR-47 (test scenario)
pub const ACTIVATE_AIRBORNE_S: f64 = 10.0;
/// The bandit returns beyond this distance from the anchor, m. // DATA: ADR-47 (test scenario)
pub const ARENA_RADIUS_M: f64 = 40_000.0;
/// A wreck falls for at most this long before it is gone, s. // DATA: ADR-47 (test scenario)
pub const WRECK_MAX_S: f64 = 30.0;
/// A new bandit spawns this long after the last is gone, s. // DATA: ADR-47 (test scenario)
pub const BANDIT_RESPAWN_S: f64 = 20.0;
/// The GCI cue: how often it reports the player, s. // DATA: ADR-47 (test scenario)
pub const GCI_UPDATE_S: f64 = 4.0;
/// ... and the standard deviation of its position on each axis, m. // DATA: ADR-47 (test scenario)
pub const GCI_SIGMA_M: f64 = 300.0;
/// The meteorological visibility, m. // DATA: ADR-47 (test scenario)
pub const V_METEO_M: f64 = 40_000.0;
/// The smallest angle the eye resolves, rad. // PHYS: GDD 16.6.2
pub const THETA_MIN_RAD: f64 = 0.0009;
/// Weapons arm after this long off the wheels, s. // DATA: ADR-47 (test scenario)
pub const ARM_DELAY_S: f64 = 1.0;
/// The gun ranging stub's range, m. // DATA: ADR-47 (test scenario)
pub const GUN_RANGING_M: f64 = 2_500.0;
/// ... and its cone about the nose, rad (30°). // DATA: ADR-47 (test scenario)
pub const GUN_RANGING_CONE_RAD: f64 = core::f64::consts::FRAC_PI_6;
/// A target is designated within this range, m. // DATA: ADR-47 (test scenario)
pub const TARGET_DESIGNATE_M: f64 = 20_000.0;
/// A crash is the player's kill if the player hit the bandit this recently, s. // DATA: GDD 65.2
pub const KILL_CREDIT_WINDOW_S: f64 = 30.0;
/// Rounds in flight at most (both guns). // DATA: ADR-47 (test scenario)
pub const ROUND_CAPACITY: usize = 192;
/// Missiles in flight at most. // DATA: ADR-47 (test scenario)
pub const MISSILE_CAPACITY: usize = crate::layout::snapshot::MISSILE_CAPACITY;
/// Explosions shown at once. // DATA: ADR-47 (test scenario)
pub const EXPLOSION_CAPACITY: usize = crate::layout::snapshot::EXPLOSION_CAPACITY;
/// Tracers in the snapshot at most. // DATA: ADR-47 (test scenario)
pub const TRACER_CAPACITY: usize = crate::layout::snapshot::TRACER_CAPACITY;
/// Events the ring keeps. // DATA: ADR-47 (test scenario)
pub const EVENT_CAPACITY: usize = crate::layout::snapshot::EVENT_CAPACITY;
/// The perception ring's length (3.2 s at 10 Hz). // DATA: ADR-47 (test scenario)
pub const PERCEPTION_RING: usize = 32;
/// The ring takes the player's state every this many ticks. // DATA: ADR-47 (test scenario)
pub const PERCEPTION_EVERY_TICKS: u64 = 6;
/// The bandit decides every this many ticks (5 Hz) ... // DATA: ADR-47 (test scenario)
pub const DECISION_EVERY_TICKS: u64 = 12;
/// ... at this phase of them. // DATA: ADR-47 (test scenario)
pub const DECISION_PHASE: u64 = 6;
/// The fly-out predictor runs every this many ticks. // DATA: ADR-47 (test scenario)
pub const PREDICTOR_EVERY_TICKS: u64 = 30;
/// The terrain lines of sight (seekers, the bandit's eye) are checked every this many ticks.
/// // DATA: ADR-47 (test scenario)
pub const SEEKER_LOS_EVERY_TICKS: u64 = 6;
/// An explosion is shown this long, s. // DATA: ADR-47 (test scenario)
pub const EXPLOSION_LIFE_S: f64 = 1.0;
/// The `HIT_RECENT` flag holds this long, s. // DATA: ADR-47 (test scenario)
pub const HIT_LATCH_S: f64 = 0.3;
/// The `KILL_RECENT` flag holds this long, s. // DATA: ADR-47 (test scenario)
pub const KILL_LATCH_S: f64 = 2.0;
/// The `PLAYER_HIT` flag holds this long, s. // DATA: ADR-47 (test scenario)
pub const PLAYER_HIT_LATCH_S: f64 = 0.5;
/// The HUD shows a refusal this long (read by the host from `C_INHIBIT_AGE`), s.
/// // DATA: ADR-47 (test scenario)
pub const INHIBIT_SHOW_S: f64 = 1.5;
/// The first bandit of a merge starts this far ahead of the player, m. // DATA: ADR-47 (test scenario)
pub const MERGE_AHEAD_M: f64 = 3_000.0;
/// The predictor is not run against a target farther than this, m (no Dart-1 shot reaches it).
/// // ALGO: ADR-48, a cost bound
pub const PREDICTOR_MAX_RANGE_M: f64 = 12_000.0;

/// Seconds to whole ticks, rounded.
#[must_use]
pub fn ticks(s: f64) -> u32 {
    (s / SIM_DT).round() as u32
}

/// An air start for the gate runners (ADR-48): where, how high, which way and how fast.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AirStart {
    /// BF east, m.
    pub east_m: f64,
    /// BF north, m.
    pub north_m: f64,
    /// Height above MSL, m.
    pub alt_msl_m: f64,
    /// Heading, rad (0 north, clockwise).
    pub heading_rad: f64,
    /// True airspeed, m/s.
    pub tas_mps: f64,
}

/// A gate runner's set-up of a duel (`SimCore::combat_setup`; a tool's knob, not part of the
/// ABI).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CombatSetup {
    /// The player's air start, replacing the session spawn's position.
    pub player: Option<AirStart>,
    /// The first bandit's air start, replacing the CAP spawn.
    pub bandit: Option<AirStart>,
    /// Which brain flies the bandits.
    pub brain: BrainKind,
    /// The bandit is active from the start (no activation wait).
    pub bandit_active: bool,
    /// The GCI cue is on.
    pub gci: bool,
}

impl Default for CombatSetup {
    fn default() -> Self {
        Self {
            player: None,
            bandit: None,
            brain: BrainKind::Normal,
            bandit_active: false,
            gci: true,
        }
    }
}

/// An explosion being shown.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Explosion {
    /// Where, BF.
    pub pos_bf: DVec3,
    /// How long ago, s.
    pub age_s: f64,
    /// `explosion_kind` (0: the entry is empty).
    pub kind: u8,
}

/// The five random streams combat draws from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CombatRng {
    /// The player's gun dispersion.
    pub gun: Xoshiro256StarStar,
    /// The bandit's gun dispersion.
    pub bandit_gun: Xoshiro256StarStar,
    /// The bandit's brain and perception.
    pub bandit: Xoshiro256StarStar,
    /// Seeker noise in flight.
    pub missile: Xoshiro256StarStar,
    /// Where bandits spawn.
    pub spawn: Xoshiro256StarStar,
}

impl CombatRng {
    /// The streams of a core's master seed.
    #[must_use]
    pub fn new(rng: &RngStreams) -> Self {
        Self {
            gun: rng.stream("combat/gun"),
            bandit_gun: rng.stream("combat/bandit-gun"),
            bandit: rng.stream("combat/bandit"),
            missile: rng.stream("combat/missile"),
            spawn: rng.stream("combat/spawn"),
        }
    }
}

/// Where both aircraft were at the start of the tick (for the swept tests), and whether either
/// spawned since (a spawn is a jump, not a path).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PrevPositions {
    /// The player's reference point, BF.
    pub player_ref_bf: DVec3,
    /// The player's spawn counter then.
    pub player_spawn_seq: u8,
    /// The bandit's reference point, BF.
    pub bandit_ref_bf: DVec3,
    /// The bandit's spawn counter then.
    pub bandit_spawn_seq: u16,
}

/// The snapshot's counters (`C_ROUNDS_FIRED` ...): observations, never hashed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CombatCounters {
    /// Rounds the player fired.
    pub rounds_fired: u32,
    /// The player's rounds that hit.
    pub round_hits: u32,
    /// Missiles the player launched.
    pub missiles_fired: u32,
    /// The player's missiles that detonated near the bandit.
    pub missile_hits: u32,
    /// Bandits the player killed.
    pub kills: u32,
    /// ... with the gun.
    pub gun_kills: u32,
    /// ... with a missile.
    pub missile_kills: u32,
    /// Times the player was destroyed in combat (shot down or a collision).
    pub deaths: u32,
    /// The bandit's rounds that hit the player.
    pub hits_taken: u32,
    /// Rounds the bandit fired.
    pub bandit_rounds: u32,
    /// Explosions.
    pub explosions: u32,
    /// Bandits spawned.
    pub bandit_spawns: u32,
}

/// When things last happened, for the snapshot's latched flags and the refusal's age:
/// observations, never hashed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stamps {
    /// The tick one of the player's rounds last hit.
    pub hit_tick: Option<u64>,
    /// The tick of the player's last kill.
    pub kill_tick: Option<u64>,
    /// The tick the player was last hit.
    pub player_hit_tick: Option<u64>,
    /// The tick of the last refused launch, and why (`launch_inhibit`).
    pub inhibit: Option<(u64, u8)>,
}

/// Everything combat adds to the world.
#[derive(Clone, Debug)]
pub struct Combat {
    /// The bandit's skill rank (1 to 3).
    pub rank: u8,
    /// Its skill.
    pub skill: SkillParams,
    /// The GCI cue is on.
    pub gci: bool,
    /// The brain the bandits fly with.
    pub brain_kind: BrainKind,
    /// The bandit has been told of the player (the player airborne 10 s, or a set-up or a merge).
    pub activated: bool,
    /// Ticks the player has been airborne, for the activation.
    pub airborne_ticks: u32,
    /// The CAP centre, BF.
    pub cap_centre_bf: DVec3,
    /// The bandit.
    pub bandit: Bandit,
    /// The player's weapons.
    pub player: PlayerWeapons,
    /// What the player's instruments show (and the bot reads).
    pub cues: Cues,
    /// Rounds in flight.
    pub rounds: RoundPool,
    /// Missiles in flight.
    pub missiles: [MissileSlot; MISSILE_CAPACITY],
    /// Explosions being shown.
    pub explosions: [Explosion; EXPLOSION_CAPACITY],
    /// What the bandit knows of the player.
    pub perception: Perception,
    /// The random streams.
    pub rng: CombatRng,
    /// Both aircraft at the start of the tick.
    pub prev: PrevPositions,
    /// Recent events (not hashed).
    pub events: EventRing,
    /// The counters (not hashed).
    pub stats: CombatCounters,
    /// When things last happened (not hashed).
    pub stamps: Stamps,
}

impl Combat {
    /// The combat of a duel session, its first bandit spawned (the player is already spawned).
    /// Allocates (once, at start).
    #[must_use]
    #[allow(clippy::too_many_arguments)] // ALGO: the session, the world and the player, unbundled
    pub fn new(
        models: &CombatModels,
        skill: SkillParams,
        session: &SessionConfig,
        setup: &CombatSetup,
        rng: &RngStreams,
        player: &AircraftSlot,
        model: &AircraftModel,
        svc: &Services,
    ) -> Box<Self> {
        let cap = DVec3::new(CAP_CENTRE_EAST_M, CAP_CENTRE_NORTH_M, 0.0);
        let cap_z = crate::spawn::z_at_height(&*svc.geo, cap.x, cap.y, CAP_ALT_MSL_M);
        let merge = session.scenario == Scenario::DuelMerge;
        let mut c = Box::new(Self {
            rank: session.skill,
            skill,
            gci: setup.gci,
            brain_kind: setup.brain,
            activated: setup.bandit_active || merge,
            airborne_ticks: 0,
            cap_centre_bf: DVec3::new(cap.x, cap.y, cap_z),
            bandit: Bandit::new(model, session, models),
            player: PlayerWeapons::new(models, player.spawn_seq),
            cues: Cues::default(),
            rounds: RoundPool::new(),
            missiles: [MissileSlot::default(); MISSILE_CAPACITY],
            explosions: [Explosion::default(); EXPLOSION_CAPACITY],
            perception: Perception::new(),
            rng: CombatRng::new(rng),
            prev: PrevPositions::default(),
            events: EventRing::new(),
            stats: CombatCounters::default(),
            stamps: Stamps::default(),
        });
        let start = setup.bandit.or_else(|| {
            merge.then(|| {
                let p = model.ref_point_bf(&player.state);
                let v = player.state.body.vel_bf;
                let heading = dmath::atan2(v.x, v.y);
                AirStart {
                    east_m: p.x + MERGE_AHEAD_M * dmath::sin(heading),
                    north_m: p.y + MERGE_AHEAD_M * dmath::cos(heading),
                    alt_msl_m: svc.geo.height_msl_m(p),
                    heading_rad: heading,
                    tas_mps: BANDIT_SPAWN_TAS_MPS,
                }
            })
        });
        c.spawn_bandit(start, models, model, svc, 0);
        c
    }

    /// A new bandit: at `start`, or on the CAP orbit at a bearing from the spawn stream, heading
    /// clockwise round it.
    pub(crate) fn spawn_bandit(
        &mut self,
        start: Option<AirStart>,
        models: &CombatModels,
        model: &AircraftModel,
        svc: &Services,
        tick: u64,
    ) {
        let s = start.unwrap_or_else(|| {
            let bearing = core::f64::consts::TAU * self.rng.spawn.next_f64();
            AirStart {
                east_m: CAP_CENTRE_EAST_M + CAP_RADIUS_M * dmath::sin(bearing),
                north_m: CAP_CENTRE_NORTH_M + CAP_RADIUS_M * dmath::cos(bearing),
                alt_msl_m: CAP_ALT_MSL_M,
                heading_rad: (bearing + core::f64::consts::FRAC_PI_2)
                    .rem_euclid(core::f64::consts::TAU),
                tas_mps: BANDIT_SPAWN_TAS_MPS,
            }
        });
        self.bandit.spawn(&s, self.brain_kind, model, svc, models);
        self.perception.reset();
        self.stats.bandit_spawns = self.stats.bandit_spawns.wrapping_add(1);
        self.events.push(
            combat_event::BANDIT_SPAWN as u8,
            combat_actor::BANDIT as u8,
            combat_actor::NONE as u8,
            self.rank,
            0.0,
            tick,
        );
    }

    /// The brain flying the bandit now.
    #[must_use]
    pub fn brain(&self) -> &BanditBrain {
        &self.bandit.brain
    }
}
