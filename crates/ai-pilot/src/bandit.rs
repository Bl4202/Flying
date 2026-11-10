//! The bandit's brain (GDD 19, 66.3 M1, ADR-47): a state machine that decides at 5 Hz and flies
//! every tick, through the manoeuvre controller, with the same inputs a player has.
//!
//! [`BanditBrain::decide`] applies the transition rules in priority order (below);
//! [`BanditBrain::act`] flies the current state. A new state other than RECOVER, the two defences
//! and WRECK waits for the skill's commitment time to run out.
//!
//! 1. WRECK is final (set by `sim-core` when the bandit is destroyed).
//! 2. RECOVER below the skill's terrain clearance, under 150 m above the ground, or in a dive that
//!    would not come level above the clearance; until clear by twice the clearance and climbing.
//! 3. RETURN beyond the arena's radius from home; until inside 0.8 of it.
//! 4. `DEFEND_MISSILE` once a noticed missile has been seen for the skill's defence delay.
//! 5. `DEFEND_GUN` when the player is seen within 2 500 m, its nose within 30° of the bandit, in the
//!    bandit's rear hemisphere (aspect under 60°), for the defence delay.
//! 6. EXTEND (Veteran) below its extend speed, until fast again.
//! 7. GUN when seen within 1 200 m with the gunsight's aim point near the nose.
//! 8. PURSUE when seen, or told within 8 km.
//! 9. INTERCEPT when told.
//! 10. PATROL otherwise.

use flight::controls::PilotControls;
use flight::env::FlightEnv;
use flight::physical::G0;
use simmath::dmath;
use simmath::math::DVec3;

use crate::geometry::{aa_rad, angle_between, closure_mps, lead_point};
use crate::hold::{HEIGHT_GAMMA_MAX, HEIGHT_PER_RAD_M, HoldCourse};
use crate::maneuver::{
    AFTERBURNER, Airframe, Goal, K_TURN, MILITARY, climb_dir, fly_goal, height_gamma, hold_speed,
    horizontal,
};
use crate::percept::{Contact, OwnView, Percept};
use crate::skill::{
    SkillParams, TACTIC_EXTEND, TACTIC_JINK, TACTIC_LAG_PURSUIT, TACTIC_LEAD_PURSUIT,
};

/// RECOVER below this height above the ground whatever the skill, m. // ALGO: ADR-47
pub const RECOVER_AGL_M: f64 = 150.0;
/// RECOVER also when a dive would not come level above the skill's terrain clearance at its load
/// cap, allowing this long for the wings to come level first, s. // ALGO: ADR-47
pub const RECOVER_REACT_S: f64 = 2.5;
/// RECOVER pulls out of a dive steeper than this at the airframe's load limit, rad (10°).
/// // ALGO: ADR-47
pub const RECOVER_DIVE_RAD: f64 = 0.174_532_925_199_432_96;
/// RETURN ends inside this share of the arena radius. // ALGO: ADR-47
pub const RETURN_LEAVE_FRACTION: f64 = 0.8;
/// `DEFEND_GUN`: the player within this range, m. // ALGO: ADR-47
pub const DEFEND_GUN_RANGE_M: f64 = 2_500.0;
/// ... its nose within this of the bandit, rad (30°). // ALGO: ADR-47
pub const DEFEND_GUN_ATA_RAD: f64 = core::f64::consts::FRAC_PI_6;
/// ... in the bandit's rear hemisphere: the bandit's aspect from it under this, rad (60°).
/// // ALGO: ADR-47
pub const DEFEND_GUN_AA_RAD: f64 = core::f64::consts::FRAC_PI_3;
/// GUN: the player within this range, m. // ALGO: ADR-47
pub const GUN_RANGE_M: f64 = 1_200.0;
/// ... and the aim point within this of the nose at that range (closer, proportionally more), rad.
/// // ALGO: ADR-47
pub const GUN_WINDOW_RAD: f64 = 0.020;
/// It fires within this range, m. // ALGO: ADR-47
pub const FIRE_RANGE_M: f64 = 1_000.0;
/// ... with the nose this close to where it aims, rad. // ALGO: ADR-47
pub const FIRE_AIM_RAD: f64 = 0.006;
/// Bursts and pauses, s; the aim error is drawn again this often, s. // ALGO: ADR-47
pub const BURST_S: f64 = 0.6;
/// See [`BURST_S`].
pub const PAUSE_S: f64 = 0.4;
/// See [`BURST_S`].
pub const AIM_REDRAW_S: f64 = 0.5;
/// PURSUE on a GCI contact inside this range, m. // ALGO: ADR-47
pub const PURSUE_GCI_RANGE_M: f64 = 8_000.0;
/// PATROL: the point it aims at on the orbit is this far ahead in bearing, rad (30°). // ALGO
pub const PATROL_LEAD_RAD: f64 = core::f64::consts::FRAC_PI_6;
/// PATROL: calibrated speed, m/s; load cap, g. // DATA: ADR-47 (test scenario)
pub const PATROL_CAS_MPS: f64 = 180.0;
/// See [`PATROL_CAS_MPS`].
pub const PATROL_N_CAP: f64 = 2.0;
/// INTERCEPT: it arrives this far above the contact, m; afterburner beyond this range, m; it
/// pulls this share of its skill's load cap. // ALGO: ADR-47
pub const INTERCEPT_ABOVE_M: f64 = 300.0;
/// See [`INTERCEPT_ABOVE_M`].
pub const INTERCEPT_AB_RANGE_M: f64 = 15_000.0;
/// See [`INTERCEPT_ABOVE_M`].
pub const INTERCEPT_N_SHARE: f64 = 0.6;
/// INTERCEPT and RETURN climb or dive at most this steeply, rad (20°). // ALGO: ADR-47
pub const TRANSIT_GAMMA_MAX: f64 = 0.349_065_850_398_865_9;
/// PURSUE: lead time is the range over (own speed + this), m/s, at most [`LEAD_MAX_S`]. // ALGO
pub const LEAD_SPEED_ADD_MPS: f64 = 300.0;
/// See [`LEAD_SPEED_ADD_MPS`], s.
pub const LEAD_MAX_S: f64 = 2.0;
/// PURSUE: lag pursuit aims this far behind the contact, inside [`LAG_RANGE_M`] when closing
/// faster than [`LAG_CLOSURE_MPS`]. // ALGO: ADR-47
pub const LAG_BEHIND_M: f64 = 300.0;
/// See [`LAG_BEHIND_M`], m.
pub const LAG_RANGE_M: f64 = 600.0;
/// See [`LAG_BEHIND_M`], m/s.
pub const LAG_CLOSURE_MPS: f64 = 50.0;
/// PURSUE and GUN light the afterburner below this calibrated speed, m/s. // ALGO: ADR-47
pub const PURSUE_AB_BELOW_CAS_MPS: f64 = 200.0;
/// `DEFEND_GUN` with JINK: the bank reverses every this plus up to half of it again, s. // ALGO
pub const JINK_PERIOD_S: f64 = 1.0;
/// `DEFEND_MISSILE`: the beam turn's load cap, g; the break starts at this time to go, s, plus the
/// skill's timing error; the throttle is idle for this long after the break starts, s.
/// // ALGO: ADR-47
pub const BEAM_N_CAP: f64 = 3.0;
/// See [`BEAM_N_CAP`].
pub const BREAK_TGO_S: f64 = 3.0;
/// See [`BEAM_N_CAP`].
pub const BREAK_IDLE_S: f64 = 3.0;
/// EXTEND: dive this far below the horizon, rad (10°); RECOVER: climb this far above it, rad (20°).
/// // ALGO: ADR-47
pub const EXTEND_GAMMA_RAD: f64 = -0.174_532_925_199_432_96;
/// See [`EXTEND_GAMMA_RAD`].
pub const RECOVER_GAMMA_RAD: f64 = 0.349_065_850_398_865_9;
/// RETURN: calibrated speed, m/s; load cap, g. // ALGO: ADR-47
pub const RETURN_CAS_MPS: f64 = 220.0;
/// See [`RETURN_CAS_MPS`].
pub const RETURN_N_CAP: f64 = 3.0;
/// WRECK: the roll stick the dead pilot leaves. // ALGO: ADR-47 (a spiral)
pub const WRECK_ROLL: f64 = 0.6;
/// The speed guard (AI4: the bandit flies the player's airframe and must not overspeed it): from
/// this share of `V_mo` up, in every state it flies itself, the throttle closes, the airbrake opens
/// and the path is held no steeper than [`GUARD_GAMMA_RAD`] below the horizon, the floor rising
/// linearly to level at `V_mo`. // ALGO: ADR-47
pub const GUARD_VMO_SHARE: f64 = 0.92;
/// See [`GUARD_VMO_SHARE`] (15° below the horizon).
pub const GUARD_GAMMA_RAD: f64 = -0.261_799_387_799_149_4;
/// Closing speed: inside this range, m, PURSUE and GUN hold the closure to the range beyond the
/// stand-off over this time, s, kept between these, m/s (negative: opening), with this much
/// throttle per m/s of error (never below military under the afterburner speed); closing this much
/// faster than wanted, m/s, the airbrake opens too (not under the afterburner speed: a bandit
/// slowed behind a slow target stalls). Without it a fast bandit overshoots its gun solution, and
/// a slow one flies into the player. // ALGO: ADR-47
pub const CLOSE_RANGE_M: f64 = 3_000.0;
/// See [`CLOSE_RANGE_M`].
pub const CLOSE_STANDOFF_M: f64 = 300.0;
/// See [`CLOSE_RANGE_M`].
pub const CLOSE_TIME_S: f64 = 20.0;
/// See [`CLOSE_RANGE_M`].
pub const CLOSE_MIN_MPS: f64 = -30.0;
/// See [`CLOSE_RANGE_M`].
pub const CLOSE_MAX_MPS: f64 = 150.0;
/// See [`CLOSE_RANGE_M`].
pub const K_CLOSE: f64 = 0.02;
/// See [`CLOSE_RANGE_M`].
pub const CLOSE_BRAKE_ABOVE_MPS: f64 = 30.0;
/// Collision avoidance, in every state it flies itself: when the contact (dead-reckoned from the
/// percept to now) will pass within this, m, and either closes faster than this, m/s, inside this
/// time, s, or is closing at all inside this range, m, the bandit turns away from where it will
/// pass at its skill's load cap. // ALGO: ADR-47
pub const AVOID_MISS_M: f64 = 60.0;
/// See [`AVOID_MISS_M`].
pub const AVOID_CLOSURE_MPS: f64 = 150.0;
/// See [`AVOID_MISS_M`].
pub const AVOID_TIME_S: f64 = 1.0;
/// See [`AVOID_MISS_M`].
pub const AVOID_RANGE_M: f64 = 200.0;

/// The bandit's states (the codes are the snapshot's `bandit_ai`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum BanditState {
    /// Orbiting its patrol point.
    #[default]
    Patrol = 0,
    /// Flying to meet a contact it was told about.
    Intercept = 1,
    /// Chasing a contact.
    Pursue = 2,
    /// Aiming (and firing) the gun.
    Gun = 3,
    /// Breaking away from the player's gun.
    DefendGun = 4,
    /// Defeating a missile.
    DefendMissile = 5,
    /// Unloaded, regaining speed.
    Extend = 6,
    /// Climbing away from the ground.
    Recover = 7,
    /// Going home.
    Return = 8,
    /// Destroyed, falling.
    Wreck = 9,
}

impl BanditState {
    /// The state's code.
    #[must_use]
    pub const fn code(self) -> u8 {
        self as u8
    }

    /// The states that do not wait for the commitment time.
    #[must_use]
    pub const fn urgent(self) -> bool {
        matches!(
            self,
            Self::Recover | Self::DefendGun | Self::DefendMissile | Self::Wreck
        )
    }
}

/// Which brain flies the bandit (the gate runners' knob, ADR-47).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum BrainKind {
    /// The state machine.
    #[default]
    Normal = 0,
    /// Straight and level at its start height, heading and speed; never fires.
    HoldCourse = 1,
    /// As `HoldCourse` until it notices a missile, then rules 4 and 2 only.
    DefendOnly = 2,
}

/// What the brain does this tick.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BrainOut {
    /// Stick, pedals and throttle.
    pub controls: PilotControls,
    /// The gun trigger is held.
    pub fire_gun: bool,
    /// It asks for the Manual assist level (WRECK).
    pub manual: bool,
}

/// The bandit's brain. Plain numbers: `Copy`, no allocation, hashed through [`Self::words`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BanditBrain {
    /// Its state.
    pub state: BanditState,
    /// Which brain it is.
    pub kind: BrainKind,
    /// When the state was entered, s.
    pub since_s: f64,
    /// A new non-urgent state waits until this, s.
    pub commit_until_s: f64,
    /// The speed hold's integral.
    pub speed_i: f64,
    /// The gun aim error now, rad (two axes across the aim line).
    pub aim_offset: [f64; 2],
    /// When the aim error is drawn again, s.
    pub aim_until_s: f64,
    /// The burst lasts until, s.
    pub burst_until_s: f64,
    /// The pause after it lasts until, s.
    pub pause_until_s: f64,
    /// The jink's direction, ±1.
    pub jink_sign: f64,
    /// When the jink reverses, s.
    pub jink_until_s: f64,
    /// The missile defence starts at, s (set when a missile is first seen).
    pub defend_at_s: Option<f64>,
    /// The time to go at which it breaks, s (drawn when a missile is first seen).
    pub break_tgo_s: Option<f64>,
    /// When the break started, s.
    pub break_at_s: Option<f64>,
    /// When the gun threat was first seen, s.
    pub gun_threat_s: Option<f64>,
    /// The patrol orbit's direction: +1 clockwise.
    pub orbit_sign: f64,
    /// The straight-and-level pilot of `HoldCourse` and `DefendOnly`.
    pub hold: HoldCourse,
}

impl BanditBrain {
    /// A fresh brain, patrolling (`orbit_sign` +1 for a clockwise orbit).
    #[must_use]
    pub const fn new(kind: BrainKind, orbit_sign: f64) -> Self {
        Self {
            state: BanditState::Patrol,
            kind,
            since_s: 0.0,
            commit_until_s: 0.0,
            speed_i: 0.0,
            aim_offset: [0.0; 2],
            aim_until_s: 0.0,
            burst_until_s: 0.0,
            pause_until_s: 0.0,
            jink_sign: 1.0,
            jink_until_s: 0.0,
            defend_at_s: None,
            break_tgo_s: None,
            break_at_s: None,
            gun_threat_s: None,
            orbit_sign,
            hold: HoldCourse::new(false),
        }
    }

    /// The bandit is destroyed: WRECK from now on.
    pub fn set_wreck(&mut self, t_s: f64) {
        self.state = BanditState::Wreck;
        self.since_s = t_s;
    }

    /// Its numbers as words, for the state hash.
    #[must_use]
    pub fn words(&self) -> [u64; 20] {
        let opt = |x: Option<f64>| x.map_or(u64::MAX, f64::to_bits);
        let h = self.hold.words();
        [
            u64::from(self.state.code()) | u64::from(self.kind as u8) << 8,
            self.since_s.to_bits(),
            self.commit_until_s.to_bits(),
            self.speed_i.to_bits(),
            self.aim_offset[0].to_bits(),
            self.aim_offset[1].to_bits(),
            self.aim_until_s.to_bits(),
            self.burst_until_s.to_bits(),
            self.pause_until_s.to_bits(),
            self.jink_sign.to_bits(),
            self.jink_until_s.to_bits(),
            opt(self.defend_at_s),
            opt(self.break_tgo_s),
            opt(self.break_at_s),
            opt(self.gun_threat_s),
            self.orbit_sign.to_bits(),
            h[0] ^ h[4].rotate_left(32),
            h[1],
            h[2],
            h[3],
        ]
    }

    fn enter(&mut self, state: BanditState, t: f64, skill: &SkillParams) {
        self.state = state;
        self.since_s = t;
        self.commit_until_s = t + skill.commitment_s;
        if state == BanditState::DefendGun {
            self.jink_sign = 1.0;
            self.jink_until_s = t;
        }
    }

    /// Switches to `next` unless it must wait for the commitment time.
    fn switch(&mut self, next: BanditState, t: f64, skill: &SkillParams) {
        if next != self.state && (next.urgent() || t >= self.commit_until_s) {
            self.enter(next, t, skill);
        }
    }

    /// The decision (5 Hz): the transition rules in priority order.
    pub fn decide(&mut self, p: &Percept, skill: &SkillParams) {
        if self.state == BanditState::Wreck {
            return;
        }
        let t = p.t_s;
        if !p.is_finite() {
            self.enter(
                BanditState::Recover,
                if t.is_finite() { t } else { self.since_s },
                skill,
            );
            return;
        }
        let next = match self.kind {
            BrainKind::HoldCourse => BanditState::Patrol,
            BrainKind::DefendOnly => self
                .recover_rule(p, skill)
                .or_else(|| self.missile_rule(p, skill))
                .unwrap_or(BanditState::Patrol),
            BrainKind::Normal => self.rules(p, skill),
        };
        self.switch(next, t, skill);
    }

    /// Rule 2.
    fn recover_rule(&self, p: &Percept, skill: &SkillParams) -> Option<BanditState> {
        let need = p.clearance_min_m < skill.terrain_min_agl_m
            || p.own.agl_m < RECOVER_AGL_M
            || p.own.agl_m < pull_out_m(&p.own, skill) + skill.terrain_min_agl_m;
        let clear = p.clearance_min_m > 2.0 * skill.terrain_min_agl_m
            && p.own.vel_bf.dot(p.own.up_bf) > 0.0;
        let stay = self.state == BanditState::Recover && !clear;
        (need || stay).then_some(BanditState::Recover)
    }

    /// Rule 4, and the bookkeeping of a missile seen or gone.
    fn missile_rule(&mut self, p: &Percept, skill: &SkillParams) -> Option<BanditState> {
        if p.missile.is_none() {
            self.defend_at_s = None;
            self.break_tgo_s = None;
            self.break_at_s = None;
            return None;
        }
        let at = *self.defend_at_s.get_or_insert(p.t_s + skill.defend_delay_s);
        self.break_tgo_s
            .get_or_insert(BREAK_TGO_S + p.dice.g[0] * skill.break_timing_sigma_s);
        (p.t_s >= at).then_some(BanditState::DefendMissile)
    }

    /// Rule 5.
    fn gun_threat_rule(
        &mut self,
        p: &Percept,
        skill: &SkillParams,
        c: Option<&Contact>,
    ) -> Option<BanditState> {
        let threat = c.is_some_and(|c| {
            c.visual
                && (c.pos_bf - p.own.pos_bf).length() < DEFEND_GUN_RANGE_M
                && p.threat_ata_rad.is_some_and(|a| a < DEFEND_GUN_ATA_RAD)
                && aa_rad(p.nose_bf(), p.own.pos_bf, c.pos_bf) < DEFEND_GUN_AA_RAD
        });
        if !threat {
            self.gun_threat_s = None;
            return None;
        }
        let since = *self.gun_threat_s.get_or_insert(p.t_s);
        (p.t_s >= since + skill.defend_delay_s).then_some(BanditState::DefendGun)
    }

    /// The Normal brain's rules 2 to 10.
    fn rules(&mut self, p: &Percept, skill: &SkillParams) -> BanditState {
        if let Some(s) = self.recover_rule(p, skill) {
            return s;
        }
        let arena = p.arena_radius_m;
        let returning = self.state == BanditState::Return;
        if (returning && p.from_home_m > RETURN_LEAVE_FRACTION * arena)
            || (!returning && p.from_home_m > arena)
        {
            return BanditState::Return;
        }
        let missile = self.missile_rule(p, skill);
        let contact = p.contact.as_ref();
        let gun_threat = self.gun_threat_rule(p, skill, contact);
        if let Some(s) = missile.or(gun_threat) {
            return s;
        }
        if skill.has(TACTIC_EXTEND) {
            let cas = p.own.cas_mps;
            let extending = self.state == BanditState::Extend;
            if (extending && cas < skill.extend_until_cas_mps)
                || (!extending && cas < skill.extend_below_cas_mps)
            {
                return BanditState::Extend;
            }
        }
        let Some(c) = contact else {
            return BanditState::Patrol;
        };
        let range = (c.pos_bf - p.own.pos_bf).length();
        if c.visual && range <= GUN_RANGE_M {
            let window = GUN_WINDOW_RAD * GUN_RANGE_M / range.max(1.0);
            let on = p
                .gun_aim_point_bf
                .is_some_and(|a| angle_between(p.nose_bf(), a - p.own.pos_bf) <= window);
            if on {
                return BanditState::Gun;
            }
        }
        if c.visual || range < PURSUE_GCI_RANGE_M {
            BanditState::Pursue
        } else {
            BanditState::Intercept
        }
    }

    /// One tick: fly the state.
    pub fn act<E: FlightEnv>(
        &mut self,
        p: &Percept,
        skill: &SkillParams,
        af: &Airframe<'_, E>,
    ) -> BrainOut {
        if !p.is_finite() {
            if self.state != BanditState::Wreck {
                let t = if p.t_s.is_finite() {
                    p.t_s
                } else {
                    self.since_s
                };
                self.enter(BanditState::Recover, t, skill);
            }
            return BrainOut {
                controls: PilotControls {
                    throttle: MILITARY,
                    ..PilotControls::default()
                },
                ..BrainOut::default()
            };
        }
        if self.state == BanditState::Wreck {
            return BrainOut {
                controls: PilotControls {
                    roll: WRECK_ROLL,
                    ..PilotControls::default()
                },
                fire_gun: false,
                manual: true,
            };
        }
        let holding = match self.kind {
            BrainKind::Normal => false,
            BrainKind::HoldCourse => true,
            BrainKind::DefendOnly => !matches!(
                self.state,
                BanditState::DefendMissile | BanditState::Recover
            ),
        };
        if holding {
            return BrainOut {
                controls: self.hold.step(af),
                ..BrainOut::default()
            };
        }
        let dt = af.fcs.tick_dt_s;
        let (mut goal, fire) = self.goal(p, skill, dt);
        avoid_collision(&mut goal, p, skill);
        let guarded = speed_guard(&mut goal, &p.own);
        let overtaking = matches!(self.state, BanditState::Pursue | BanditState::Gun)
            && p.own.cas_mps >= PURSUE_AB_BELOW_CAS_MPS
            && p.contact.is_some_and(|c| {
                let (_, closure, want) = closing(&p.own, &c);
                closure > want + CLOSE_BRAKE_ABOVE_MPS
            });
        let pulling_out = self.state == BanditState::Recover && diving(&p.own);
        let mut controls = fly_goal(&goal, af);
        if guarded || overtaking || pulling_out {
            controls.airbrake = 1.0;
        }
        if !(controls.pitch.is_finite() && controls.roll.is_finite()) {
            controls = PilotControls {
                throttle: MILITARY,
                ..PilotControls::default()
            };
        }
        BrainOut {
            controls,
            fire_gun: fire,
            manual: false,
        }
    }

    /// The goal of the state (and the trigger).
    fn goal(&mut self, p: &Percept, skill: &SkillParams, dt: f64) -> (Goal, bool) {
        let o = &p.own;
        let up = o.up_bf;
        let v_hat = o.vel_bf.normalized_or(p.nose_bf());
        let level = horizontal(o.vel_bf, up, p.nose_bf());
        let contact = p.contact;
        let goal = match (self.state, contact) {
            (BanditState::Patrol, _)
            | (
                BanditState::Intercept
                | BanditState::Pursue
                | BanditState::Gun
                | BanditState::DefendGun,
                None,
            ) => self.patrol(p, dt),
            (BanditState::Intercept, Some(c)) => intercept(p, skill, &c),
            (BanditState::Pursue, Some(c)) => pursue(p, skill, &c, v_hat),
            (BanditState::Gun, Some(c)) => return self.gun(p, skill, &c),
            (BanditState::DefendGun, Some(c)) => self.defend_gun(p, skill, &c, v_hat),
            (BanditState::DefendMissile, _) => self.defend_missile(p, skill, v_hat),
            (BanditState::Extend, _) => Goal {
                dir_bf: climb_dir(level, up, EXTEND_GAMMA_RAD),
                n_cap: 1.0,
                throttle: AFTERBURNER,
            },
            // Diving, it pulls out at the airframe's limit with the throttle closed (and the
            // airbrake out, in `act`), whatever its skill; then it climbs in afterburner.
            (BanditState::Recover, _) => Goal {
                dir_bf: climb_dir(level, up, RECOVER_GAMMA_RAD),
                n_cap: if diving(o) { o.n_max } else { skill.n_cap },
                throttle: if diving(o) { 0.0 } else { AFTERBURNER },
            },
            (BanditState::Return, _) => {
                let gamma = height_gamma(
                    p.cap_alt_msl_m,
                    o.alt_msl_m,
                    HEIGHT_PER_RAD_M,
                    HEIGHT_GAMMA_MAX,
                );
                let dir = climb_dir(horizontal(p.home_bf - o.pos_bf, up, level), up, gamma);
                let throttle = hold_speed(
                    RETURN_CAS_MPS,
                    o.cas_mps,
                    &mut self.speed_i,
                    dt,
                    AFTERBURNER,
                );
                Goal {
                    dir_bf: dir,
                    n_cap: RETURN_N_CAP,
                    throttle,
                }
            }
            (BanditState::Wreck, _) => Goal {
                dir_bf: v_hat,
                n_cap: 1.0,
                throttle: MILITARY,
            },
        };
        (goal, false)
    }

    /// PATROL: round the orbit at the patrol height and speed.
    fn patrol(&mut self, p: &Percept, dt: f64) -> Goal {
        let o = &p.own;
        let up = o.up_bf;
        let rel = o.pos_bf - p.cap_centre_bf;
        let bearing = dmath::atan2(rel.x, rel.y) + self.orbit_sign * PATROL_LEAD_RAD;
        let point = p.cap_centre_bf
            + DVec3::new(dmath::sin(bearing), dmath::cos(bearing), 0.0) * p.cap_radius_m;
        let gamma = height_gamma(
            p.cap_alt_msl_m,
            o.alt_msl_m,
            HEIGHT_PER_RAD_M,
            HEIGHT_GAMMA_MAX,
        );
        let dir = climb_dir(horizontal(point - o.pos_bf, up, p.nose_bf()), up, gamma);
        let throttle = hold_speed(
            PATROL_CAS_MPS,
            o.cas_mps,
            &mut self.speed_i,
            dt,
            AFTERBURNER,
        );
        Goal {
            dir_bf: dir,
            n_cap: PATROL_N_CAP,
            throttle,
        }
    }

    /// `DEFEND_GUN`: a break turn across the line to the player, reversed now and then (JINK).
    fn defend_gun(&mut self, p: &Percept, skill: &SkillParams, c: &Contact, v_hat: DVec3) -> Goal {
        let o = &p.own;
        if skill.has(TACTIC_JINK) && p.t_s >= self.jink_until_s {
            if p.t_s > self.since_s {
                self.jink_sign = -self.jink_sign;
            }
            self.jink_until_s = p.t_s + JINK_PERIOD_S * (1.0 + 0.5 * p.dice.u[0]);
        }
        let s_hat = (c.pos_bf - o.pos_bf).normalized_or(-v_hat);
        let lift = o.att.rotate(DVec3::new(0.0, 0.0, -1.0));
        let mut dir = perpendicular_to(v_hat, s_hat, lift);
        if self.jink_sign < 0.0 {
            let along = v_hat * dir.dot(v_hat);
            dir = along - (dir - along);
        }
        Goal {
            dir_bf: dir,
            n_cap: skill.n_cap,
            throttle: AFTERBURNER,
        }
    }

    /// GUN: the nose on the aim point, off by the skill's aim error, and the trigger.
    fn gun(&mut self, p: &Percept, skill: &SkillParams, c: &Contact) -> (Goal, bool) {
        let o = &p.own;
        let t = p.t_s;
        if t >= self.aim_until_s {
            self.aim_offset = [
                skill.gun_aim_sigma_rad * p.dice.g[1],
                skill.gun_aim_sigma_rad * p.dice.g[2],
            ];
            self.aim_until_s = t + AIM_REDRAW_S;
        }
        let aim_point = p.gun_aim_point_bf.unwrap_or(c.pos_bf);
        let base = (aim_point - o.pos_bf).normalized_or(p.nose_bf());
        let (e1, e2) = across_pair(base);
        let aim = (base + e1 * self.aim_offset[0] + e2 * self.aim_offset[1]).normalized_or(base);
        let nose = p.nose_bf();
        // Steer the velocity so the nose (the gun) comes onto the aim: the angle of attack stays.
        // The line of sight's own rate is fed forward, so a crossing target is tracked without
        // the lag of the controller's turn gain.
        let v_hat = o.vel_bf.normalized_or(nose);
        let r = c.pos_bf + c.vel_bf * c.age_s - o.pos_bf;
        let los_rate = r.cross(c.vel_bf - o.vel_bf) * (1.0 / r.length_squared().max(1.0));
        let dir = v_hat + (aim - nose) + los_rate.cross(aim) * (1.0 / K_TURN);
        let range = (c.pos_bf - o.pos_bf).length();
        let want =
            skill.fires_gun && range <= FIRE_RANGE_M && angle_between(nose, aim) <= FIRE_AIM_RAD;
        let fire = self.trigger(want, t);
        (
            Goal {
                dir_bf: dir,
                n_cap: skill.n_cap,
                throttle: closing_throttle(o, c),
            },
            fire,
        )
    }

    /// Bursts of [`BURST_S`] with pauses of [`PAUSE_S`] while it wants to fire.
    fn trigger(&mut self, want: bool, t: f64) -> bool {
        if !want {
            return false;
        }
        if t < self.burst_until_s {
            return true;
        }
        if t < self.pause_until_s {
            return false;
        }
        self.burst_until_s = t + BURST_S;
        self.pause_until_s = t + BURST_S + PAUSE_S;
        true
    }

    /// `DEFEND_MISSILE`: the missile to the beam, then at the break a hard pull across its line of
    /// sight with the throttle at idle, back to military after [`BREAK_IDLE_S`].
    fn defend_missile(&mut self, p: &Percept, skill: &SkillParams, v_hat: DVec3) -> Goal {
        let o = &p.own;
        let lift = o.att.rotate(DVec3::new(0.0, 0.0, -1.0));
        let Some(m) = p.missile else {
            return Goal {
                dir_bf: v_hat,
                n_cap: BEAM_N_CAP,
                throttle: MILITARY,
            };
        };
        let m_hat = (m.pos_bf - o.pos_bf).normalized_or(-v_hat);
        if self.break_at_s.is_none() {
            let range = (m.pos_bf - o.pos_bf).length();
            let closure = closure_mps(o.pos_bf, o.vel_bf, m.pos_bf, m.vel_bf);
            let tgo = if closure > 0.0 {
                range / closure
            } else {
                f64::INFINITY
            };
            if tgo < self.break_tgo_s.unwrap_or(BREAK_TGO_S) {
                self.break_at_s = Some(p.t_s);
            }
        }
        match self.break_at_s {
            Some(at) => {
                // Across both the line of sight and the path, on the side the lift points.
                let axis = m_hat.cross(v_hat).normalized_or(lift);
                let dir = if axis.dot(lift) >= 0.0 { axis } else { -axis };
                let throttle = if p.t_s - at < BREAK_IDLE_S {
                    0.0
                } else {
                    MILITARY
                };
                Goal {
                    dir_bf: dir,
                    n_cap: skill.n_cap,
                    throttle,
                }
            }
            None => Goal {
                dir_bf: perpendicular_to(v_hat, m_hat, lift),
                n_cap: BEAM_N_CAP,
                throttle: MILITARY,
            },
        }
    }
}

/// Descending more steeply than [`RECOVER_DIVE_RAD`].
fn diving(o: &OwnView) -> bool {
    o.vel_bf.dot(o.up_bf) < -dmath::sin(RECOVER_DIVE_RAD) * o.vel_bf.length()
}

/// The height a dive needs to come level ([`RECOVER_REACT_S`]): the arc at the skill's load cap,
/// plus the path flown while the wings come level, m (0 climbing).
fn pull_out_m(o: &OwnView, skill: &SkillParams) -> f64 {
    let v = o.vel_bf.length();
    let sin_down = -o.vel_bf.dot(o.up_bf) / v.max(1.0);
    if !(sin_down > 0.0 && v.is_finite()) {
        return 0.0;
    }
    let radius = v * v / ((skill.n_cap - 1.0).max(1.0) * G0);
    let cos_dive = dmath::sqrt((1.0 - sin_down * sin_down).max(0.0));
    radius * (1.0 - cos_dive) + v * sin_down * RECOVER_REACT_S
}

/// Collision avoidance ([`AVOID_CLOSURE_MPS`]): true when it turns the goal away.
fn avoid_collision(goal: &mut Goal, p: &Percept, skill: &SkillParams) -> bool {
    let Some(c) = p.contact else {
        return false;
    };
    let o = &p.own;
    let r = c.pos_bf + c.vel_bf * c.age_s - o.pos_bf;
    let v = c.vel_bf - o.vel_bf;
    let vv = v.length_squared();
    if vv.is_nan() || vv <= 1.0 {
        return false;
    }
    let t = -r.dot(v) / vv;
    let miss = r + v * t;
    let fast = vv > AVOID_CLOSURE_MPS * AVOID_CLOSURE_MPS && t < AVOID_TIME_S;
    let near = r.length() < AVOID_RANGE_M;
    if !(t > 0.0 && miss.length() < AVOID_MISS_M && (fast || near)) {
        return false;
    }
    let v_hat = o.vel_bf.normalized_or(p.nose_bf());
    let lift = o.att.rotate(DVec3::new(0.0, 0.0, -1.0));
    let away = (-miss).normalized_or(lift);
    let side = (away - v_hat * away.dot(v_hat)).normalized_or(lift);
    goal.dir_bf = v_hat + side;
    goal.n_cap = skill.n_cap;
    true
}

/// The speed guard ([`GUARD_VMO_SHARE`]): true when it acts (the caller opens the airbrake).
fn speed_guard(goal: &mut Goal, o: &OwnView) -> bool {
    let share = o.cas_mps / o.vmo_mps;
    if share.is_nan() || share < GUARD_VMO_SHARE {
        return false;
    }
    goal.throttle = 0.0;
    let k = ((share - GUARD_VMO_SHARE) / (1.0 - GUARD_VMO_SHARE)).clamp(0.0, 1.0);
    let floor = GUARD_GAMMA_RAD * (1.0 - k);
    let up = o.up_bf;
    let path = horizontal(o.vel_bf, up, DVec3::Y);
    let d = goal.dir_bf.normalized_or(o.vel_bf.normalized_or(path));
    if d.dot(up) < dmath::sin(floor) {
        goal.dir_bf = climb_dir(horizontal(d, up, path), up, floor);
    }
    true
}

/// `INTERCEPT`: to the lead-collision point, a little above the contact.
fn intercept(p: &Percept, skill: &SkillParams, c: &Contact) -> Goal {
    let o = &p.own;
    let up = o.up_bf;
    let range = (c.pos_bf - o.pos_bf).length();
    let lead = lead_point(o.pos_bf, o.tas_mps, c.pos_bf, c.vel_bf).map_or(c.pos_bf, |(x, _)| x);
    let aim = lead + up * INTERCEPT_ABOVE_M;
    let throttle = if range > INTERCEPT_AB_RANGE_M {
        AFTERBURNER
    } else {
        MILITARY
    };
    Goal {
        dir_bf: transit_dir(aim - o.pos_bf, up),
        n_cap: INTERCEPT_N_SHARE * skill.n_cap,
        throttle,
    }
}

/// `PURSUE`: pure, lead or lag pursuit by the repertoire.
fn pursue(p: &Percept, skill: &SkillParams, c: &Contact, v_hat: DVec3) -> Goal {
    let o = &p.own;
    let range = (c.pos_bf - o.pos_bf).length();
    let closure = closure_mps(o.pos_bf, o.vel_bf, c.pos_bf, c.vel_bf);
    let aim = if skill.has(TACTIC_LAG_PURSUIT) && range < LAG_RANGE_M && closure > LAG_CLOSURE_MPS {
        c.pos_bf - c.vel_bf.normalized_or(v_hat) * LAG_BEHIND_M
    } else if skill.has(TACTIC_LEAD_PURSUIT) {
        let t = (range / (o.tas_mps + LEAD_SPEED_ADD_MPS)).clamp(0.0, LEAD_MAX_S);
        c.pos_bf + c.vel_bf * t
    } else {
        c.pos_bf
    };
    Goal {
        dir_bf: aim - o.pos_bf,
        n_cap: skill.n_cap,
        throttle: closing_throttle(o, c),
    }
}

/// The range, the closure and the closure wanted ([`CLOSE_RANGE_M`]) on a contact, dead-reckoned
/// to now; m, m/s, m/s.
fn closing(o: &OwnView, c: &Contact) -> (f64, f64, f64) {
    let pos = c.pos_bf + c.vel_bf * c.age_s;
    let range = (pos - o.pos_bf).length();
    let closure = closure_mps(o.pos_bf, o.vel_bf, pos, c.vel_bf);
    let want = if range < CLOSE_RANGE_M {
        ((range - CLOSE_STANDOFF_M) / CLOSE_TIME_S).clamp(CLOSE_MIN_MPS, CLOSE_MAX_MPS)
    } else {
        f64::INFINITY
    };
    (range, closure, want)
}

/// The throttle of PURSUE and GUN: [`pursuit_throttle`] far out, the closure held inside
/// [`CLOSE_RANGE_M`].
fn closing_throttle(o: &OwnView, c: &Contact) -> f64 {
    let (range, closure, want) = closing(o, c);
    if range >= CLOSE_RANGE_M || !closure.is_finite() {
        return pursuit_throttle(o.cas_mps);
    }
    let lever = (MILITARY + K_CLOSE * (want - closure)).clamp(0.0, AFTERBURNER);
    if o.cas_mps < PURSUE_AB_BELOW_CAS_MPS {
        lever.max(MILITARY)
    } else {
        lever
    }
}

/// The throttle of PURSUE and GUN: military, the afterburner when slow.
fn pursuit_throttle(cas: f64) -> f64 {
    if cas < PURSUE_AB_BELOW_CAS_MPS {
        AFTERBURNER
    } else {
        MILITARY
    }
}

/// A direction towards `to` with its climb or dive held to [`TRANSIT_GAMMA_MAX`].
fn transit_dir(to: DVec3, up: DVec3) -> DVec3 {
    let d = to.normalized_or(up);
    let gamma =
        dmath::asin(d.dot(up).clamp(-1.0, 1.0)).clamp(-TRANSIT_GAMMA_MAX, TRANSIT_GAMMA_MAX);
    climb_dir(horizontal(d, up, d), up, gamma)
}

/// The direction perpendicular to the line `s_hat`, as near `v_hat` as can be
/// (`v̂ − (v̂·ŝ) ŝ`); along the lift's side when `v̂` lies on the line.
fn perpendicular_to(v_hat: DVec3, s_hat: DVec3, lift: DVec3) -> DVec3 {
    let p = v_hat - s_hat * v_hat.dot(s_hat);
    if p.length_squared() > 1e-6 {
        p.normalized_or(lift)
    } else {
        (lift - s_hat * lift.dot(s_hat)).normalized_or(lift)
    }
}

/// Two unit vectors across a unit direction.
fn across_pair(d: DVec3) -> (DVec3, DVec3) {
    let c = d.cross(DVec3::Z);
    let e1 = if c.length_squared() > 1e-6 {
        c.normalized_or(DVec3::X)
    } else {
        d.cross(DVec3::X).normalized_or(DVec3::Y)
    };
    (e1, d.cross(e1))
}

#[cfg(test)]
mod tests;
