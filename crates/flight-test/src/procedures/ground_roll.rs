//! The take-off and landing ground rolls (GDD C.7 to C.9, ADR-32).
//!
//! Both points are initialised ground-roll segments, as C.8 defines them. The aircraft is put on
//! its gear at the static pose, the roll is flown through the real gear model, and the distance is
//! measured: `to_roll` from brake release at maximum afterburner to `V_LOF = 1.2 V_s,TO` with no
//! rotation; `ldg_roll` from `V_TD = 1.15 V_s,LDG` at idle with the airbrake out and full braking
//! to a ground speed of 2 m/s. Lift-off and the flare are the circuit gate's (ADR-33).
//!
//! The pilot is an instrument: surfaces at their configuration, elevator centred, brakes at 0 or
//! 1, and a small loop from heading and offset to the nose wheel and the rudder to hold the
//! centreline. Two static cross-checks gate each run: the C.8 point-mass integral with the
//! model's own forces at the static attitude, and the closure of the kinetic energy against the
//! work of the forces along the flown path.

use content::EnvelopeConfig;
use content::aircraft::EnvelopePoint;
use flight::actuators::Surfaces;
use flight::aero::{AeroInput, AeroModel};
use flight::aircraft::{AircraftModel, AircraftState, Commands};
use flight::airdata::AirData;
use flight::atmosphere::NonStandardDay;
use flight::engine::{EngineCommand, EngineConditions, EngineModel};
use flight::env::{FlightEnv, SphereSea};
use flight::frames::euler;
use flight::gear::{self, ANTI_SKID_FACTOR, GearHeld, WheelCommands};
use flight::physical::{G0, RHO0};
use flight::surface::friction;
use serde_json::{Value, json};
use simmath::dmath;
use simmath::math::DVec3;

use super::{TICK_DT_S, TRACE_EVERY, Trace, config_surfaces};

/// Simulated-time cap of the take-off roll, s.
pub const TO_CAP_S: f64 = 30.0;
/// Simulated-time cap of the landing roll, s.
pub const LDG_CAP_S: f64 = 40.0;
/// The landing roll ends at this ground speed, m/s.
pub const LDG_END_MPS: f64 = 2.0;
/// Lift-off speed over the take-off stall speed. // DATA: GDD C.8
pub const LOF_FACTOR: f64 = 1.2;
/// Touchdown speed over the landing stall speed. // DATA: GDD C.8
pub const TD_FACTOR: f64 = 1.15;
/// The point-mass cross-check band for the take-off, percent. // DATA: ADR-32
pub const TO_BAND_PCT: f64 = 2.0;
/// The point-mass cross-check band for the landing, percent. // DATA: ADR-32
pub const LDG_BAND_PCT: f64 = 3.0;
/// The energy-closure band, percent. // DATA: ADR-32
pub const ENERGY_BAND_PCT: f64 = 1.0;
/// A run is invalid if the aircraft strays this far from the centreline, m. // DATA: ADR-32
pub const LATERAL_LIMIT_M: f64 = 2.0;
/// Pitch may differ from the static pose by this much in the take-off, degrees. // DATA: ADR-32
pub const TO_PITCH_BAND_DEG: f64 = 0.5;
/// Pitch may differ from the static pose by this much in the landing, degrees: full braking
/// moves load to the nose strut, which is physical. // DATA: ADR-32 (amended)
pub const LDG_PITCH_BAND_DEG: f64 = 3.0;
/// Step of the point-mass integral, s (C.8). // DATA: GDD C.8
const POINT_MASS_DT_S: f64 = 0.01;

/// Heading change commanded per metre of offset, rad. // ALGO: centreline loop
const K_OFFSET: f64 = 0.004;
/// The largest heading change the offset may command, rad. // ALGO
const MAX_ASSIST_RAD: f64 = 0.03;
/// Nose-wheel angle per radian of heading error at and below [`SCHED_V_MPS`]. // ALGO
const K_HEADING: f64 = 1.0;
/// Above this ground speed the steering gain falls as 1/V, m/s. // ALGO
const SCHED_V_MPS: f64 = 15.0;
/// Nose-wheel angle per rad/s of yaw rate. // ALGO
const K_YAW_DAMP: f64 = 0.15;
/// Rudder per radian of heading error, before the dynamic-pressure schedule. // ALGO
const K_RUDDER: f64 = 0.6;
/// The largest rudder the loop uses, rad. // ALGO
const RUDDER_MAX_RAD: f64 = 0.15;
/// Dynamic pressure at which the rudder gain halves, Pa. // ALGO
const Q_REF_PA: f64 = 2000.0;

/// The columns of a ground-roll trace.
pub const GROUND_COLUMNS: [&str; 18] = [
    "t_s",
    "dist_m",
    "lateral_m",
    "gs_mps",
    "pitch_deg",
    "roll_deg",
    "heading_deg",
    "cg_agl_m",
    "thrust_n",
    "brake",
    "steer_deg",
    "rudder_deg",
    "ground_fx_n",
    "normal_n",
    "nz_g",
    "accel_mps2",
    "energy_err_pct",
    "wow",
];

/// Which roll.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Brake release at maximum afterburner to lift-off speed.
    TakeOff,
    /// Touchdown speed to a stop with full braking.
    Landing,
}

impl Kind {
    /// The roll of an envelope configuration, if it is one.
    #[must_use]
    pub fn of(config: EnvelopeConfig) -> Option<Self> {
        match config {
            EnvelopeConfig::ToFlapsAb => Some(Self::TakeOff),
            EnvelopeConfig::LdgBrakes => Some(Self::Landing),
            _ => None,
        }
    }

    /// The band of the point-mass cross-check, percent.
    #[must_use]
    pub fn band_pct(self) -> f64 {
        match self {
            Self::TakeOff => TO_BAND_PCT,
            Self::Landing => LDG_BAND_PCT,
        }
    }

    /// The report's description of the method.
    #[must_use]
    pub fn method(self) -> &'static str {
        match self {
            Self::TakeOff => {
                "static pose on the gear, engine steady at maximum afterburner, brake release, \
                 no rotation, CG distance to V_LOF = 1.2 V_s,TO"
            }
            Self::Landing => {
                "static pose on the gear at V_TD = 1.15 V_s,LDG, idle, airbrake out, landing \
                 flaps, full brakes with anti-skid, CG distance to 2 m/s"
            }
        }
    }

    fn pitch_band_deg(self) -> f64 {
        match self {
            Self::TakeOff => TO_PITCH_BAND_DEG,
            Self::Landing => LDG_PITCH_BAND_DEG,
        }
    }

    fn cap_s(self) -> f64 {
        match self {
            Self::TakeOff => TO_CAP_S,
            Self::Landing => LDG_CAP_S,
        }
    }
}

/// The world of the rolls: the sphere with a paved surface at height 0, dry or at a wetness.
#[must_use]
pub fn world(wetness: f64) -> SphereSea {
    SphereSea {
        wetness,
        ..SphereSea::default()
    }
}

/// Stall speed with the trailing-edge flaps at `tef_rad`: `C_L,max` is the table's at Mach 0
/// plus the flap increment (ADR-32), m/s.
#[must_use]
pub fn stall_speed_mps(model: &AircraftModel, mass_kg: f64, tef_rad: f64) -> f64 {
    let cl = model.aero.cl_max(0.0) + model.aero.flap_increments(tef_rad).0;
    dmath::sqrt(2.0 * mass_kg * G0 / (RHO0 * model.aero.wing_area_m2() * cl))
}

/// Where a roll starts and what it ends at.
#[derive(Clone, Copy, Debug)]
struct Start {
    state: AircraftState,
    pitch_rad: f64,
    v0_mps: f64,
    v_end_mps: f64,
    cfg: Surfaces,
    engine: EngineCommand,
}

/// The aircraft on its gear at the start of the roll. The struts carry the weight less the lift
/// and the vertical thrust at the starting speed, found by iterating the pose with the model's
/// own free forces.
fn start<E: FlightEnv>(
    model: &AircraftModel,
    p: &EnvelopePoint,
    kind: Kind,
    env: &E,
) -> Option<Start> {
    let cfg = config_surfaces(model, p.config);
    let mass_kg = f64::from(p.mass_kg);
    let fuel_kg = mass_kg - model.mass.empty_kg();
    let vs = stall_speed_mps(model, mass_kg, cfg.tef_rad);
    let (v0, v_end, engine) = match kind {
        Kind::TakeOff => (
            0.0,
            LOF_FACTOR * vs,
            EngineCommand::max_ab(model.ab_stages()),
        ),
        Kind::Landing => (TD_FACTOR * vs, LDG_END_MPS, EngineCommand::IDLE),
    };
    let mut lift_n = 0.0;
    let mut out = None;
    for _ in 0..6 {
        let (mut state, pose) =
            model.static_state(fuel_kg, lift_n, 0.0, DVec3::ZERO, env, engine)?;
        state.surfaces = cfg;
        state.mass_frozen = true;
        state.body.vel_bf = DVec3::new(0.0, v0, 0.0);
        let tel = model.telemetry(&state, env, 0.0);
        let free = state
            .body
            .att
            .rotate(tel.force_body_n - tel.ground_force_body_n);
        let up = free.dot(env.up_bf(state.body.pos_bf));
        let settled = (up - lift_n).abs() < 0.5;
        lift_n = up;
        out = Some(Start {
            state,
            pitch_rad: pose.pitch_rad,
            v0_mps: v0,
            v_end_mps: v_end,
            cfg,
            engine,
        });
        if settled {
            break;
        }
    }
    out
}

/// Nose-wheel angle and rudder that hold the centreline, rad.
fn lateral_loop(model: &AircraftModel, s: &AircraftState, qbar_pa: f64, gs_mps: f64) -> (f64, f64) {
    let env = world(0.0);
    let e = euler(env.up_bf(s.body.pos_bf), s.body.att);
    let offset = s.body.pos_bf.x;
    let reference = (-K_OFFSET * offset).clamp(-MAX_ASSIST_RAD, MAX_ASSIST_RAD);
    let err = e.heading_rad - reference;
    let steer_max = model
        .gear
        .legs()
        .iter()
        .map(|l| l.steer_max_rad)
        .fold(0.0, f64::max);
    let sched = SCHED_V_MPS / gs_mps.max(SCHED_V_MPS);
    let steer =
        (-K_HEADING * err * sched - K_YAW_DAMP * s.body.omega_body.z).clamp(-steer_max, steer_max);
    let rudder =
        (K_RUDDER * err * Q_REF_PA / (qbar_pa + Q_REF_PA)).clamp(-RUDDER_MAX_RAD, RUDDER_MAX_RAD);
    (steer, rudder)
}

/// A flown roll.
#[derive(Clone, Debug, PartialEq)]
pub struct Run {
    /// CG distance from the start to the end speed, m.
    pub distance_m: f64,
    /// Speed the run started at, m/s.
    pub start_speed_mps: f64,
    /// Speed the run ended at, m/s.
    pub end_speed_mps: f64,
    /// Simulated time to the end speed, s.
    pub sim_seconds: f64,
    /// 10 Hz time history.
    pub trace: Trace,
    /// Integrator faults (any is a failed run).
    pub faults: u32,
    /// The cap was reached before the end speed.
    pub timed_out: bool,
    /// Why the measurement is not valid, if it is not.
    pub invalid: Option<String>,
    /// Pitch of the static pose, degrees.
    pub static_pitch_deg: f64,
    /// Largest difference from it, degrees.
    pub pitch_dev_max_deg: f64,
    /// Largest offset from the centreline, m.
    pub lateral_max_m: f64,
    /// Change of kinetic energy, J.
    pub ke_gain_j: f64,
    /// Work of every force along the flown path, J.
    pub work_j: f64,
    /// The aircraft's mass, kg.
    pub mass_kg: f64,
    /// Every tick: ground speed, pitch (rad) and CG height above the ground (m). The point-mass
    /// cross-check flies the attitude the run flew.
    pub samples: Vec<[f64; 3]>,
}

impl Run {
    /// Measurement quality figures for the report.
    #[must_use]
    pub fn stats(&self) -> Value {
        json!({
            "start_speed_mps": self.start_speed_mps,
            "end_speed_mps": self.end_speed_mps,
            "static_pitch_deg": self.static_pitch_deg,
            "pitch_dev_max_deg": self.pitch_dev_max_deg,
            "lateral_max_m": self.lateral_max_m,
            "ke_gain_j": self.ke_gain_j,
            "work_j": self.work_j,
            "mass_kg": self.mass_kg,
        })
    }
}

/// Turns the aircraft off the centreline by a heading error and an offset, keeping its speed.
fn perturb(
    s: &mut AircraftState,
    env: &SphereSea,
    v0_mps: f64,
    heading_error_rad: f64,
    offset_m: f64,
) {
    if heading_error_rad == 0.0 && offset_m == 0.0 {
        return;
    }
    let up = env.up_bf(s.body.pos_bf);
    let e = euler(up, s.body.att);
    s.body.att = flight::frames::attitude(
        up,
        flight::frames::Euler {
            heading_rad: e.heading_rad + heading_error_rad,
            ..e
        },
    );
    s.body.pos_bf.x += offset_m;
    // The velocity stays level, along the new heading.
    let nose = s.body.att.rotate(DVec3::X);
    let level = (nose - up * nose.dot(up)).normalized_or(DVec3::Y);
    s.body.vel_bf = level * v0_mps;
}

/// Brakes off for the take-off, full on every braked leg for the landing.
fn brake_commands(model: &AircraftModel, kind: Kind) -> WheelCommands {
    let mut wheels = WheelCommands::NONE;
    if kind == Kind::Landing {
        for (b, leg) in wheels.brake.iter_mut().zip(model.gear.legs()) {
            *b = if leg.braked { 1.0 } else { 0.0 };
        }
    }
    wheels
}

/// An empty run record for a start.
fn new_run(kind: Kind, st: &Start, mass_kg: f64, cg_agl_m: f64) -> Run {
    Run {
        distance_m: f64::NAN,
        start_speed_mps: st.v0_mps,
        end_speed_mps: st.v_end_mps,
        sim_seconds: 0.0,
        trace: Trace {
            name: match kind {
                Kind::TakeOff => "ground_to_roll".into(),
                Kind::Landing => "ground_ldg_roll".into(),
            },
            columns: &GROUND_COLUMNS,
            rows: Vec::new(),
        },
        faults: 0,
        timed_out: false,
        invalid: None,
        static_pitch_deg: st.pitch_rad.to_degrees(),
        pitch_dev_max_deg: 0.0,
        lateral_max_m: 0.0,
        ke_gain_j: 0.0,
        work_j: 0.0,
        mass_kg,
        samples: vec![[st.v0_mps, st.pitch_rad, cg_agl_m]],
    }
}

/// Flies the roll of a point. `None` if the aircraft has no gear to stand on.
#[must_use]
pub fn fly(model: &AircraftModel, p: &EnvelopePoint, kind: Kind, env: &SphereSea) -> Option<Run> {
    fly_from(model, p, kind, env, 0.0, 0.0)
}

/// [`fly`] with a heading error (rad) and an offset (m) at the start: the centreline loop's test.
#[must_use]
pub fn fly_from(
    model: &AircraftModel,
    p: &EnvelopePoint,
    kind: Kind,
    env: &SphereSea,
    heading_error_rad: f64,
    offset_m: f64,
) -> Option<Run> {
    let st = start(model, p, kind, env)?;
    let mut s = st.state;
    perturb(&mut s, env, st.v0_mps, heading_error_rad, offset_m);
    let start_pos = s.body.pos_bf;
    let n_legs = model.gear.legs().len();
    let mass_kg = model.telemetry(&s, env, 0.0).mass_kg;
    let mut wheels = brake_commands(model, kind);
    let mut run = new_run(
        kind,
        &st,
        mass_kg,
        env.height_msl_m(s.body.pos_bf) - env.ground_height_m(s.body.pos_bf),
    );
    let ke = |t: &flight::aircraft::Telemetry, s: &AircraftState| {
        let _ = t;
        0.5 * mass_kg * s.body.vel_bf.length_squared()
    };
    let force_bf = |s: &AircraftState, t: &flight::aircraft::Telemetry| {
        s.body.att.rotate(t.force_body_n) + env.gravity_bf(s.body.pos_bf) * mass_kg
    };

    let mut tel = model.telemetry(&s, env, 0.0);
    let ke0 = ke(&tel, &s);
    let mut dist = 0.0;
    let mut prev_gs = tel.ground_speed_mps;
    let ticks = (kind.cap_s() / TICK_DT_S).round() as u64;
    for tick in 0..ticks {
        let t = tick as f64 * TICK_DT_S;
        let (steer, rudder) = lateral_loop(model, &s, tel.air.qbar_pa, tel.ground_speed_mps);
        wheels.steer_rad = steer;
        let cmd = Commands {
            surfaces: Surfaces {
                rudder_rad: rudder,
                ..st.cfg
            },
            engine: st.engine,
            wheels,
        };
        let before = (s.body.pos_bf, force_bf(&s, &tel));
        let out = model.step_tick(&mut s, &cmd, env, t);
        run.faults += out.faults;
        if out.faults > 0 {
            return Some(run);
        }
        tel = model.telemetry(&s, env, t + TICK_DT_S);
        let after = force_bf(&s, &tel);
        run.work_j += 0.5 * (before.1 + after).dot(s.body.pos_bf - before.0);
        let d_prev = dist;
        let delta = s.body.pos_bf - start_pos;
        dist = dmath::sqrt(delta.x * delta.x + delta.y * delta.y);

        let e = euler(env.up_bf(s.body.pos_bf), s.body.att);
        let pitch_dev = (e.pitch_rad - st.pitch_rad).to_degrees().abs();
        run.samples.push([
            tel.ground_speed_mps,
            e.pitch_rad,
            env.height_msl_m(s.body.pos_bf) - env.ground_height_m(s.body.pos_bf),
        ]);
        run.pitch_dev_max_deg = run.pitch_dev_max_deg.max(pitch_dev);
        run.lateral_max_m = run.lateral_max_m.max(s.body.pos_bf.x.abs());
        if run.invalid.is_none() {
            run.invalid = invalid_reason(model, &s, n_legs, kind, pitch_dev, t + TICK_DT_S);
        }
        if tick.is_multiple_of(TRACE_EVERY) {
            run.trace.rows.push(trace_row(
                t + TICK_DT_S,
                dist,
                &s,
                &tel,
                &e,
                (wheels.brake[0], steer, rudder),
                (tel.ground_speed_mps - prev_gs) / TICK_DT_S,
                (
                    0.5 * mass_kg * s.body.vel_bf.length_squared() - ke0,
                    run.work_j,
                ),
            ));
        }
        let gs = tel.ground_speed_mps;
        let done = match kind {
            Kind::TakeOff => gs >= st.v_end_mps,
            Kind::Landing => gs <= st.v_end_mps,
        };
        if done {
            let f = if (gs - prev_gs).abs() > 1e-12 {
                ((st.v_end_mps - prev_gs) / (gs - prev_gs)).clamp(0.0, 1.0)
            } else {
                1.0
            };
            run.distance_m = d_prev + f * (dist - d_prev);
            run.sim_seconds = (tick as f64 + f) * TICK_DT_S;
            run.ke_gain_j = ke(&tel, &s) - ke0;
            return Some(run);
        }
        prev_gs = gs;
    }
    run.timed_out = true;
    run.sim_seconds = kind.cap_s();
    Some(run)
}

/// Why the state at `t_s` invalidates the measurement, if it does.
fn invalid_reason(
    model: &AircraftModel,
    s: &AircraftState,
    n_legs: usize,
    kind: Kind,
    pitch_dev_deg: f64,
    t_s: f64,
) -> Option<String> {
    let _ = model;
    if let Some(i) = (0..n_legs).find(|&i| !s.ground.in_contact[i]) {
        return Some(format!("leg {i} left the ground at {t_s:.2} s"));
    }
    if s.ground.skid {
        return Some(format!("the airframe touched the ground at {t_s:.2} s"));
    }
    if s.body.pos_bf.x.abs() > LATERAL_LIMIT_M {
        return Some(format!(
            "{:.1} m off the centreline at {t_s:.2} s",
            s.body.pos_bf.x
        ));
    }
    if pitch_dev_deg > kind.pitch_band_deg() {
        return Some(format!(
            "pitch {pitch_dev_deg:.2} deg from the static pose at {t_s:.2} s (limit {:.1})",
            kind.pitch_band_deg()
        ));
    }
    None
}

#[allow(clippy::too_many_arguments, reason = "one row of a trace")]
fn trace_row(
    t_s: f64,
    dist_m: f64,
    s: &AircraftState,
    tel: &flight::aircraft::Telemetry,
    e: &flight::frames::Euler,
    (brake, steer, rudder): (f64, f64, f64),
    accel: f64,
    (ke_gain, work): (f64, f64),
) -> [f64; 18] {
    let ground = s.body.att.rotate(tel.ground_force_body_n);
    let energy_err = if ke_gain.abs() > 1.0e3 {
        (ke_gain - work) / ke_gain.abs() * 100.0
    } else {
        0.0
    };
    [
        t_s,
        dist_m,
        s.body.pos_bf.x,
        tel.ground_speed_mps,
        e.pitch_rad.to_degrees(),
        e.roll_rad.to_degrees(),
        e.heading_rad.to_degrees(),
        tel.agl_m,
        tel.thrust_n,
        brake,
        steer.to_degrees(),
        rudder.to_degrees(),
        ground.y,
        ground.z,
        tel.nz_g,
        accel,
        energy_err,
        f64::from(u8::from(tel.wow)),
    ]
}

/// The pitch and CG height the run had at a ground speed, linearly interpolated (the speed rises
/// along a take-off and falls along a landing).
fn flown_at(samples: &[[f64; 3]], kind: Kind, v: f64) -> (f64, f64) {
    let i = match kind {
        Kind::TakeOff => samples.partition_point(|s| s[0] < v),
        Kind::Landing => samples.partition_point(|s| s[0] > v),
    };
    match (
        i.checked_sub(1).and_then(|j| samples.get(j)),
        samples.get(i),
    ) {
        (Some(a), Some(b)) if (b[0] - a[0]).abs() > 1e-12 => {
            let f = ((v - a[0]) / (b[0] - a[0])).clamp(0.0, 1.0);
            (a[1] + f * (b[1] - a[1]), a[2] + f * (b[2] - a[2]))
        }
        (_, Some(b)) => (b[1], b[2]),
        (Some(a), None) => (a[1], a[2]),
        (None, None) => (0.0, 0.0),
    }
}

/// The C.8 point-mass integral with the model's own forces (ADR-32): thrust `T(V)` of the engines
/// at sea level, lift and drag from the aero tables at the attitude and height the run flew (the
/// braking pitch-over of a landing moves load to the nose and takes lift away), rolling resistance
/// from the surface table, and for the landing the brakes at `0.9 mu_b` on the braked share of the
/// load. A 0.01 s Euler step. Returns the distance, m.
#[must_use]
pub fn point_mass_m(
    model: &AircraftModel,
    p: &EnvelopePoint,
    kind: Kind,
    env: &SphereSea,
    flown: &Run,
) -> Option<f64> {
    let st = start(model, p, kind, env)?;
    let s = st.state;
    let mass = model.mass.props(&s.fuel);
    let m = mass.mass_kg;
    let atmo = model.atmosphere().sample(0.0, NonStandardDay::ISA);
    let surf = env.surface(s.body.pos_bf);
    let fr = friction(surf.kind, surf.wetness);

    // The share of the load on wheels with brakes, from the static pose.
    let h_cg = env.height_msl_m(s.body.pos_bf) - env.ground_height_m(s.body.pos_bf);
    let loads = gear::forces(&model.gear, &s.body, &mass, &GearHeld::DOWN, env, h_cg);
    let total: f64 = loads.n_leg_n.iter().sum();
    let braked: f64 = model
        .gear
        .legs()
        .iter()
        .zip(&loads.n_leg_n)
        .filter(|(l, _)| l.braked)
        .map(|(_, n)| n)
        .sum();
    let share = if total > 0.0 { braked / total } else { 0.0 };
    let brake_scale = if model.gear.anti_skid {
        ANTI_SKID_FACTOR
    } else {
        1.0
    };
    let mu = match kind {
        Kind::TakeOff => fr.mu_roll,
        Kind::Landing => fr.mu_roll + brake_scale * fr.mu_brake * share,
    };

    let engines: Vec<_> = model
        .engines()
        .iter()
        .map(|e| (e, e.model.steady(st.engine)))
        .collect();
    let force_at = |v: f64| -> (f64, f64) {
        let (pitch, h_agl) = flown_at(&flown.samples, kind, v);
        let att = flight::frames::attitude(
            env.up_bf(s.body.pos_bf),
            flight::frames::Euler {
                pitch_rad: pitch,
                ..flight::frames::Euler::default()
            },
        );
        let vel_body = att.conjugate().rotate(DVec3::new(0.0, v, 0.0));
        let air = AirData::compute(
            vel_body,
            DVec3::ZERO,
            &atmo,
            model.aero.span_m(),
            model.aero.mac_m(),
        );
        let aero = model.aero.evaluate(&AeroInput {
            air,
            alpha_dot_rps: 0.0,
            surfaces: st.cfg,
            height_agl_m: h_agl,
            cg_body_m: mass.cg_body_m,
        });
        let cond = EngineConditions {
            rho_kgpm3: atmo.rho_kgpm3,
            t_k: atmo.t_k,
            mach: air.mach,
            alpha_rad: air.alpha_rad,
            tas_mps: v,
        };
        let thrust: f64 = engines
            .iter()
            .map(|(e, st)| e.model.thrust_n(st, &cond))
            .sum();
        let f_bf = att.rotate(aero.force_body_n + DVec3::new(thrust, 0.0, 0.0));
        (f_bf.y, f_bf.z)
    };

    let (mut v, mut x) = (st.v0_mps, 0.0);
    let mut t = 0.0;
    while t < 120.0 {
        let (along, up) = force_at(v);
        let n = (m * G0 - up).max(0.0);
        let sign = match kind {
            Kind::TakeOff => 1.0,
            Kind::Landing => -1.0,
        };
        // Friction opposes the motion; thrust and drag are in `along`.
        let a = (along - mu * n) / m;
        let v_next = v + a * POINT_MASS_DT_S;
        let reached = match kind {
            Kind::TakeOff => v_next >= st.v_end_mps,
            Kind::Landing => v_next <= st.v_end_mps,
        };
        if reached {
            let f = ((st.v_end_mps - v) / (v_next - v)).clamp(0.0, 1.0);
            return Some(x + f * v * POINT_MASS_DT_S);
        }
        if sign * (v_next - v) < 0.0 {
            // Never reaches the end speed (the thrust cannot overcome the drag, say).
            return None;
        }
        x += v * POINT_MASS_DT_S;
        v = v_next;
        t += POINT_MASS_DT_S;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use content::Registry;

    fn kestrel() -> (AircraftModel, content::AircraftDef) {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content/core");
        let baked = content_bake::bake::bake_dir(&dir).expect("the core pack bakes");
        let r = Registry::from_cpack(&baked.cpack).unwrap();
        let h = r.aircraft_handles().next().unwrap();
        let m = AircraftModel::from_registry(&r, h, TICK_DT_S).unwrap();
        (m, r.aircraft(h).unwrap().clone())
    }

    fn point(ac: &content::AircraftDef, id: &str) -> EnvelopePoint {
        ac.envelope.iter().find(|p| p.id == id).unwrap().clone()
    }

    #[test]
    fn the_stall_speeds_are_c8s() {
        // V_s = sqrt(2 m g / (rho0 S C_L,max)) with C_L,max = 1.60 + 0.30 (flaps). 10 500 kg with
        // the take-off flaps: sqrt(205 940 / (1.225 * 28 * 1.9)) = 56.21 m/s, so V_LOF 67.46.
        let (m, _) = kestrel();
        let tef = m.actuators.tef.max;
        let vs = stall_speed_mps(&m, 10_500.0, 0.5 * tef);
        assert!((vs - 56.21).abs() < 0.05, "{vs}");
        assert!((LOF_FACTOR * vs - 67.46).abs() < 0.06);
        // 8 500 kg with the landing flaps: 50.6 m/s, touchdown at 58.2.
        let vl = stall_speed_mps(&m, 8_500.0, tef);
        assert!((vl - 50.6).abs() < 0.1, "{vl}");
        assert!((TD_FACTOR * vl - 58.2).abs() < 0.1);
    }

    #[test]
    fn the_start_is_a_static_pose_with_the_lift_and_thrust_taken_off_the_struts() {
        let (m, ac) = kestrel();
        let env = world(0.0);
        let to = start(&m, &point(&ac, "to_roll"), Kind::TakeOff, &env).unwrap();
        let ldg = start(&m, &point(&ac, "ldg_roll"), Kind::Landing, &env).unwrap();
        assert_eq!(to.v0_mps, 0.0);
        assert!((to.v_end_mps - 67.5).abs() < 0.1);
        assert!((ldg.v0_mps - 58.2).abs() < 0.1 && ldg.v_end_mps == LDG_END_MPS);
        // About three degrees nose up on the gear.
        for st in [&to, &ldg] {
            let deg = st.pitch_rad.to_degrees();
            assert!((2.5..3.5).contains(&deg), "{deg}");
            assert!(st.state.mass_frozen);
            assert_eq!(st.state.surfaces, st.cfg);
        }
        assert_eq!(ldg.cfg.airbrake, 1.0, "C.7: brakes and airbrake");
        assert_eq!(to.cfg.airbrake, 0.0);
        // The struts are in balance at the start: no net vertical force, whatever the lift and
        // the vertical thrust take off them.
        for st in [&to, &ldg] {
            let tel = m.telemetry(&st.state, &env, 0.0);
            let net = st.state.body.att.rotate(tel.force_body_n).z - tel.mass_kg * G0;
            assert!(net.abs() < 5.0, "{net} N out of balance");
        }
    }

    #[test]
    fn the_validity_rules_name_what_went_wrong() {
        let (m, ac) = kestrel();
        let env = world(0.0);
        let st = start(&m, &point(&ac, "to_roll"), Kind::TakeOff, &env).unwrap();
        let mut s = st.state;
        s.ground.in_contact = [true; gear::MAX_SLOTS];
        s.ground.skid = false;
        let n = m.gear.legs().len();
        let ok = |s: &AircraftState, dev: f64, k: Kind| invalid_reason(&m, s, n, k, dev, 1.0);
        assert_eq!(ok(&s, 0.1, Kind::TakeOff), None);
        s.ground.in_contact[1] = false;
        assert!(ok(&s, 0.1, Kind::TakeOff).unwrap().contains("leg 1"));
        s.ground.in_contact[1] = true;
        s.ground.skid = true;
        assert!(ok(&s, 0.1, Kind::TakeOff).unwrap().contains("airframe"));
        s.ground.skid = false;
        s.body.pos_bf.x = 2.5;
        assert!(ok(&s, 0.1, Kind::TakeOff).unwrap().contains("centreline"));
        s.body.pos_bf.x = 0.0;
        assert!(ok(&s, 0.6, Kind::TakeOff).unwrap().contains("pitch"));
        // The landing allows the braking pitch-over (measured 1.7 degrees), not a tail strike.
        assert_eq!(ok(&s, 1.7, Kind::Landing), None);
        assert!(ok(&s, 3.5, Kind::Landing).is_some());
    }

    #[test]
    fn the_flown_attitude_is_interpolated_by_speed() {
        let up = [[0.0, 0.05, 1.5], [10.0, 0.04, 1.4], [20.0, 0.03, 1.3]];
        let (p, h) = flown_at(&up, Kind::TakeOff, 15.0);
        assert!((p - 0.035).abs() < 1e-12 && (h - 1.35).abs() < 1e-12);
        let down = [[20.0, 0.03, 1.3], [10.0, 0.04, 1.4], [0.0, 0.05, 1.5]];
        let (p, h) = flown_at(&down, Kind::Landing, 15.0);
        assert!((p - 0.035).abs() < 1e-12 && (h - 1.35).abs() < 1e-12);
        // Outside the samples it holds the nearest.
        assert_eq!(flown_at(&up, Kind::TakeOff, 99.0).0, 0.03);
        assert_eq!(flown_at(&up, Kind::TakeOff, -1.0).0, 0.05);
        assert_eq!(flown_at(&[], Kind::TakeOff, 1.0), (0.0, 0.0));
    }

    #[test]
    fn the_rolls_are_the_envelope_configurations_and_nothing_else() {
        assert_eq!(Kind::of(EnvelopeConfig::ToFlapsAb), Some(Kind::TakeOff));
        assert_eq!(Kind::of(EnvelopeConfig::LdgBrakes), Some(Kind::Landing));
        assert_eq!(Kind::of(EnvelopeConfig::Clean), None);
        assert_eq!(Kind::TakeOff.band_pct(), 2.0);
        assert_eq!(Kind::Landing.band_pct(), 3.0);
        assert!(Kind::TakeOff.method().contains("V_LOF"));
        assert!(Kind::Landing.method().contains("brakes"));
    }

    #[test]
    fn the_point_mass_and_the_flown_roll_agree_and_the_energy_closes() {
        let (m, ac) = kestrel();
        let env = world(0.0);
        for (id, kind, band) in [
            ("to_roll", Kind::TakeOff, TO_BAND_PCT),
            ("ldg_roll", Kind::Landing, LDG_BAND_PCT),
        ] {
            let p = point(&ac, id);
            let run = fly(&m, &p, kind, &env).unwrap();
            assert!(
                run.invalid.is_none() && !run.timed_out && run.faults == 0,
                "{id}"
            );
            let pm = point_mass_m(&m, &p, kind, &env, &run).unwrap();
            let delta = (run.distance_m - pm) / pm * 100.0;
            let energy = (run.ke_gain_j - run.work_j) / run.work_j * 100.0;
            println!(
                "{id}: {:.2} m, point mass {pm:.2} m ({delta:+.2} %), energy {energy:+.3} %",
                run.distance_m
            );
            assert!(delta.abs() < band, "{id}: {delta}");
            assert!(energy.abs() < ENERGY_BAND_PCT, "{id}: {energy}");
            // The trace is the ground columns and ends at the end speed.
            assert_eq!(run.trace.columns, &GROUND_COLUMNS);
            assert!(run.trace.csv().starts_with("t_s,dist_m,lateral_m,gs_mps,"));
        }
    }
}
