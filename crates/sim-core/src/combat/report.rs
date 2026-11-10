//! The duel gates' runners and judges (ADR-45, ADR-48): the bot's sortie (BP1, BP2), the missile
//! sweep (MS5) and the bandit's gates (AI1, AI2, AI4, AI5, AI8). Each run is one core from a
//! pack and a seed, so a tool can fly them on many threads; each is judged from what the
//! snapshot shows (the player's flight, the event ring) and, for the bandit's gates, from the
//! bandit's exact state (the gates are about its airframe, which the snapshot does not carry).

use crate::circuit::report::{Sample, sample_of};
use crate::layout::{
    combat_actor, combat_event, crash_cause, input_script, kill_weapon, snapshot as snap,
};
use crate::math::DVec3;
use crate::rng::Xoshiro256StarStar;
use crate::session::{Arena, Assist, GEffects, Scenario, SessionConfig, Spawn};
use crate::sim::SimCore;
use crate::time::SIM_DT;

use super::{AirStart, BanditLife, BrainKind, CombatSetup, SortiePhase};

/// The sortie is cut off here, ticks (600 s). // DATA: ADR-45 BP1
pub const SORTIE_TICKS: u64 = 36_000;
/// The runway's half width the stop must be within, m. // DATA: ADR-45 BP1
pub const RUNWAY_HALF_WIDTH_M: f64 = 22.5;
/// The runway's length, m. // DATA: ADR-45 BP1
pub const RUNWAY_LENGTH_M: f64 = 3_000.0;
/// Stopped below this ground speed, m/s. // DATA: ADR-45 BP1
pub const STOPPED_MPS: f64 = 2.0;
/// BP1's successes needed of 50 in each assist. // DATA: ADR-45 BP1
pub const BP1_NEEDED: usize = 40;
/// The bandit's CAS may not exceed `V_mo` by more than this factor. // DATA: ADR-45 AI4
pub const VMO_MARGIN: f64 = 1.01;
/// AI1: the player starts this far north of the bandit, m. // DATA: ADR-45 AI1
pub const AI1_START_NORTH_M: f64 = 8_000.0;
/// AI1 and AI2: the height, m above MSL. // DATA: ADR-45 AI1, AI2
pub const AI_ALT_MSL_M: f64 = 4_000.0;
/// AI1 and AI2: the speed, m/s TAS. // DATA: ADR-45 AI1, AI2
pub const AI_TAS_MPS: f64 = 200.0;
/// AI1: the time allowed, s. // DATA: ADR-45 AI1
pub const AI1_TIME_S: f64 = 180.0;
/// AI1's gun position: range, m ... // DATA: ADR-45 AI1
pub const AI1_RANGE_M: f64 = 1_000.0;
/// ... the bandit's antenna train angle, rad (10°) ... // DATA: ADR-45 AI1
pub const AI1_ATA_RAD: f64 = 0.174_532_925_199_432_96;
/// ... and the aspect angle, rad (60°). // DATA: ADR-45 AI1
pub const AI1_AA_RAD: f64 = core::f64::consts::FRAC_PI_3;
/// AI1: the share of runs per skill that must reach it. // DATA: ADR-45 AI1
pub const AI1_NEEDED: f64 = 0.95;
/// AI8: the share of AI1 runs a Regular or Veteran must fire in. // DATA: ADR-45 AI8
pub const AI8_FIRE_NEEDED: f64 = 0.8;
/// AI2: the bot starts this far behind the bandit, m. // DATA: ADR-45 AI2
pub const AI2_BEHIND_M: f64 = 1_500.0;
/// AI2: the run's length, s. // DATA: ADR-45 AI2
pub const AI2_TIME_S: f64 = 30.0;
/// AI2's gun solution: range, m ... // DATA: ADR-45 AI2
pub const AI2_RANGE_M: f64 = 1_000.0;
/// ... and the pipper's error, rad. // DATA: ADR-45 AI2
pub const AI2_AIM_RAD: f64 = 0.006;
/// MS5: both aircraft's speed, m/s TAS. // DATA: ADR-45 MS5
pub const SWEEP_TAS_MPS: f64 = 230.0;
/// MS5: the height, m above MSL. // DATA: ADR-45 MS5
pub const SWEEP_ALT_MSL_M: f64 = 4_000.0;
/// MS5: the target's aspects off its tail, rad (0° and 30°). // DATA: ADR-45 MS5
pub const SWEEP_ASPECTS_RAD: [f64; 2] = [0.0, core::f64::consts::FRAC_PI_6];
/// MS5: the ranges, m. // DATA: ADR-45 MS5
pub const SWEEP_RANGES_M: [f64; 3] = [1_000.0, 2_000.0, 3_000.0];
/// MS5: shots per cell. // DATA: ADR-45 MS5
pub const SWEEP_SHOTS: u32 = 200;
/// MS5: the launch position's jitter, ±m on each axis. // DATA: ADR-45 MS5
pub const SWEEP_JITTER_M: f64 = 30.0;
/// MS5: a shot waits this long for the lock, s ... // ALGO: ADR-48
pub const SWEEP_LOCK_WAIT_S: f64 = 10.0;
/// ... and this long for the missile to end, s (its 25 s and a margin). // ALGO: ADR-48
pub const SWEEP_FLIGHT_WAIT_S: f64 = 27.0;
/// MS5: hits needed in a cell the predictor marks `IN RNG` (non-manoeuvring). // DATA: ADR-45 MS5
pub const SWEEP_HIT_NEEDED: f64 = 0.85;
/// MS5: a defending target is hit in fewer than this share of shots in every cell ... // DATA: ADR-45 MS5
pub const SWEEP_DEFEND_MAX: f64 = 0.95;
/// ... and in more than this share in at least one. // DATA: ADR-45 MS5
pub const SWEEP_DEFEND_MIN: f64 = 0.05;

/// One event read from the snapshot's ring.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SnapEvent {
    /// Sequence number.
    pub seq: u32,
    /// The tick, mod 2³².
    pub tick: u32,
    /// `combat_event`.
    pub kind: u8,
    /// `combat_actor`.
    pub actor: u8,
    /// `combat_actor`.
    pub target: u8,
    /// The kind's detail.
    pub arg: u8,
    /// The kind's value.
    pub value: f32,
}

/// The snapshot's events newer than `after`, oldest first, to `f`; returns the newest sequence.
pub fn read_events(b: &[u8], after: u32, mut f: impl FnMut(&SnapEvent)) -> u32 {
    let u32_at = |o: usize| u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]);
    let newest = u32_at(snap::C_EVENT_SEQ);
    let first = after.max(newest.saturating_sub(snap::EVENT_CAPACITY as u32)) + 1;
    for seq in first..=newest {
        let at = snap::EVENTS + ((seq as usize - 1) % snap::EVENT_CAPACITY) * snap::EVENT_STRIDE;
        if u32_at(at + snap::EV_SEQ) == seq {
            f(&SnapEvent {
                seq,
                tick: u32_at(at + snap::EV_TICK),
                kind: b[at + snap::EV_KIND],
                actor: b[at + snap::EV_ACTOR],
                target: b[at + snap::EV_TARGET],
                arg: b[at + snap::EV_ARG],
                value: f32::from_bits(u32_at(at + snap::EV_VALUE)),
            });
        }
    }
    newest
}

/// What a run saw of the bandit's airframe and gun (AI4, AI5, AI8).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BanditWatch {
    /// Its structure reported OVERSTRESS while it was alive.
    pub overstress: bool,
    /// Its highest CAS over `V_mo` while alive.
    pub max_cas_over_vmo: f64,
    /// Times a living bandit met the ground.
    pub live_ground_impacts: u32,
    /// Rounds the bandits fired.
    pub rounds_fired: u32,
    /// Bandits spawned.
    pub bandits: u32,
}

impl BanditWatch {
    /// One tick of a duel core.
    pub fn observe(&mut self, core: &SimCore) {
        let (Some(c), Some(model)) = (core.combat(), core.services().aircraft.as_ref()) else {
            return;
        };
        if c.bandit.life == BanditLife::Alive {
            self.overstress |= c.bandit.slot.warnings.overstress;
            self.max_cas_over_vmo = self
                .max_cas_over_vmo
                .max(c.bandit.cas_mps / model.limits.vmo_mps);
        }
        self.rounds_fired = c.stats.bandit_rounds;
        self.bandits = c.stats.bandit_spawns;
    }

    /// An event of the run.
    pub fn event(&mut self, e: &SnapEvent) {
        if e.kind == combat_event::BANDIT_CRASH as u8 {
            self.live_ground_impacts += 1;
        }
    }

    /// AI4: the airframe was never overstressed or over `V_mo × 1.01`.
    #[must_use]
    pub fn same_physics(&self) -> bool {
        !self.overstress && self.max_cas_over_vmo <= VMO_MARGIN
    }
}

/// A started duel core.
///
/// # Errors
/// A message when the pack, the session, the set-up or the script is refused.
pub fn duel_core(
    pack: &[u8],
    session: SessionConfig,
    setup: Option<&CombatSetup>,
    script: usize,
    seed: u64,
) -> Result<SimCore, String> {
    let mut core = SimCore::new(seed);
    core.load_content(pack)
        .map_err(|e| format!("cannot load content: {e}"))?;
    core.configure(session)
        .map_err(|e| format!("cannot configure the duel: {e}"))?;
    if let Some(s) = setup
        && !core.combat_setup(*s)
    {
        return Err("the combat set-up was refused".into());
    }
    if !core.set_input_script(script as u32) {
        return Err(format!("the script {script} was refused"));
    }
    if !core.start() {
        return Err("the core would not start".into());
    }
    Ok(core)
}

/// A duel session.
#[must_use]
pub fn duel_session(spawn: Spawn, assist: Assist, arena: Arena, skill: u8) -> SessionConfig {
    SessionConfig {
        spawn,
        assist,
        g_effects: GEffects::Full,
        arena,
        scenario: Scenario::Duel,
        skill,
    }
}

/// How a bot sortie ended (BP1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortieOutcome {
    /// Lifted off, killed the bandit, stopped on the runway, never destroyed.
    Success,
    /// It never left the runway.
    NoLiftOff,
    /// The player was destroyed.
    Destroyed,
    /// No kill credited to the player.
    NoKill,
    /// It did not stop after the kill.
    NotStopped,
    /// It stopped off the runway.
    OffRunway,
}

impl SortieOutcome {
    /// A short name for tables and JSON.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::NoLiftOff => "no lift-off",
            Self::Destroyed => "destroyed",
            Self::NoKill => "no kill",
            Self::NotStopped => "not stopped",
            Self::OffRunway => "stopped off the runway",
        }
    }
}

/// One bot sortie, judged.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SortieReport {
    /// The outcome.
    pub outcome: SortieOutcome,
    /// Why the bot was destroyed (`crash_cause`; NONE unless the outcome is `Destroyed`).
    pub crash_cause: u8,
    /// Lift-off, s.
    pub lift_off_s: Option<f64>,
    /// The first kill credited to the player, s, and its `kill_weapon`.
    pub kill: Option<(f64, u8)>,
    /// The stop after the kill, s, and where (along, across), m.
    pub stop: Option<(f64, f64, f64)>,
    /// The player's deaths.
    pub deaths: u32,
    /// Rounds the player fired.
    pub rounds_fired: u32,
    /// Missiles the player launched.
    pub missiles_fired: u32,
    /// The player's missiles that detonated near the bandit.
    pub missile_hits: u32,
    /// The bandit's rounds that hit the player.
    pub hits_taken: u32,
    /// Kills credited to the player.
    pub kills: u32,
    /// When the run ended, s.
    pub end_s: f64,
    /// The bot's phase at the end.
    pub phase: SortiePhase,
    /// The bandit's side.
    pub bandit: BanditWatch,
}

/// The extra trace columns of a duel (10 Hz CSV): the player's gun, the bandit's nearest round,
/// the seeker, the launches, the hits taken and the explosions.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CombatTrace {
    /// The gun is firing.
    pub gun_firing: bool,
    /// The nearest of the bandit's rounds, m (−1: none).
    pub enemy_gun_dist_m: f64,
    /// `seeker_mode`.
    pub seeker: u8,
    /// The seeker's signal ratio.
    pub signal: f64,
    /// Missiles launched so far.
    pub launches: u32,
    /// Hits taken so far.
    pub hits_taken: u32,
    /// Explosions so far.
    pub explosions: u32,
    /// The newest explosion's distance, m (−1: none showing).
    pub explosion_dist_m: f64,
}

/// The duel's trace columns now.
#[must_use]
pub fn combat_trace(core: &SimCore) -> Option<CombatTrace> {
    let c = core.combat()?;
    let model = core.services().aircraft.as_ref()?;
    let me = model.ref_point_bf(&core.ownship().aircraft()?.state);
    let enemy = c
        .rounds
        .slots
        .iter()
        .filter(|s| s.live() && s.owner == combat_actor::BANDIT as u8)
        .map(|s| (s.round.pos - me).length())
        .fold(f64::INFINITY, f64::min);
    let newest = c
        .explosions
        .iter()
        .filter(|e| e.kind != 0)
        .min_by(|a, b| a.age_s.total_cmp(&b.age_s));
    Some(CombatTrace {
        gun_firing: c.player.gun.firing,
        enemy_gun_dist_m: if enemy.is_finite() { enemy } else { -1.0 },
        seeker: c.player.seeker.mode.code(),
        signal: c.player.signal,
        launches: c.stats.missiles_fired,
        hits_taken: c.stats.hits_taken,
        explosions: c.stats.explosions,
        explosion_dist_m: newest.map_or(-1.0, |e| (e.pos_bf - me).length()),
    })
}

/// Flies the bot's sortie on `core` (started, SORTIE, a runway spawn) to the stop after the kill,
/// its destruction or [`SORTIE_TICKS`], and judges it (BP1). `observe` sees every tick's sample.
pub fn fly_sortie(core: &mut SimCore, mut observe: impl FnMut(&SimCore, &Sample)) -> SortieReport {
    let mut watch = BanditWatch::default();
    let mut seq = 0;
    let mut lift_off_s = None;
    let mut kill = None;
    let mut stop = None;
    let mut destroyed = false;
    let mut cause = crash_cause::NONE as u8;
    let mut stopped_since: Option<f64> = None;
    for _ in 0..SORTIE_TICKS {
        core.step(1);
        let t = core.tick() as f64 * SIM_DT;
        let Some(s) = sample_of(core) else { break };
        observe(core, &s);
        watch.observe(core);
        seq = read_events(core.snapshot_bytes(), seq, |e| {
            watch.event(e);
            if e.kind == combat_event::KILL as u8 && e.actor == combat_actor::PLAYER as u8 {
                kill.get_or_insert((f64::from(e.tick) * SIM_DT, e.arg));
            }
            if e.kind == combat_event::DEATH as u8 {
                destroyed = true;
                cause = e.arg;
            }
        });
        if s.crashed && !destroyed {
            destroyed = true;
            cause = core
                .ownship()
                .aircraft()
                .map_or(crash_cause::NONE as u8, |a| a.crash_cause);
        }
        if destroyed {
            break;
        }
        if lift_off_s.is_none() && !s.wow {
            lift_off_s = Some(t);
        }
        if kill.is_some() && lift_off_s.is_some() && s.wow && s.gs_mps < STOPPED_MPS {
            let t0 = *stopped_since.get_or_insert(t);
            if t - t0 >= 2.0 {
                stop = Some((t0, s.along_m, s.across_m));
                break;
            }
        } else {
            stopped_since = None;
        }
    }
    let outcome = if destroyed {
        SortieOutcome::Destroyed
    } else if lift_off_s.is_none() {
        SortieOutcome::NoLiftOff
    } else if kill.is_none() {
        SortieOutcome::NoKill
    } else {
        match stop {
            None => SortieOutcome::NotStopped,
            Some((_, along, across))
                if across.abs() > RUNWAY_HALF_WIDTH_M
                    || !(0.0..=RUNWAY_LENGTH_M).contains(&along) =>
            {
                SortieOutcome::OffRunway
            }
            Some(_) => SortieOutcome::Success,
        }
    };
    let stats = core.combat().map(|c| c.stats).unwrap_or_default();
    SortieReport {
        outcome,
        crash_cause: cause,
        lift_off_s,
        kill,
        stop,
        deaths: stats.deaths,
        rounds_fired: stats.rounds_fired,
        missiles_fired: stats.missiles_fired,
        missile_hits: stats.missile_hits,
        hits_taken: stats.hits_taken,
        kills: stats.kills,
        end_s: core.tick() as f64 * SIM_DT,
        phase: core
            .input()
            .sortie()
            .map_or(SortiePhase::TakeOff, ai_pilot::SortieTester::phase),
        bandit: watch,
    }
}

/// A bot sortie from the runway against a bandit of `skill` (BP1, BP2).
///
/// # Errors
/// A message when the core cannot be set up.
pub fn bot_sortie(
    pack: &[u8],
    arena: Arena,
    assist: Assist,
    skill: u8,
    seed: u64,
    observe: impl FnMut(&SimCore, &Sample),
) -> Result<SortieReport, String> {
    let session = duel_session(Spawn::Runway, assist, arena, skill);
    let mut core = duel_core(pack, session, None, input_script::SORTIE, seed)?;
    Ok(fly_sortie(&mut core, observe))
}

/// An air start at BF `(east, north)`.
#[must_use]
pub const fn air(
    east_m: f64,
    north_m: f64,
    alt_msl_m: f64,
    heading_rad: f64,
    tas_mps: f64,
) -> AirStart {
    AirStart {
        east_m,
        north_m,
        alt_msl_m,
        heading_rad,
        tas_mps,
    }
}

/// One AI1 run.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PursueReport {
    /// When the bandit first had a gun position, s.
    pub reached_s: Option<f64>,
    /// It fired at least once.
    pub fired: bool,
    /// The bandit's side.
    pub bandit: BanditWatch,
}

/// AI1: the player straight and level heading east, 8 km north of an active bandit heading north;
/// does the bandit reach a gun position within 180 s?
///
/// # Errors
/// A message when the core cannot be set up.
pub fn pursue_run(pack: &[u8], arena: Arena, skill: u8, seed: u64) -> Result<PursueReport, String> {
    let setup = CombatSetup {
        player: Some(air(
            0.0,
            AI1_START_NORTH_M,
            AI_ALT_MSL_M,
            core::f64::consts::FRAC_PI_2,
            AI_TAS_MPS,
        )),
        bandit: Some(air(0.0, 0.0, AI_ALT_MSL_M, 0.0, AI_TAS_MPS)),
        brain: BrainKind::Normal,
        bandit_active: true,
        gci: true,
        unarmed: false,
    };
    let session = duel_session(Spawn::Air, Assist::Standard, arena, skill);
    let mut core = duel_core(pack, session, Some(&setup), input_script::HOLD, seed)?;
    let mut watch = BanditWatch::default();
    let mut seq = 0;
    let mut reached_s = None;
    let ticks = (AI1_TIME_S / SIM_DT).round() as u64;
    for _ in 0..ticks {
        core.step(1);
        watch.observe(&core);
        seq = read_events(core.snapshot_bytes(), seq, |e| watch.event(e));
        if reached_s.is_none() && gun_position(&core) {
            reached_s = Some(core.tick() as f64 * SIM_DT);
        }
    }
    Ok(PursueReport {
        reached_s,
        fired: watch.rounds_fired > 0,
        bandit: watch,
    })
}

/// AI1's gun position: the bandit within 1 000 m of the player, its nose within 10° of it, in the
/// player's rear 60°.
fn gun_position(core: &SimCore) -> bool {
    let (Some(c), Some(model), Some(p)) = (
        core.combat(),
        core.services().aircraft.as_ref(),
        core.ownship().aircraft(),
    ) else {
        return false;
    };
    if c.bandit.life != BanditLife::Alive || p.crashed {
        return false;
    }
    let pp = model.ref_point_bf(&p.state);
    let bp = model.ref_point_bf(&c.bandit.slot.state);
    let rel = pp - bp;
    let ata = ai_pilot::geometry::angle_between(c.bandit.slot.state.body.att.rotate(DVec3::X), rel);
    let aa = weapons::ir::aspect_angle_rad(p.state.body.att.rotate(DVec3::X), pp, bp);
    rel.length() <= AI1_RANGE_M && ata <= AI1_ATA_RAD && aa <= AI1_AA_RAD
}

/// AI2's opponents.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Opponent {
    /// A bandit of this rank with the normal brain.
    Skill(u8),
    /// The same bandit with its brain replaced by "hold course".
    HoldCourse,
}

/// One AI2 run.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DefendReport {
    /// How long the bot held a gun solution, s.
    pub held_s: f64,
    /// The bandit's side.
    pub bandit: BanditWatch,
}

/// AI2: the bot (unarmed, so it tracks rather than kills) 1 500 m behind an active bandit,
/// co-speed and co-height, for 30 s; how long does it hold a gun solution?
///
/// # Errors
/// A message when the core cannot be set up.
pub fn defend_run(
    pack: &[u8],
    arena: Arena,
    opponent: Opponent,
    seed: u64,
) -> Result<DefendReport, String> {
    let (skill, brain) = match opponent {
        Opponent::Skill(k) => (k, BrainKind::Normal),
        Opponent::HoldCourse => (2, BrainKind::HoldCourse),
    };
    let setup = CombatSetup {
        player: Some(air(0.0, -AI2_BEHIND_M, AI_ALT_MSL_M, 0.0, AI_TAS_MPS)),
        bandit: Some(air(0.0, 0.0, AI_ALT_MSL_M, 0.0, AI_TAS_MPS)),
        brain,
        bandit_active: true,
        gci: true,
        unarmed: true,
    };
    let session = duel_session(Spawn::Air, Assist::Standard, arena, skill);
    let mut core = duel_core(pack, session, Some(&setup), input_script::SORTIE, seed)?;
    let mut watch = BanditWatch::default();
    let mut seq = 0;
    let mut held = 0_u32;
    let ticks = (AI2_TIME_S / SIM_DT).round() as u64;
    for _ in 0..ticks {
        core.step(1);
        watch.observe(&core);
        seq = read_events(core.snapshot_bytes(), seq, |e| watch.event(e));
        if solution(&core) {
            held += 1;
        }
    }
    Ok(DefendReport {
        held_s: f64::from(held) * SIM_DT,
        bandit: watch,
    })
}

/// AI2's gun solution: the bandit within 1 000 m and within 6 mrad of the pipper.
fn solution(core: &SimCore) -> bool {
    let (Some(c), Some(model), Some(p)) = (
        core.combat(),
        core.services().aircraft.as_ref(),
        core.ownship().aircraft(),
    ) else {
        return false;
    };
    let Some(pip) = c.cues.pipper_bf else {
        return false;
    };
    if c.bandit.life != BanditLife::Alive {
        return false;
    }
    let me = model.ref_point_bf(&p.state);
    let rel = model.ref_point_bf(&c.bandit.slot.state) - me;
    rel.length() <= AI2_RANGE_M && ai_pilot::geometry::angle_between(pip - me, rel) <= AI2_AIM_RAD
}

/// A cell of the missile sweep.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SweepCell {
    /// The target's aspect off its tail, rad.
    pub aspect_rad: f64,
    /// The range, m.
    pub range_m: f64,
    /// The target defends (a Veteran, `DefendOnly`) or holds its course.
    pub defending: bool,
}

/// One shot of the sweep.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShotReport {
    /// The missile left the rail (the seeker locked in time and the launch was not refused).
    pub launched: bool,
    /// The predictor said `IN RNG` when it left.
    pub in_range: bool,
    /// It detonated near the target.
    pub hit: bool,
}

/// The cell's geometry: where the launcher and the target start, the launcher's nose (its trim
/// angle of attack above its path) on the target.
fn sweep_setup(cell: &SweepCell, alpha_rad: f64, jitter: DVec3) -> CombatSetup {
    let (r, a) = (cell.range_m, cell.aspect_rad);
    let horizontal = r * simmath::dmath::cos(alpha_rad);
    let rise = r * simmath::dmath::sin(alpha_rad);
    // The target flies north from the origin; the launcher sits `a` off its tail.
    let east = -horizontal * simmath::dmath::sin(a) + jitter.x;
    let north = -horizontal * simmath::dmath::cos(a) + jitter.y;
    let alt = SWEEP_ALT_MSL_M - rise + jitter.z;
    let heading = simmath::dmath::atan2(-east, -north).rem_euclid(core::f64::consts::TAU);
    CombatSetup {
        player: Some(air(east, north, alt, heading, SWEEP_TAS_MPS)),
        bandit: Some(air(0.0, 0.0, SWEEP_ALT_MSL_M, 0.0, SWEEP_TAS_MPS)),
        brain: if cell.defending {
            BrainKind::DefendOnly
        } else {
            BrainKind::HoldCourse
        },
        bandit_active: true,
        gci: false,
        unarmed: false,
    }
}

/// The launcher's trimmed angle of attack at the sweep's speed and height.
///
/// # Errors
/// A message when the core cannot be set up.
pub fn sweep_alpha(pack: &[u8], arena: Arena) -> Result<f64, String> {
    let cell = SweepCell {
        aspect_rad: 0.0,
        range_m: SWEEP_RANGES_M[0],
        defending: false,
    };
    let setup = sweep_setup(&cell, 0.0, DVec3::ZERO);
    let session = duel_session(Spawn::Air, Assist::Standard, arena, 3);
    let core = duel_core(pack, session, Some(&setup), input_script::HOLD, 1)?;
    let p = core.ownship().aircraft().ok_or("no aircraft")?;
    let v_body = p.state.body.att.conjugate().rotate(p.state.body.vel_bf);
    Ok(simmath::dmath::atan2(v_body.z, v_body.x))
}

/// One shot of the sweep (`shot` seeds the core and the jitter).
///
/// # Errors
/// A message when the core cannot be set up.
pub fn sweep_shot(
    pack: &[u8],
    arena: Arena,
    cell: &SweepCell,
    alpha_rad: f64,
    shot: u32,
) -> Result<ShotReport, String> {
    let seed = 0x5_EE90_0000_u64 + u64::from(shot);
    let mut rng =
        Xoshiro256StarStar::from_seed(seed ^ cell.range_m.to_bits() ^ cell.aspect_rad.to_bits());
    let mut j = || rng.range_f64(-SWEEP_JITTER_M, SWEEP_JITTER_M);
    let jitter = DVec3::new(j(), j(), j());
    let setup = sweep_setup(cell, alpha_rad, jitter);
    let session = duel_session(Spawn::Air, Assist::Standard, arena, 3);
    let mut core = duel_core(pack, session, Some(&setup), input_script::HOLD_FIRE, seed)?;
    let mut report = ShotReport {
        launched: false,
        in_range: false,
        hit: false,
    };
    let mut seq = 0;
    let wait = ((SWEEP_LOCK_WAIT_S + SWEEP_FLIGHT_WAIT_S) / SIM_DT) as u64;
    let lock_wait = (SWEEP_LOCK_WAIT_S / SIM_DT) as u64;
    for k in 0..wait {
        let in_range = core.combat().is_some_and(|c| c.cues.in_range);
        core.step(1);
        let mut ended = false;
        seq = read_events(core.snapshot_bytes(), seq, |e| match usize::from(e.kind) {
            combat_event::MISSILE_LAUNCH => {
                report.launched = true;
                report.in_range = in_range;
            }
            combat_event::MISSILE_HIT => {
                report.hit = true;
                ended = true;
            }
            combat_event::MISSILE_MISS => ended = true,
            combat_event::KILL if e.arg == kill_weapon::MISSILE as u8 => report.hit = true,
            _ => {}
        });
        if ended || (!report.launched && k >= lock_wait) {
            break;
        }
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::pack;

    #[test]
    fn the_event_reader_gives_each_new_event_once_in_order() {
        let p = pack(8400);
        let session = duel_session(Spawn::Air, Assist::Standard, Arena::Rugged, 2);
        let mut core = duel_core(&p, session, None, input_script::NONE, 3).unwrap();
        core.step(1);
        let mut seen = Vec::new();
        let last = read_events(core.snapshot_bytes(), 0, |e| seen.push(e.kind));
        assert_eq!(
            (last, seen.as_slice()),
            (1, &[combat_event::BANDIT_SPAWN as u8][..])
        );
        let mut again = Vec::new();
        assert_eq!(
            read_events(core.snapshot_bytes(), last, |e| again.push(e.seq)),
            1
        );
        assert_eq!(again, Vec::<u32>::new());
    }

    #[test]
    fn the_sweep_puts_the_target_on_the_launchers_nose() {
        // The launcher heads at the target, which sits the trim angle of attack above its path.
        let cell = SweepCell {
            aspect_rad: core::f64::consts::FRAC_PI_6,
            range_m: 2_000.0,
            defending: false,
        };
        let s = sweep_setup(&cell, 0.05, DVec3::ZERO);
        let p = s.player.unwrap();
        let back = simmath::dmath::atan2(p.east_m, p.north_m);
        assert!(
            (back - (core::f64::consts::PI + cell.aspect_rad - core::f64::consts::TAU)).abs()
                < 1e-9,
            "{back}"
        );
        assert!(
            (p.heading_rad - cell.aspect_rad).abs() < 1e-9,
            "{}",
            p.heading_rad
        );
        let d = simmath::dmath::hypot(p.east_m, p.north_m);
        assert!((d - 2_000.0 * simmath::dmath::cos(0.05)).abs() < 1e-6);
        assert!((SWEEP_ALT_MSL_M - p.alt_msl_m - 2_000.0 * simmath::dmath::sin(0.05)).abs() < 1e-6);
    }

    #[test]
    fn a_recruit_sortie_succeeds() {
        // A seed BP1 wins under Standard (seed 17): take-off, a kill, the landing, the stop.
        let r = bot_sortie(
            &pack(8400),
            Arena::Rugged,
            Assist::Standard,
            1,
            17,
            |_, _| {},
        )
        .unwrap();
        assert_eq!(r.outcome, SortieOutcome::Success, "{r:?}");
        assert!(r.kill.is_some() && r.bandit.same_physics());
        assert_eq!(r.bandit.rounds_fired, 0, "the Recruit holds its fire");
    }
}
