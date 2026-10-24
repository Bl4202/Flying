//! The virtual tester (ADR-33): a deterministic pilot that takes off from the runway, flies a
//! left-hand circuit and lands, through the same stick, pedals, levers and buttons a person has.
//!
//! It is an instrument, not gameplay: it reads the aircraft's exact state, thinks in flight-path
//! rate and bank angle, and gives them to the control system as the stick position each assist
//! level reads (`sticks`). It writes [`PilotControls`] and button presses only, as the v0.1d AI
//! will, and the caller turns them into input frames, so the whole path from the frame to the
//! gear is the one the game uses.
//!
//! ```text
//! Hold -> Roll -> Rotate -> Climb -> Crosswind -> Downwind -> Base -> Final -> Flare -> Rollout
//!  2 s    brakes    at V_R   gear up   left 90     300 m AGL   gear,    3 degrees   sink      brakes,
//!         off, AB   to 10 deg                       130 m/s     flaps    to 12 m    1 m/s     stop
//! ```

pub mod geom;
pub mod sticks;

use flight::aircraft::{AircraftModel, AircraftState};
use flight::controls::PilotControls;
use flight::env::FlightEnv;
use flight::frames::euler;
use flight::physical::{G0, RHO0};
use simmath::dmath;

use crate::model::FcsModel;
use crate::sense::Sensed;
use crate::state::FcsState;
use geom::{Circuit, Leg, Runway, RunwayPoint, wrap};
use sticks::AirDemand;

/// How long the tester sits on the brakes before it starts the take-off, s.
pub const HOLD_S: f64 = 2.0;
/// Rotation speed over the take-off stall speed. // DATA: ADR-33
pub const ROTATE_FACTOR: f64 = 1.1;
/// The pitch attitude of the rotation, rad (10 degrees). // DATA: ADR-33
pub const ROTATE_PITCH_RAD: f64 = 0.174_532_925_199_432_96;
/// Full aft stick is held until the nose is this high, rad (6 degrees). // ALGO
pub const ROTATE_FIRM_RAD: f64 = 0.104_719_755_119_659_77;
/// The stick the attitude hold starts from after the firm pull. // ALGO
const ROTATE_FIRM_STICK: f64 = 0.5;
/// The fastest rotation, rad/s (8 degrees a second). // ALGO
pub const ROTATE_RATE_RPS: f64 = 0.139_626_340_159_546_36;
/// The height of the circuit above the runway, m. // DATA: ADR-33
pub const CIRCUIT_AGL_M: f64 = 300.0;
/// The downwind speed, calibrated, m/s. // DATA: ADR-33
pub const DOWNWIND_CAS_MPS: f64 = 130.0;
/// The base speed, calibrated, m/s. // ALGO
pub const BASE_CAS_MPS: f64 = 105.0;
/// The climb-out flight-path angle at most, rad (8 degrees). // DATA: ADR-33
pub const CLIMB_GAMMA_RAD: f64 = 0.139_626_340_159_546_36;
/// The glide path, rad (3 degrees). // DATA: ADR-33
pub const GLIDE_RAD: f64 = 0.052_359_877_559_829_89;
/// Where on the runway the glide path aims, m past the threshold. // DATA: ADR-33
pub const AIM_ALONG_M: f64 = 300.0;
/// The wheel height at which the flare starts, m. // DATA: ADR-33
pub const FLARE_AGL_M: f64 = 15.0;
/// The flare's sink rate at the ground, m/s (a hundredth of a metre per second of height more
/// at 12 m: `1.1 + 0.25 h` meets the 3 degree glide path's sink at the flare height). // ALGO
pub const FLARE_SINK_MPS: f64 = 1.1;
/// The base leg's height above the ground, m. // ALGO
const BASE_AGL_M: f64 = 170.0;
/// The throttle goes to idle below this wheel height in the flare, m. // DATA: ADR-33
const IDLE_AGL_M: f64 = 5.0;
/// Flight-path error to flight-path rate in the flare, 1/s. // ALGO
const K_GAMMA_FLARE: f64 = 1.5;
/// The bank and intercept limits in the flare, rad. // ALGO
const FLARE_BANK_RAD: f64 = 0.09;
const FLARE_INTERCEPT_RAD: f64 = 0.1;
/// The widest intercept angle on the final, rad. // ALGO
const FINAL_INTERCEPT_MAX_RAD: f64 = 0.35;
/// The flare's sink rate grows by this much per metre of wheel height, m/s. // ALGO
pub const FLARE_SINK_PER_M: f64 = 0.2;
/// Bank of the turns onto the crosswind and downwind legs, rad (45 degrees). // ALGO
pub const PATTERN_BANK_RAD: f64 = core::f64::consts::FRAC_PI_4;
/// Bank of the turn onto the base leg, rad (40 degrees). // ALGO
pub const BASE_TURN_BANK_RAD: f64 = 0.698_131_700_797_731_8;
/// Bank of the turn onto the final, rad (35 degrees). // ALGO
pub const FINAL_TURN_BANK_RAD: f64 = 0.610_865_238_198_015_4;
/// The final's bank once it is nearly on the centreline, rad (12 degrees). // ALGO
pub const FINAL_BANK_RAD: f64 = 0.209_439_510_239_319_55;
/// The gear goes down below this calibrated speed on the base leg (the limit is 128.6), m/s.
pub const GEAR_DOWN_CAS_MPS: f64 = 118.0;
/// The flaps may come down below this calibrated speed (the limit is 154.3), m/s.
pub const FLAPS_DOWN_CAS_MPS: f64 = 140.0;
/// The climb-out accelerates only to this calibrated speed while the gear is coming up (the
/// limit is 128.6), m/s. // ALGO
pub const GEAR_TRANSIT_CAS_MPS: f64 = 122.0;
/// The flaps come up above this calibrated speed on the climb-out, m/s. // DATA: ADR-33
pub const FLAPS_UP_CAS_MPS: f64 = 120.0;
/// Approach speed over the landing stall speed. // DATA: ADR-33
pub const APPROACH_FACTOR: f64 = 1.3;
/// The circuit's geometry.
pub const CIRCUIT: Circuit = Circuit {
    corner_along_m: 4_200.0,
    width_m: 3_800.0,
    base_before_m: 3_900.0,
};

/// Heading error to turn rate, 1/s. // ALGO
const K_OMEGA: f64 = 0.2;
/// The same two on the final, which has a runway to hit. // ALGO
const K_OMEGA_FINAL: f64 = 0.25;
const L_INTERCEPT_FINAL_M: f64 = 500.0;
/// Cross-track error at which the intercept angle reaches its limit, m. // ALGO
const L_INTERCEPT_M: f64 = 1_000.0;
/// The widest intercept angle in the circuit, rad. // ALGO
const INTERCEPT_MAX_RAD: f64 = core::f64::consts::FRAC_PI_4;
/// Flight-path error to flight-path rate, 1/s. // ALGO
const K_GAMMA: f64 = 1.0;
/// Height error to commanded flight-path angle, 1/s. // ALGO
const K_HEIGHT: f64 = 0.4;
/// The same two on the final approach. // ALGO
const K_GAMMA_FINAL: f64 = 0.7;
const K_HEIGHT_FINAL: f64 = 0.25;
/// Throttle per m/s of speed error and per m/s s. // ALGO
const K_SPEED_P: f64 = 0.03;
const K_SPEED_I: f64 = 0.004;
/// The ground steering: pedal per radian of heading error and per rad/s of yaw rate. // ALGO
const K_STEER_PSI: f64 = 1.0;
const K_STEER_RATE: f64 = 0.4;
/// Heading offered per metre of offset from the centreline on the ground, and its limit. // ALGO
const K_OFFSET: f64 = 0.004;
const MAX_OFFSET_HEADING_RAD: f64 = 0.03;
/// Pitch-rate loop of the rotation and the rollout: stick per rad/s, and its integral. // ALGO
const K_PITCH_Q: f64 = 3.0;
const K_PITCH_QI: f64 = 1.0;
/// The pitch-down rate after touchdown, rad/s (4 degrees a second), and the stick it starts from.
/// // ALGO
const ROLLOUT_NOSE_RATE_RPS: f64 = 0.069_813_170_079_773_18;
const ROLLOUT_STICK_START: f64 = -0.15;
/// A button is pressed at most every this many ticks. // ALGO: each press needs an edge
const PRESS_GAP_TICKS: u8 = 4;

/// Where the tester is in the sortie. The numbers are the phase codes of the circuit report.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum Phase {
    /// On the parking brake, engine at idle.
    Hold = 0,
    /// Brakes off, full power, holding the centreline.
    Roll = 1,
    /// At rotation speed: the nose to 10 degrees until the wheels leave.
    Rotate = 2,
    /// Climbing out on the runway heading; gear up, flaps up.
    Climb = 3,
    /// Left 90 degrees, level at the circuit height.
    Crosswind = 4,
    /// Opposite to the landing, 300 m, 130 m/s.
    Downwind = 5,
    /// Across to the extended centreline, gear and flaps down.
    Base = 6,
    /// On the 3 degree path.
    Final = 7,
    /// Below 12 m, the sink eased to about 1 m/s.
    Flare = 8,
    /// On the wheels: nose down, brakes, stop.
    Rollout = 9,
    /// Stopped.
    Stopped = 10,
}

impl Phase {
    /// How many phases there are.
    pub const COUNT: usize = 11;

    /// All of them, in order.
    pub const ALL: [Self; Self::COUNT] = [
        Self::Hold,
        Self::Roll,
        Self::Rotate,
        Self::Climb,
        Self::Crosswind,
        Self::Downwind,
        Self::Base,
        Self::Final,
        Self::Flare,
        Self::Rollout,
        Self::Stopped,
    ];

    /// The name in the report.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Hold => "hold",
            Self::Roll => "roll",
            Self::Rotate => "rotate",
            Self::Climb => "climb",
            Self::Crosswind => "crosswind",
            Self::Downwind => "downwind",
            Self::Base => "base",
            Self::Final => "final",
            Self::Flare => "flare",
            Self::Rollout => "rollout",
            Self::Stopped => "stopped",
        }
    }
}

/// The levers as the aircraft has them (the tester presses buttons; it does not set them).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Levers {
    /// The gear lever is down.
    pub gear_down: bool,
    /// The flap lever: 0 up, 1 take-off, 2 landing.
    pub flap_detent: u8,
    /// The parking brake is set.
    pub parking_brake: bool,
}

/// The buttons the tester presses this tick (each a press, not a hold).
#[allow(clippy::struct_excessive_bools, reason = "one flag per button")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Presses {
    /// The gear lever.
    pub gear: bool,
    /// The flap lever one detent down.
    pub flaps_down: bool,
    /// The flap lever one detent up.
    pub flaps_up: bool,
    /// The parking brake.
    pub parking_brake: bool,
}

/// What the tester does this tick.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TesterOut {
    /// The stick, pedals, throttle, brakes and airbrake lever (`airbrake` above 0.5 is held).
    pub controls: PilotControls,
    /// The buttons it presses.
    pub presses: Presses,
}

/// The flight condition, in the runway frame, that the tester decides on.
#[derive(Clone, Copy, Debug)]
struct View {
    s: Sensed,
    /// Position in the runway frame.
    pos: RunwayPoint,
    /// Heading of the ground track in the runway frame, rad (0 along the runway, right positive).
    track: f64,
    /// Heading of the nose in the runway frame, rad.
    nose: f64,
    /// Ground speed, m/s.
    gs: f64,
    /// Height of the lowest wheel above the ground, m.
    agl: f64,
    /// Vertical speed, m/s.
    vs: f64,
    /// Weight on the wheels.
    wow: bool,
    /// The nose wheel is on the ground.
    nose_wheel: bool,
    /// Mass, kg.
    mass: f64,
}

/// The virtual tester. Its state is plain numbers: it is `Copy`, allocates nothing, and the same
/// observations give the same controls.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tester {
    runway: Runway,
    phase: Phase,
    phase_start_s: f64,
    /// Which leg of the circuit is flown, 0 to 4.
    leg: usize,
    /// The integral of the speed error, m/s s.
    speed_i: f64,
    /// The integral of the pitch-rate error, rad.
    pitch_i: f64,
    /// Ticks until another button may be pressed.
    press_gap: u8,
    /// The simulation time the wheels left the ground, s.
    lift_off_s: Option<f64>,
    /// The tester wants the gear up.
    gear_up_pressed: bool,
}

impl Tester {
    /// A tester for a runway, on the brakes.
    #[must_use]
    pub fn new(runway: Runway) -> Self {
        Self {
            runway,
            phase: Phase::Hold,
            phase_start_s: 0.0,
            leg: 0,
            speed_i: 0.0,
            pitch_i: 0.0,
            press_gap: 0,
            lift_off_s: None,
            gear_up_pressed: false,
        }
    }

    /// Where it is.
    #[must_use]
    pub fn phase(&self) -> Phase {
        self.phase
    }

    /// When the wheels left the ground, s since the start of the run.
    #[must_use]
    pub fn lift_off_s(&self) -> Option<f64> {
        self.lift_off_s
    }

    /// The tester's numbers as words, for the state hash.
    #[must_use]
    pub fn words(&self) -> [u64; 6] {
        [
            self.phase as u64,
            self.leg as u64
                | u64::from(self.press_gap) << 8
                | u64::from(self.gear_up_pressed) << 16,
            self.phase_start_s.to_bits(),
            self.speed_i.to_bits(),
            self.pitch_i.to_bits(),
            self.lift_off_s.map_or(0, f64::to_bits),
        ]
    }

    fn enter(&mut self, phase: Phase, t_s: f64) {
        self.phase = phase;
        self.phase_start_s = t_s;
        if phase == Phase::Rollout {
            // The stick the rotation or the flare left behind is not the one for the wheels: start
            // with the nose being pushed down.
            self.pitch_i = ROLLOUT_STICK_START;
        }
    }

    /// One tick: what the tester does with the aircraft as it is now. `t_s` is the time since
    /// the start of the run.
    #[allow(clippy::too_many_arguments, reason = "the pilot's instruments")]
    pub fn step<E: FlightEnv>(
        &mut self,
        model: &AircraftModel,
        fcs: &FcsModel,
        ac: &AircraftState,
        fs: &FcsState,
        levers: Levers,
        env: &E,
        t_s: f64,
    ) -> TesterOut {
        let sense = model.sense(ac, env, t_s);
        let s = Sensed::of(model, ac, &sense, env);
        let v = self.view(model, ac, &s, env);
        let dt = fcs.tick_dt_s;
        self.press_gap = self.press_gap.saturating_sub(1);
        let mut out = TesterOut::default();

        self.advance(model, &v, t_s);
        match self.phase {
            Phase::Hold => {
                out.controls.throttle = 0.0;
            }
            Phase::Roll => {
                out.controls.throttle = 1.0;
                if levers.parking_brake {
                    self.press(&mut out, |p| p.parking_brake = true);
                }
                out.controls.yaw = Self::ground_yaw(&v, ac.body.omega_body.z);
            }
            Phase::Rotate => {
                out.controls.throttle = 1.0;
                out.controls.yaw = Self::ground_yaw(&v, ac.body.omega_body.z);
                out.controls.pitch = if v.s.theta < ROTATE_FIRM_RAD {
                    // Firm aft stick until the nose is off the stops, then the attitude is held.
                    self.pitch_i = ROTATE_FIRM_STICK;
                    1.0
                } else {
                    self.rotate_stick(&v, ROTATE_PITCH_RAD, ROTATE_RATE_RPS, dt)
                };
            }
            Phase::Rollout | Phase::Stopped => self.rollout(ac, &v, dt, &mut out),
            _ => self.fly(model, fcs, fs, ac, &sense, &v, levers, dt, &mut out),
        }
        out
    }

    fn view<E: FlightEnv>(
        &self,
        model: &AircraftModel,
        ac: &AircraftState,
        s: &Sensed,
        env: &E,
    ) -> View {
        let pos = self.runway.locate(ac.body.pos_bf);
        let (va, vc) = self.runway.velocity(ac.body.vel_bf);
        let up = env.up_bf(ac.body.pos_bf);
        let heading = euler(up, ac.body.att).heading_rad;
        let nose_wheel = model
            .gear
            .legs()
            .iter()
            .zip(ac.ground.in_contact.iter())
            .any(|(leg, &touching)| !leg.main && touching);
        View {
            s: *s,
            pos,
            track: dmath::atan2(vc, va),
            nose: wrap(heading - self.runway.heading_rad),
            gs: s.tel.ground_speed_mps,
            agl: s.tel.agl_m,
            vs: s.tel.vs_mps,
            wow: ac.ground.wow,
            nose_wheel,
            mass: s.tel.mass_kg,
        }
    }

    /// Presses a button unless one was pressed a moment ago.
    fn press(&mut self, out: &mut TesterOut, f: impl FnOnce(&mut Presses)) {
        if self.press_gap == 0 {
            f(&mut out.presses);
            self.press_gap = PRESS_GAP_TICKS;
        }
    }

    /// Phase changes.
    fn advance(&mut self, model: &AircraftModel, v: &View, t_s: f64) {
        let in_phase = t_s - self.phase_start_s;
        match self.phase {
            Phase::Hold if in_phase >= HOLD_S => self.enter(Phase::Roll, t_s),
            Phase::Roll
                if v.s.cas >= ROTATE_FACTOR * stall_speed(model, v.mass, take_off_tef(model)) =>
            {
                self.enter(Phase::Rotate, t_s);
            }
            Phase::Rotate if !v.wow && v.agl > 1.0 => {
                self.lift_off_s = Some(t_s);
                self.enter(Phase::Climb, t_s);
            }
            Phase::Climb | Phase::Crosswind | Phase::Downwind | Phase::Base => {
                self.next_leg(v, t_s);
            }
            Phase::Final | Phase::Flare if v.wow => self.enter(Phase::Rollout, t_s),
            Phase::Final if v.agl <= FLARE_AGL_M => self.enter(Phase::Flare, t_s),
            Phase::Rollout if v.gs < 2.0 && in_phase > 1.0 => self.enter(Phase::Stopped, t_s),
            _ => {}
        }
    }

    /// Starts the next leg of the circuit when the corner is near enough to turn it.
    fn next_leg(&mut self, v: &View, t_s: f64) {
        if self.leg >= 4 {
            return;
        }
        let legs = CIRCUIT.legs();
        let (_, to_go) = legs[self.leg].track(v.pos);
        let here = legs[self.leg].direction();
        let next = legs[self.leg + 1].direction();
        let turn = wrap(dmath::atan2(next.1, next.0) - dmath::atan2(here.1, here.0)).abs();
        let radius = v.gs * v.gs / (G0 * dmath::tan(turn_bank(self.leg + 1)));
        let lead = radius * dmath::tan(0.5 * turn) + v.gs;
        // The climb-out turns only once it has some height.
        let high_enough = self.leg != 0 || v.agl >= 100.0;
        if to_go <= lead && high_enough {
            self.leg += 1;
            let phase = match self.leg {
                1 => Phase::Crosswind,
                2 => Phase::Downwind,
                3 => Phase::Base,
                _ => Phase::Final,
            };
            self.enter(phase, t_s);
        }
    }

    /// The pedals that hold the runway heading on the wheels, from the heading error, the offset
    /// from the centreline and the yaw rate.
    fn ground_yaw(v: &View, yaw_rate_rps: f64) -> f64 {
        let reference =
            -(K_OFFSET * v.pos.across).clamp(-MAX_OFFSET_HEADING_RAD, MAX_OFFSET_HEADING_RAD);
        let err = wrap(v.nose - reference);
        let sched = 15.0 / v.gs.max(15.0);
        (-(K_STEER_PSI * err + K_STEER_RATE * yaw_rate_rps) * sched).clamp(-1.0, 1.0)
    }

    /// The pitch stick that turns the nose at `rate_rps` towards `target_rad`, on the wheels.
    fn rotate_stick(&mut self, v: &View, target_rad: f64, max_rate_rps: f64, dt: f64) -> f64 {
        let theta = v.s.theta;
        let q_des = (1.0 * (target_rad - theta)).clamp(-max_rate_rps, max_rate_rps);
        let err = q_des - v.s.rates.y;
        self.pitch_i = (self.pitch_i + K_PITCH_QI * err * dt).clamp(-0.3, 0.8);
        (self.pitch_i + K_PITCH_Q * err).clamp(-1.0, 1.0)
    }

    /// The throttle lever for a calibrated speed: a proportional-integral loop about `ff`.
    fn throttle(&mut self, cas: f64, target: f64, ff: f64, lo: f64, hi: f64, dt: f64) -> f64 {
        let e = target - cas;
        let u = ff + K_SPEED_P * e + K_SPEED_I * self.speed_i;
        if (u > lo && u < hi) || (u >= hi && e < 0.0) || (u <= lo && e > 0.0) {
            self.speed_i = (self.speed_i + e * dt).clamp(-150.0, 150.0);
        }
        (ff + K_SPEED_P * e + K_SPEED_I * self.speed_i).clamp(lo, hi)
    }

    /// The flight-path rate that takes the aircraft to the target height along the target path,
    /// rad/s.
    fn path_rate(v: &View, tg: &Targets, gentle: bool) -> f64 {
        // On the approach, at the back of the power curve, the loops are slower.
        let (k_h, k_g) = if gentle {
            (K_HEIGHT_FINAL, K_GAMMA_FINAL)
        } else {
            (K_HEIGHT, K_GAMMA)
        };
        let gamma_cmd = tg.gamma_ff + (k_h * (tg.height_m - v.agl) / v.s.v).clamp(-tg.down, tg.up);
        k_g * (gamma_cmd - v.s.gamma)
    }

    /// The bank that follows `leg`, rad.
    fn bank_for(v: &View, leg: &Leg, bank_max: f64, intercept_max: f64, tight: bool) -> f64 {
        let (err, _) = leg.track(v.pos);
        let dir = leg.direction();
        let heading = dmath::atan2(dir.1, dir.0);
        // The final holds the centreline harder than the circuit holds its legs.
        let (l_intercept, k_omega) = if tight {
            (L_INTERCEPT_FINAL_M, K_OMEGA_FINAL)
        } else {
            (L_INTERCEPT_M, K_OMEGA)
        };
        let intercept = dmath::atan(err / l_intercept).clamp(-intercept_max, intercept_max);
        let d_psi = wrap(heading - intercept - v.track);
        let omega = k_omega * d_psi;
        dmath::atan(v.gs * omega / G0).clamp(-bank_max, bank_max)
    }

    /// What the airborne phase aims for.
    fn targets(&self, model: &AircraftModel, ac: &AircraftState, v: &View, leg: &Leg) -> Targets {
        let t = |speed_cas, height_m, gamma_ff, down, up, bank_max| Targets {
            speed_cas,
            height_m,
            gamma_ff,
            down,
            up,
            bank_max,
            intercept_max: INTERCEPT_MAX_RAD,
        };
        match self.phase {
            Phase::Climb => {
                // Until the gear is up it must not be flown through its speed limit.
                let cas = if ac.surfaces.gear > 0.02 {
                    GEAR_TRANSIT_CAS_MPS
                } else {
                    DOWNWIND_CAS_MPS
                };
                t(cas, CIRCUIT_AGL_M, 0.0, 0.05, CLIMB_GAMMA_RAD, turn_bank(1))
            }
            Phase::Crosswind => t(DOWNWIND_CAS_MPS, CIRCUIT_AGL_M, 0.0, 0.1, 0.1, turn_bank(1)),
            Phase::Downwind => t(DOWNWIND_CAS_MPS, CIRCUIT_AGL_M, 0.0, 0.1, 0.1, turn_bank(2)),
            Phase::Base => t(BASE_CAS_MPS, BASE_AGL_M, 0.0, 0.1, 0.1, turn_bank(3)),
            _ => {
                let approach =
                    APPROACH_FACTOR * stall_speed(model, v.mass, model.actuators.tef.max);
                let glide = ((AIM_ALONG_M - v.pos.along) * dmath::tan(GLIDE_RAD)).max(0.0);
                // The bank eases from the turn onto the final to the small one that holds it.
                let off = (leg.track(v.pos).0.abs() - 100.0).clamp(0.0, 600.0) / 600.0;
                let bank = FINAL_BANK_RAD + off * (turn_bank(4) - FINAL_BANK_RAD);
                Targets {
                    intercept_max: FINAL_INTERCEPT_MAX_RAD,
                    ..t(approach, glide, -GLIDE_RAD, 0.12, 0.05, bank)
                }
            }
        }
    }

    /// The gear, the flaps and the throttle for the phase.
    fn configure(
        &mut self,
        speed_cas: f64,
        v: &View,
        levers: Levers,
        dt: f64,
        out: &mut TesterOut,
    ) {
        let cas = v.s.cas;
        match self.phase {
            Phase::Climb => {
                if levers.gear_down && v.vs > 2.0 && v.agl > 10.0 && !self.gear_up_pressed {
                    self.press(out, |p| p.gear = true);
                    self.gear_up_pressed = true;
                }
                if levers.flap_detent > 0 && cas >= FLAPS_UP_CAS_MPS {
                    self.press(out, |p| p.flaps_up = true);
                }
            }
            Phase::Base | Phase::Final | Phase::Flare => {
                if !levers.gear_down && cas < GEAR_DOWN_CAS_MPS {
                    self.press(out, |p| p.gear = true);
                }
                if levers.flap_detent < 2 && cas < FLAPS_DOWN_CAS_MPS {
                    self.press(out, |p| p.flaps_down = true);
                }
            }
            _ => {}
        }
        // Afterburner while the gear comes up, then the speed; idle in the last few metres.
        out.controls.throttle = match self.phase {
            Phase::Climb if levers.gear_down || cas < 115.0 => 1.0,
            Phase::Climb => self.throttle(cas, speed_cas, 0.25, 0.0, 1.0, dt),
            Phase::Flare if v.agl < IDLE_AGL_M => 0.0,
            Phase::Flare => self.throttle(cas, speed_cas, 0.1, 0.0, 0.8, dt),
            _ => self.throttle(cas, speed_cas, 0.25, 0.0, 0.8, dt),
        };
    }

    /// The airborne phases.
    #[allow(clippy::too_many_arguments, reason = "the pilot's instruments")]
    fn fly(
        &mut self,
        model: &AircraftModel,
        fcs: &FcsModel,
        fs: &FcsState,
        ac: &AircraftState,
        sense: &flight::aircraft::Sense,
        v: &View,
        levers: Levers,
        dt: f64,
        out: &mut TesterOut,
    ) {
        let legs = CIRCUIT.legs();
        let leg = &legs[self.leg];
        let tg = self.targets(model, ac, v, leg);
        self.configure(tg.speed_cas, v, levers, dt, out);

        // The vertical and lateral demands.
        let flare = self.phase == Phase::Flare;
        let gamma_dot = if flare {
            let sink = FLARE_SINK_MPS + FLARE_SINK_PER_M * v.agl.max(0.0);
            let gamma_cmd = dmath::asin((-sink / v.s.v).clamp(-0.3, 0.3));
            K_GAMMA_FLARE * (gamma_cmd - v.s.gamma)
        } else {
            Self::path_rate(v, &tg, self.phase == Phase::Final)
        };
        let bank = if flare {
            Self::bank_for(v, leg, FLARE_BANK_RAD, FLARE_INTERCEPT_RAD, true)
        } else {
            Self::bank_for(
                v,
                leg,
                tg.bank_max,
                tg.intercept_max,
                self.phase == Phase::Final,
            )
        };
        let st = sticks::air(
            fs.assist,
            model,
            fcs,
            ac,
            sense,
            &v.s,
            &AirDemand { gamma_dot, bank },
        );
        out.controls.pitch = st.pitch;
        out.controls.roll = st.roll;
        out.controls.yaw = st.yaw;
        // After lift-off, until the ground law has let go, the pitch stick is still read as an
        // elevator by the part of the blend that is the ground law: keep the rotation going.
        if fs.ground_weight > 0.05 && self.phase == Phase::Climb && v.s.theta < ROTATE_PITCH_RAD {
            out.controls.pitch =
                out.controls
                    .pitch
                    .max(self.rotate_stick(v, ROTATE_PITCH_RAD, ROTATE_RATE_RPS, dt));
        }
    }

    /// On the wheels after the landing: the nose down, the brakes on once the nose wheel is down,
    /// the airbrake open, the centreline held.
    fn rollout(&mut self, ac: &AircraftState, v: &View, dt: f64, out: &mut TesterOut) {
        out.controls.throttle = 0.0;
        out.controls.airbrake = 1.0;
        out.controls.yaw = Self::ground_yaw(v, ac.body.omega_body.z);
        // The nose comes down at a steady rate, then the stick comes back to rest.
        let q_des = (-v.s.theta).clamp(-ROLLOUT_NOSE_RATE_RPS, 0.0);
        let err = q_des - v.s.rates.y;
        self.pitch_i = (self.pitch_i + K_PITCH_QI * err * dt).clamp(-0.5, 0.2);
        out.controls.pitch = (self.pitch_i + K_PITCH_Q * err).clamp(-0.6, 0.2);
        if v.nose_wheel || v.s.theta.abs() < 0.03 {
            out.controls.brake_l = 1.0;
            out.controls.brake_r = 1.0;
        }
    }
}

/// What an airborne phase aims for.
#[derive(Clone, Copy, Debug)]
struct Targets {
    /// Calibrated airspeed, m/s.
    speed_cas: f64,
    /// Height of the lowest wheel above the ground, m.
    height_m: f64,
    /// The flight-path angle the height is approached along (the glide path), rad.
    gamma_ff: f64,
    /// How far below and above `gamma_ff` the height loop may command, rad.
    down: f64,
    up: f64,
    /// The largest bank, rad.
    bank_max: f64,
    /// The widest intercept angle, rad.
    intercept_max: f64,
}

/// The bank the turn onto leg `leg` is flown at, rad.
fn turn_bank(leg: usize) -> f64 {
    match leg {
        1 | 2 => PATTERN_BANK_RAD,
        3 => BASE_TURN_BANK_RAD,
        _ => FINAL_TURN_BANK_RAD,
    }
}

/// The trailing-edge flap angle of the take-off detent, rad.
#[must_use]
pub fn take_off_tef(model: &AircraftModel) -> f64 {
    0.5 * model.actuators.tef.max
}

/// The stall speed at sea level with the flaps at `tef_rad`: `C_L,max` is the table's at Mach 0
/// plus the flap increment (ADR-32), m/s.
#[must_use]
pub fn stall_speed(model: &AircraftModel, mass_kg: f64, tef_rad: f64) -> f64 {
    let cl = model.aero.cl_max(0.0) + model.aero.flap_increments(tef_rad).0;
    dmath::sqrt(2.0 * mass_kg * G0 / (RHO0 * model.aero.wing_area_m2() * cl))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_content::kestrel;

    #[test]
    fn the_stall_speed_follows_the_root_of_the_mass_and_the_flaps_lower_it() {
        let (model, _) = kestrel();
        let clean = stall_speed(&model, 10_000.0, 0.0);
        let to = stall_speed(&model, 10_000.0, take_off_tef(&model));
        let ldg = stall_speed(&model, 10_000.0, model.actuators.tef.max);
        assert!(to < clean && ldg <= to, "{clean} {to} {ldg}");
        // 0.3 more lift coefficient with take-off flaps.
        let cl = model.aero.cl_max(0.0);
        assert!((to / clean - dmath::sqrt(cl / (cl + 0.3))).abs() < 1e-6);
        let heavy = stall_speed(&model, 12_000.0, 0.0);
        assert!((heavy / clean - dmath::sqrt(1.2)).abs() < 1e-9);
        // A worked value: 10 260 kg with take-off flaps stalls at about 55.6 m/s.
        assert!((stall_speed(&model, 10_260.0, take_off_tef(&model)) - 55.6).abs() < 0.2);
    }

    #[test]
    fn the_phases_are_numbered_in_order_and_named_once() {
        for (i, p) in Phase::ALL.iter().enumerate() {
            assert_eq!(*p as usize, i);
        }
        let mut names: Vec<_> = Phase::ALL.iter().map(|p| p.name()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), Phase::COUNT);
        assert!(Phase::Hold < Phase::Rollout);
    }

    #[test]
    fn the_circuit_fits_the_airfield_and_its_turns_fit_the_legs() {
        // The legs stay inside the 4 km plain round the runway (ADR-35).
        let legs = CIRCUIT.legs();
        for leg in legs {
            for p in [leg.from, leg.to] {
                assert!(p.across.abs() <= 3_900.0, "{p:?}");
                assert!(p.along >= -3_900.0 && p.along <= 3_000.0 + 3_900.0, "{p:?}");
            }
        }
        // The turns onto the crosswind and downwind legs at the downwind speed take less than the
        // legs have between them.
        let radius = DOWNWIND_CAS_MPS * DOWNWIND_CAS_MPS / (G0 * dmath::tan(PATTERN_BANK_RAD));
        assert!(
            2.0 * (radius + DOWNWIND_CAS_MPS) <= legs[1].length(),
            "{radius}"
        );
    }

    #[test]
    fn the_tester_starts_on_the_brakes_and_hashes_its_state() {
        let t = Tester::new(Runway {
            threshold_bf: simmath::math::DVec3::ZERO,
            heading_rad: 0.0,
            length_m: 3_000.0,
            width_m: 45.0,
        });
        assert_eq!(t.phase(), Phase::Hold);
        assert_eq!(t.lift_off_s(), None);
        let mut u = t;
        u.leg = 2;
        assert_ne!(t.words(), u.words());
    }
}
