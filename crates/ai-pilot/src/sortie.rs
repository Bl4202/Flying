//! The bot playtester (ADR-45 BP1, ADR-47): a whole sortie against the bandit through the same
//! inputs a player has, for the AC-4.5 precursor gate.
//!
//! ```text
//! TakeOff -> Climb -> Attack -> Return -> Land -> Done
//!  the fcs     to the    missile at   to the downwind   the fcs tester
//!  tester      bandit    SHOOT, gun   leg, then hands   lands and stops
//!  (to the              inside 900 m  it back
//!  crosswind)
//! ```
//!
//! Like the virtual tester it wraps, it is an instrument: it reads the aircraft's exact state, the
//! bandit's exact position and the combat cues the HUD shows (lock, `IN RNG`, `SHOOT`, the
//! pipper). A new spawn (after being shot down) starts it again: on the runway with the take-off,
//! in the air with the attack or the climb.

use fcs::tester::geom::{Runway, RunwayPoint, wrap};
use fcs::tester::{CIRCUIT, Levers, Phase, Presses, Tester};
use flight::controls::PilotControls;
use flight::env::FlightEnv;
use simmath::dmath;
use simmath::math::{DQuat, DVec3};

use crate::geometry::{aa_rad, angle_between, lead_point};
use crate::hold::HEIGHT_PER_RAD_M;
use crate::maneuver::{
    AFTERBURNER, Airframe, Goal, MILITARY, climb_dir, fly_goal, heading_dir, height_gamma,
    hold_speed, horizontal,
};

/// The bandit counts as near within this, m. // ALGO: ADR-47
pub const ATTACK_RANGE_M: f64 = 12_000.0;
/// The climb goes to this height, m above MSL, at most this steeply, rad (10°), at this calibrated
/// speed, m/s, pulling at most this, g. // ALGO: ADR-47
pub const CLIMB_ALT_MSL_M: f64 = 4_000.0;
/// See [`CLIMB_ALT_MSL_M`].
pub const CLIMB_GAMMA_MAX_RAD: f64 = 0.174_532_925_199_432_96;
/// See [`CLIMB_ALT_MSL_M`].
pub const CLIMB_CAS_MPS: f64 = 220.0;
/// See [`CLIMB_ALT_MSL_M`].
pub const CLIMB_N_CAP: f64 = 4.0;
/// A missile is fired at most this often, s. // ALGO: ADR-47
pub const MISSILE_COOLDOWN_S: f64 = 8.0;
/// The seeker's pursuit: with missiles left, from this range to that, and the bandit's aspect
/// under this, the nose goes on the bandit. // ALGO: ADR-47
pub const SEEKER_MIN_RANGE_M: f64 = 600.0;
/// See [`SEEKER_MIN_RANGE_M`].
pub const SEEKER_MAX_RANGE_M: f64 = 5_000.0;
/// See [`SEEKER_MIN_RANGE_M`] (35°).
pub const SEEKER_MAX_ASPECT_RAD: f64 = 0.610_865_238_198_015_4;
/// The attack pulls at most this, g; on the pipper at most that. // ALGO: ADR-47
pub const ATTACK_N_CAP: f64 = 7.0;
/// See [`ATTACK_N_CAP`].
pub const PIPPER_N_CAP: f64 = 7.5;
/// Inside this range it flies the pipper onto the bandit, m. // ALGO: ADR-47
pub const PIPPER_RANGE_M: f64 = 1_500.0;
/// It fires the gun inside this range with the pipper this close (as a distance at the target, m).
/// // ALGO: ADR-47
pub const GUN_RANGE_M: f64 = 900.0;
/// See [`GUN_RANGE_M`].
pub const GUN_AIM_M: f64 = 5.0;
/// Gun bursts and the pauses between them, s. // ALGO: ADR-47
pub const GUN_BURST_S: f64 = 0.5;
/// See [`GUN_BURST_S`].
pub const GUN_PAUSE_S: f64 = 0.25;
/// With no kill after this long since the start, it goes home, s. // ALGO: ADR-47
pub const ATTACK_TIMEOUT_S: f64 = 480.0;
/// The return: the point it flies to first is this far up the extended downwind line from the
/// crosswind corner (BF (−3 800, 1 200) at the test airfield), m; at this height above the runway,
/// m, and this speed, m/s; it is reached within this, m. // ALGO: ADR-47
pub const RETURN_LEAD_IN_M: f64 = 3_000.0;
/// See [`RETURN_LEAD_IN_M`].
pub const RETURN_HEIGHT_M: f64 = 600.0;
/// See [`RETURN_LEAD_IN_M`].
pub const RETURN_CAS_MPS: f64 = 160.0;
/// See [`RETURN_LEAD_IN_M`].
pub const RETURN_CAPTURE_M: f64 = 1_500.0;
/// The downwind line: the tester's height and speed; the joining tolerances (heading, rad (30°),
/// height, m, speed, m/s). // ALGO: ADR-47
pub const JOIN_HEADING_TOL_RAD: f64 = core::f64::consts::FRAC_PI_6;
/// See [`JOIN_HEADING_TOL_RAD`].
pub const JOIN_HEIGHT_TOL_M: f64 = 150.0;
/// See [`JOIN_HEADING_TOL_RAD`].
pub const JOIN_CAS_TOL_MPS: f64 = 20.0;
/// The downwind line's intercept: cross-track error for the full intercept angle, m; the widest
/// angle, rad (45°). // ALGO: as the tester's circuit
pub const LINE_INTERCEPT_M: f64 = 1_000.0;
/// See [`LINE_INTERCEPT_M`].
pub const LINE_INTERCEPT_MAX_RAD: f64 = core::f64::consts::FRAC_PI_4;
/// The return pulls at most this, g. // ALGO: ADR-47
pub const RETURN_N_CAP: f64 = 3.0;
/// On the way home the airbrake opens while this much faster than the speed wanted, m/s (at idle
/// the clean Kestrel sheds speed too slowly to arrive at the downwind speed). // ALGO: ADR-47
pub const RETURN_AIRBRAKE_ABOVE_MPS: f64 = 8.0;

/// Where the bot is in its sortie (the codes are the sortie report's).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum SortiePhase {
    /// The virtual tester's take-off, to the crosswind turn.
    #[default]
    TakeOff = 0,
    /// Climbing towards the bandit.
    Climb = 1,
    /// Fighting.
    Attack = 2,
    /// Flying back to the downwind leg.
    Return = 3,
    /// The virtual tester's circuit from the downwind leg: landing.
    Land = 4,
    /// Stopped on the runway.
    Done = 5,
}

/// The bot's own side: its levers, its spawn and the combat cues.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct OwnInputs {
    /// The gear and flap levers and the parking brake.
    pub levers: Levers,
    /// The aircraft's spawn sequence (it changes on every respawn).
    pub spawn_seq: u8,
    /// The seeker is locked.
    pub locked: bool,
    /// `IN RNG`.
    pub in_range: bool,
    /// `SHOOT`: locked, in range, no launch inhibit.
    pub shoot_missile: bool,
    /// Rounds left.
    pub rounds: u16,
    /// Missiles left.
    pub missiles: u8,
    /// The gunsight's aim point on the bandit, BF, when there is one.
    pub pipper_bf: Option<DVec3>,
}

/// The bandit as the bot sees it (exactly).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BanditView {
    /// Reference point, BF, m.
    pub pos_bf: DVec3,
    /// Velocity, BF, m/s.
    pub vel_bf: DVec3,
    /// Attitude, body to BF.
    pub att: DQuat,
    /// It is alive (not a wreck, not gone).
    pub alive: bool,
}

/// What the bot does this tick.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SortieOut {
    /// Stick, pedals, throttle, brakes and airbrake.
    pub controls: PilotControls,
    /// The lever buttons it presses.
    pub presses: Presses,
    /// The gun trigger is held.
    pub fire_gun: bool,
    /// The missile button is pressed this tick.
    pub fire_missile: bool,
}

/// The bot playtester. Plain numbers: `Copy`, no allocation, hashed through [`Self::words`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SortieTester {
    phase: SortiePhase,
    inner: Tester,
    since_s: f64,
    missile_cooldown_until_s: f64,
    burst_until_s: f64,
    pause_until_s: f64,
    last_spawn_seq: u8,
    started: bool,
    speed_i: f64,
    /// The return's leg: 0 to the lead-in point, 1 down the downwind line.
    return_leg: u8,
}

impl SortieTester {
    /// A bot for a runway; it decides how to start at its first tick.
    #[must_use]
    pub fn new(runway: Runway) -> Self {
        Self {
            phase: SortiePhase::TakeOff,
            inner: Tester::new(runway),
            since_s: 0.0,
            missile_cooldown_until_s: 0.0,
            burst_until_s: 0.0,
            pause_until_s: 0.0,
            last_spawn_seq: 0,
            started: false,
            speed_i: 0.0,
            return_leg: 0,
        }
    }

    /// Where it is.
    #[must_use]
    pub fn phase(&self) -> SortiePhase {
        self.phase
    }

    /// The virtual tester it takes off and lands with.
    #[must_use]
    pub fn inner(&self) -> &Tester {
        &self.inner
    }

    /// Its numbers as words, for the state hash.
    #[must_use]
    pub fn words(&self) -> [u64; 13] {
        let w = self.inner.words();
        [
            u64::from(self.phase as u8)
                | u64::from(self.return_leg) << 8
                | u64::from(self.started) << 16
                | u64::from(self.last_spawn_seq) << 24,
            self.since_s.to_bits(),
            self.missile_cooldown_until_s.to_bits(),
            self.burst_until_s.to_bits(),
            self.pause_until_s.to_bits(),
            self.speed_i.to_bits(),
            w[0],
            w[1],
            w[2],
            w[3],
            w[4],
            w[5],
            self.inner.runway().heading_rad.to_bits(),
        ]
    }

    fn enter(&mut self, phase: SortiePhase, t: f64) {
        self.phase = phase;
        self.since_s = t;
        self.speed_i = 0.0;
        self.return_leg = 0;
    }

    /// One tick of the sortie.
    pub fn step<E: FlightEnv>(
        &mut self,
        own: &OwnInputs,
        bandit: Option<&BanditView>,
        af: &Airframe<'_, E>,
    ) -> SortieOut {
        let t = af.t_s;
        let alive = bandit.filter(|b| b.alive);
        let near = |b: &&BanditView| (b.pos_bf - af.ac.body.pos_bf).length() <= ATTACK_RANGE_M;
        if !self.started || own.spawn_seq != self.last_spawn_seq {
            self.started = true;
            self.last_spawn_seq = own.spawn_seq;
            self.inner = Tester::new(self.inner.runway());
            self.missile_cooldown_until_s = t;
            self.burst_until_s = t;
            self.pause_until_s = t;
            let phase = if af.ac.ground.wow {
                SortiePhase::TakeOff
            } else if alive.as_ref().is_some_and(near) {
                SortiePhase::Attack
            } else {
                SortiePhase::Climb
            };
            self.enter(phase, t);
        }
        match self.phase {
            SortiePhase::TakeOff => {
                let out = self
                    .inner
                    .step(af.model, af.fcs, af.ac, af.fs, own.levers, af.env, t);
                if self.inner.phase() < Phase::Crosswind {
                    return SortieOut {
                        controls: out.controls,
                        presses: out.presses,
                        ..SortieOut::default()
                    };
                }
                self.enter(SortiePhase::Climb, t);
                self.climb(alive, af)
            }
            SortiePhase::Climb => {
                if alive.as_ref().is_some_and(near) {
                    self.enter(SortiePhase::Attack, t);
                    return self.attack(own, alive, af);
                }
                self.climb(alive, af)
            }
            SortiePhase::Attack => self.attack(own, alive, af),
            SortiePhase::Return => self.go_home(own, af),
            SortiePhase::Land | SortiePhase::Done => {
                let out = self
                    .inner
                    .step(af.model, af.fcs, af.ac, af.fs, own.levers, af.env, t);
                if self.inner.phase() == Phase::Stopped {
                    self.phase = SortiePhase::Done;
                }
                let mut controls = out.controls;
                if self.phase == SortiePhase::Done {
                    controls.brake_l = 1.0;
                    controls.brake_r = 1.0;
                    controls.throttle = 0.0;
                }
                SortieOut {
                    controls,
                    presses: out.presses,
                    ..SortieOut::default()
                }
            }
        }
    }

    /// Climb towards the bandit (or north) to the attack height.
    fn climb<E: FlightEnv>(
        &mut self,
        bandit: Option<&BanditView>,
        af: &Airframe<'_, E>,
    ) -> SortieOut {
        let up = af.up();
        let pos = af.ac.body.pos_bf;
        let to = bandit.map_or(DVec3::Y, |b| b.pos_bf - pos);
        let height = af.env.height_msl_m(pos);
        let gamma = height_gamma(
            CLIMB_ALT_MSL_M,
            height,
            HEIGHT_PER_RAD_M,
            CLIMB_GAMMA_MAX_RAD,
        );
        let dir = climb_dir(horizontal(to, up, DVec3::Y), up, gamma);
        let cas = af.sensed().cas;
        let throttle = hold_speed(
            CLIMB_CAS_MPS,
            cas,
            &mut self.speed_i,
            af.fcs.tick_dt_s,
            MILITARY,
        );
        SortieOut {
            controls: fly_goal(
                &Goal {
                    dir_bf: dir,
                    n_cap: CLIMB_N_CAP,
                    throttle,
                },
                af,
            ),
            ..SortieOut::default()
        }
    }

    /// The fight.
    fn attack<E: FlightEnv>(
        &mut self,
        own: &OwnInputs,
        bandit: Option<&BanditView>,
        af: &Airframe<'_, E>,
    ) -> SortieOut {
        let t = af.t_s;
        let Some(b) = bandit else {
            self.enter(SortiePhase::Return, t);
            return self.go_home(own, af);
        };
        if t >= ATTACK_TIMEOUT_S {
            self.enter(SortiePhase::Return, t);
            return self.go_home(own, af);
        }
        let body = &af.ac.body;
        let pos = body.pos_bf;
        let r = b.pos_bf - pos;
        let range = r.length();
        let s_hat = r.normalized_or(DVec3::Y);
        let nose = body.att.rotate(DVec3::X);
        let v_hat = body.vel_bf.normalized_or(nose);
        let aspect = aa_rad(b.att.rotate(DVec3::X), b.pos_bf, pos);
        let fire_missile =
            own.shoot_missile && own.missiles > 0 && t >= self.missile_cooldown_until_s;
        if fire_missile {
            self.missile_cooldown_until_s = t + MISSILE_COOLDOWN_S;
        }
        let pip = own.pipper_bf.map(|p| (p - pos).normalized_or(nose));
        let (dir, n_cap) = if own.missiles > 0
            && (SEEKER_MIN_RANGE_M..=SEEKER_MAX_RANGE_M).contains(&range)
            && aspect <= SEEKER_MAX_ASPECT_RAD
        {
            // The nose (the caged seeker) on the bandit.
            (v_hat + (s_hat - nose), ATTACK_N_CAP)
        } else if range > PIPPER_RANGE_M || pip.is_none() {
            let speed = body.vel_bf.length();
            let lead = lead_point(pos, speed, b.pos_bf, b.vel_bf).map_or(b.pos_bf, |(p, _)| p);
            (lead - pos, ATTACK_N_CAP)
        } else {
            let p_hat = pip.unwrap_or(s_hat);
            (v_hat + (s_hat - p_hat), PIPPER_N_CAP)
        };
        let aim_err = pip.map_or(f64::INFINITY, |p| angle_between(s_hat, p));
        let want = own.rounds > 0 && range <= GUN_RANGE_M && aim_err <= GUN_AIM_M / range.max(1.0);
        let fire_gun = self.trigger(want, t);
        let cas = af.sensed().cas;
        let throttle = if cas < CLIMB_CAS_MPS {
            AFTERBURNER
        } else {
            MILITARY
        };
        SortieOut {
            controls: fly_goal(
                &Goal {
                    dir_bf: dir,
                    n_cap,
                    throttle,
                },
                af,
            ),
            presses: Presses::default(),
            fire_gun,
            fire_missile,
        }
    }

    /// Gun bursts while it wants to fire.
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
        self.burst_until_s = t + GUN_BURST_S;
        self.pause_until_s = t + GUN_BURST_S + GUN_PAUSE_S;
        true
    }

    /// Back to the downwind leg: to the lead-in point, then down the line, then the tester.
    fn go_home<E: FlightEnv>(&mut self, own: &OwnInputs, af: &Airframe<'_, E>) -> SortieOut {
        let t = af.t_s;
        let rw = self.inner.runway();
        let up = af.up();
        let pos = af.ac.body.pos_bf;
        let here = rw.locate(pos);
        let s = af.sensed();
        let dt = af.fcs.tick_dt_s;
        let line_across = -CIRCUIT.width_m;
        let lead_in = RunwayPoint {
            along: CIRCUIT.corner_along_m + RETURN_LEAD_IN_M,
            across: line_across,
        };
        let (da, dc) = (lead_in.along - here.along, lead_in.across - here.across);
        if self.return_leg == 0 && dmath::hypot(da, dc) < RETURN_CAPTURE_M {
            self.return_leg = 1;
            self.speed_i = 0.0;
        }
        let mut cas_wanted = RETURN_CAS_MPS;
        let goal = if self.return_leg == 0 {
            let rw_height = af.env.height_msl_m(rw.threshold_bf);
            let height = af.env.height_msl_m(pos);
            let gamma = height_gamma(
                rw_height + RETURN_HEIGHT_M,
                height,
                HEIGHT_PER_RAD_M,
                CLIMB_GAMMA_MAX_RAD,
            );
            let dir = climb_dir(heading_dir(rw.heading_of(da, dc), up), up, gamma);
            let throttle = hold_speed(RETURN_CAS_MPS, s.cas, &mut self.speed_i, dt, MILITARY);
            Goal {
                dir_bf: dir,
                n_cap: RETURN_N_CAP,
                throttle,
            }
        } else {
            let (va, vc) = rw.velocity(af.ac.body.vel_bf);
            let track = dmath::atan2(vc, va);
            let agl = s.tel.agl_m;
            let downwind_cas = fcs::tester::DOWNWIND_CAS_MPS;
            let downwind_agl = fcs::tester::CIRCUIT_AGL_M;
            cas_wanted = downwind_cas;
            let joined = here.along <= CIRCUIT.corner_along_m
                && wrap(track - core::f64::consts::PI).abs() < JOIN_HEADING_TOL_RAD
                && (agl - downwind_agl).abs() < JOIN_HEIGHT_TOL_M
                && (s.cas - downwind_cas).abs() < JOIN_CAS_TOL_MPS;
            if joined {
                self.inner.join_downwind(t);
                self.enter(SortiePhase::Land, t);
                let out = self
                    .inner
                    .step(af.model, af.fcs, af.ac, af.fs, own.levers, af.env, t);
                return SortieOut {
                    controls: out.controls,
                    presses: out.presses,
                    ..SortieOut::default()
                };
            }
            // Down the line (towards the runway's back), intercepting it; the error is to the
            // right of the line as flown (westwards for a northbound runway).
            let err = line_across - here.across;
            let intercept = dmath::atan(err / LINE_INTERCEPT_M)
                .clamp(-LINE_INTERCEPT_MAX_RAD, LINE_INTERCEPT_MAX_RAD);
            let heading = rw.heading_rad + core::f64::consts::PI - intercept;
            let gamma = height_gamma(downwind_agl, agl, HEIGHT_PER_RAD_M, CLIMB_GAMMA_MAX_RAD);
            let dir = climb_dir(heading_dir(heading, up), up, gamma);
            let throttle = hold_speed(downwind_cas, s.cas, &mut self.speed_i, dt, MILITARY);
            Goal {
                dir_bf: dir,
                n_cap: RETURN_N_CAP,
                throttle,
            }
        };
        let mut controls = fly_goal(&goal, af);
        if s.cas > cas_wanted + RETURN_AIRBRAKE_ABOVE_MPS {
            controls.airbrake = 1.0;
        }
        SortieOut {
            controls,
            ..SortieOut::default()
        }
    }
}

#[cfg(test)]
mod tests;
