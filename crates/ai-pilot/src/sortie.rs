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
/// The attack's energy: the load cap is held to this share of what the clean wing gives at the
/// speed (`(CAS / V_s)²`, at least 1 g), and below this calibrated speed, m/s, the path is held no
/// higher than level in afterburner until it is faster again (Reduced and Manual do not stop a
/// stall; a slow fight at the top of a zoom ends in one). // ALGO: ADR-47
pub const ATTACK_N_STALL_SHARE: f64 = 0.8;
/// See [`ATTACK_N_STALL_SHARE`].
pub const ATTACK_MIN_CAS_MPS: f64 = 150.0;
/// The pilot's g: from this stress the attack's load cap eases, reaching this at that stress
/// (below the 6 g tolerance, where the stress recovers; ADR-38: 0.7 is the blackout, 1 G-LOC).
/// // ALGO: ADR-47
pub const G_EASE_STRESS: f64 = 0.3;
/// See [`G_EASE_STRESS`].
pub const G_EASED_STRESS: f64 = 0.6;
/// See [`G_EASE_STRESS`].
pub const G_EASED_N: f64 = 5.0;
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
/// crosswind corner (BF (−3 800, 4 200) at the test airfield), m, at the circuit's height over
/// the corner's ground but at least this over the ground below, m, and this speed, m/s; it is
/// reached within this, m, and this close to its height ([`JOIN_HEIGHT_TOL_M`]). // ALGO: ADR-47
pub const RETURN_LEAD_IN_M: f64 = 6_000.0;
/// See [`RETURN_LEAD_IN_M`].
pub const RETURN_HEIGHT_M: f64 = 300.0;
/// See [`RETURN_LEAD_IN_M`].
pub const RETURN_CAS_MPS: f64 = 140.0;
/// See [`RETURN_LEAD_IN_M`].
pub const RETURN_CAPTURE_M: f64 = 1_500.0;
/// The way to the lead-in point descends at most this steeply, rad (15°); arriving within this of
/// it, m, away from its height, the bot rounds it at this radius, m, until the height is right.
/// // ALGO: ADR-47
pub const RETURN_DESCENT_MAX_RAD: f64 = 0.261_799_387_799_149_4;
/// See [`RETURN_DESCENT_MAX_RAD`].
pub const RETURN_ORBIT_M: f64 = 3_000.0;
/// See [`RETURN_DESCENT_MAX_RAD`].
pub const RETURN_ORBIT_RADIUS_M: f64 = 2_000.0;
/// The return hands over to the tester's downwind leg when its track is within this of the leg's,
/// rad (30°); the lead-in point counts as reached this close to its height, m. // ALGO: ADR-47
pub const JOIN_HEADING_TOL_RAD: f64 = core::f64::consts::FRAC_PI_6;
/// See [`JOIN_HEADING_TOL_RAD`].
pub const JOIN_HEIGHT_TOL_M: f64 = 150.0;
/// The downwind line's intercept: cross-track error for the full intercept angle, m; the widest
/// angle, rad (45°). // ALGO: as the tester's circuit
pub const LINE_INTERCEPT_M: f64 = 1_000.0;
/// See [`LINE_INTERCEPT_M`].
pub const LINE_INTERCEPT_MAX_RAD: f64 = core::f64::consts::FRAC_PI_4;
/// The return pulls at most this, g. // ALGO: ADR-47
pub const RETURN_N_CAP: f64 = 2.0;
/// On the way home a goal more than this far round from the velocity, rad (120°), is turned onto
/// level, 90° at a time (the manoeuvre controller flies a goal straight behind as a loop, and at
/// the return's 2 g a slow loop climbs out of the circuit). // ALGO: ADR-47
pub const LEVEL_TURN_RAD: f64 = 2.094_395_102_393_195_5;
/// The ground guard, over the climb, the attack and the return: the straight path ahead is looked
/// at every this, s, this many times; with less than this clearance anywhere on it or below the
/// aircraft, m, it climbs away at this angle, rad (20°), in afterburner, pulling up to this, g.
/// // ALGO: ADR-47
pub const GROUND_LOOK_STEP_S: f64 = 1.0;
/// See [`GROUND_LOOK_STEP_S`].
pub const GROUND_LOOK_STEPS: u32 = 8;
/// See [`GROUND_LOOK_STEP_S`].
pub const GROUND_MIN_M: f64 = 250.0;
/// The ground guard's clearance on the way home, m (the return comes down to the circuit's 300 m).
pub const GROUND_MIN_RETURN_M: f64 = 150.0;
/// See [`GROUND_LOOK_STEP_S`].
pub const GROUND_CLIMB_RAD: f64 = 0.349_065_850_398_865_9;
/// See [`GROUND_LOOK_STEP_S`].
pub const GROUND_N_CAP: f64 = 5.0;
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
    /// The pilot's g stress (0 fine, 0.7 the blackout, 1 G-LOC; ADR-38).
    pub g_stress: f64,
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
            SortiePhase::Land | SortiePhase::Done => self.land(own, af),
        }
    }

    /// The tester's circuit from the downwind leg to the stop. Joined from the return, the
    /// aircraft may still be fast: the airbrake helps the tester's throttle on the downwind and
    /// base legs.
    fn land<E: FlightEnv>(&mut self, own: &OwnInputs, af: &Airframe<'_, E>) -> SortieOut {
        let t = af.t_s;
        let out = self
            .inner
            .step(af.model, af.fcs, af.ac, af.fs, own.levers, af.env, t);
        if self.inner.phase() == Phase::Stopped {
            self.phase = SortiePhase::Done;
        }
        let mut controls = out.controls;
        let fast = af.sensed().cas > fcs::tester::DOWNWIND_CAS_MPS + RETURN_AIRBRAKE_ABOVE_MPS;
        if fast && matches!(self.inner.phase(), Phase::Downwind | Phase::Base) {
            controls.airbrake = 1.0;
        }
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
            controls: guarded(
                Goal {
                    dir_bf: dir,
                    n_cap: CLIMB_N_CAP,
                    throttle,
                },
                af,
                GROUND_MIN_M,
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
        let (mut dir, mut n_cap) = if own.missiles > 0
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
        let s = af.sensed();
        let cas = s.cas;
        let v_s = fcs::tester::stall_speed(af.model, s.tel.mass_kg, 0.0);
        n_cap = n_cap.min((ATTACK_N_STALL_SHARE * (cas / v_s) * (cas / v_s)).max(1.0));
        let ease =
            ((own.g_stress - G_EASE_STRESS) / (G_EASED_STRESS - G_EASE_STRESS)).clamp(0.0, 1.0);
        n_cap = n_cap
            .min(ATTACK_N_CAP + ease * (G_EASED_N - ATTACK_N_CAP))
            .max(1.0);
        let slow = cas < ATTACK_MIN_CAS_MPS;
        if slow {
            let up = af.up();
            if dir.dot(up) > 0.0 {
                dir = horizontal(dir, up, v_hat);
            }
        }
        let aim_err = pip.map_or(f64::INFINITY, |p| angle_between(s_hat, p));
        let want = own.rounds > 0 && range <= GUN_RANGE_M && aim_err <= GUN_AIM_M / range.max(1.0);
        let fire_gun = self.trigger(want, t);
        let throttle = if slow || cas < CLIMB_CAS_MPS {
            AFTERBURNER
        } else {
            MILITARY
        };
        SortieOut {
            controls: guarded(
                Goal {
                    dir_bf: dir,
                    n_cap,
                    throttle,
                },
                af,
                GROUND_MIN_M,
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
        let dist = dmath::hypot(da, dc);
        // The height to hand over at: the circuit's over the ground at the crosswind corner, as a
        // height above MSL (the ground under the extended line falls towards the airfield, and
        // the tester descends only gently on its downwind leg), but never lower than this over
        // the ground below.
        let corner = runway_point_bf(
            &rw,
            RunwayPoint {
                along: CIRCUIT.corner_along_m,
                across: line_across,
            },
        );
        let height = af.env.height_msl_m(pos);
        let target_msl = (af.env.ground_height_m(corner) + fcs::tester::CIRCUIT_AGL_M)
            .max(af.env.ground_height_m(pos) + RETURN_HEIGHT_M);
        let at_height = (height - target_msl).abs() < JOIN_HEIGHT_TOL_M;
        if self.return_leg == 0 && dist < RETURN_CAPTURE_M && at_height {
            self.return_leg = 1;
            self.speed_i = 0.0;
        }
        let goal = if self.return_leg == 0 {
            let gamma = height_gamma(target_msl, height, HEIGHT_PER_RAD_M, RETURN_DESCENT_MAX_RAD);
            // Near the lead-in point but not yet at its height: round it, the way the aircraft
            // is already going round (the other way would put the goal behind it, and a goal
            // behind is flown as a loop), until the height is right; a radial correction holds
            // the orbit's radius.
            let (ha, hc) = if dist < RETURN_ORBIT_M && !at_height {
                let (ra, rc) = (-da / dist.max(1.0), -dc / dist.max(1.0));
                let (va, vc) = rw.velocity(af.ac.body.vel_bf);
                let sense = if va * -rc + vc * ra >= 0.0 { 1.0 } else { -1.0 };
                let k = ((RETURN_ORBIT_RADIUS_M - dist) / RETURN_ORBIT_RADIUS_M).clamp(-1.0, 1.0);
                (-rc * sense + k * ra, ra * sense + k * rc)
            } else {
                (da, dc)
            };
            let dir = climb_dir(heading_dir(rw.heading_of(ha, hc), up), up, gamma);
            let throttle = hold_speed(RETURN_CAS_MPS, s.cas, &mut self.speed_i, dt, MILITARY);
            Goal {
                dir_bf: dir,
                n_cap: RETURN_N_CAP,
                throttle,
            }
        } else {
            // On the extended downwind line: turn down it, and hand over to the tester as soon as
            // the heading is right; the tester's downwind leg intercepts the line and settles the
            // height and the speed over the 14 km to the base turn.
            let (va, vc) = rw.velocity(af.ac.body.vel_bf);
            let track = dmath::atan2(vc, va);
            if wrap(track - core::f64::consts::PI).abs() < JOIN_HEADING_TOL_RAD {
                self.inner.join_downwind(t);
                self.enter(SortiePhase::Land, t);
                return self.land(own, af);
            }
            let err = line_across - here.across;
            let intercept = dmath::atan(err / LINE_INTERCEPT_M)
                .clamp(-LINE_INTERCEPT_MAX_RAD, LINE_INTERCEPT_MAX_RAD);
            let heading = rw.heading_rad + core::f64::consts::PI - intercept;
            let gamma = height_gamma(target_msl, height, HEIGHT_PER_RAD_M, CLIMB_GAMMA_MAX_RAD);
            let throttle = hold_speed(RETURN_CAS_MPS, s.cas, &mut self.speed_i, dt, MILITARY);
            Goal {
                dir_bf: climb_dir(heading_dir(heading, up), up, gamma),
                n_cap: RETURN_N_CAP,
                throttle,
            }
        };
        let goal = Goal {
            dir_bf: level_turn(goal.dir_bf, af.ac.body.vel_bf, up),
            ..goal
        };
        let mut controls = guarded(goal, af, GROUND_MIN_RETURN_M);
        if s.cas > RETURN_CAS_MPS + RETURN_AIRBRAKE_ABOVE_MPS {
            controls.airbrake = 1.0;
        }
        SortieOut {
            controls,
            ..SortieOut::default()
        }
    }
}

/// A point of the runway frame in BF, at the threshold's height.
fn runway_point_bf(rw: &Runway, p: RunwayPoint) -> DVec3 {
    let (s, c) = (dmath::sin(rw.heading_rad), dmath::cos(rw.heading_rad));
    rw.threshold_bf + DVec3::new(p.along * s + p.across * c, p.along * c - p.across * s, 0.0)
}

/// On the way home a goal further than [`LEVEL_TURN_RAD`] round from the velocity becomes the
/// direction 90° to its side, at its own climb angle, so the turn is flown level.
fn level_turn(dir: DVec3, vel: DVec3, up: DVec3) -> DVec3 {
    let vh = horizontal(vel, up, DVec3::Y);
    let dh = horizontal(dir, up, vh);
    if vh.dot(dh) > dmath::cos(LEVEL_TURN_RAD) {
        return dir;
    }
    let right = vh.cross(up);
    let side = if dh.dot(right) >= 0.0 { right } else { -right };
    let gamma = dmath::asin(dir.normalized_or(vh).dot(up).clamp(-1.0, 1.0));
    climb_dir(side, up, gamma)
}

/// The ground guard ([`GROUND_LOOK_STEP_S`], keeping `min_m` clear) over a goal, then the
/// manoeuvre controller.
fn guarded<E: FlightEnv>(goal: Goal, af: &Airframe<'_, E>, min_m: f64) -> PilotControls {
    fly_goal(&ground_guard(goal, af, min_m), af)
}

/// The goal, or the climb away from the ground when the path ahead comes closer to it than
/// `min_m`.
fn ground_guard<E: FlightEnv>(goal: Goal, af: &Airframe<'_, E>, min_m: f64) -> Goal {
    let body = &af.ac.body;
    let clearance = |p: DVec3| af.env.height_msl_m(p) - af.env.ground_height_m(p);
    let low = clearance(body.pos_bf) < min_m
        || (1..=GROUND_LOOK_STEPS).any(|k| {
            let ahead = body.pos_bf + body.vel_bf * (f64::from(k) * GROUND_LOOK_STEP_S);
            clearance(ahead) < min_m
        });
    if !low {
        return goal;
    }
    let up = af.up();
    let level = horizontal(body.vel_bf, up, horizontal(goal.dir_bf, up, DVec3::Y));
    Goal {
        dir_bf: climb_dir(level, up, GROUND_CLIMB_RAD),
        n_cap: goal.n_cap.max(GROUND_N_CAP),
        throttle: AFTERBURNER,
    }
}

#[cfg(test)]
mod tests;
