//! The player's ownship: the 6-DOF aircraft when content provides one, otherwise the kinematic
//! spectator of v0.0b (ADR-29).

use flight::aircraft::{AircraftModel, AircraftState};
use flight::env::FlightEnv;
use flight::gear::{GEAR_DOWN_MIN, GroundEvents};
use flight::physio::{MAX_DELAY_TICKS, PhysioEffects, PhysioState};
use flight::structure::{StructureFlags, StructureState};

use crate::hash::StateHasher;
use crate::layout::{crash_cause, fcs_mode, ownship_flag, ownship_flag2};
use crate::math::{DQuat, DVec3};
use crate::observer::Observer;
use crate::services::Services;
use crate::session::{GEffects, SessionConfig};

/// Ticks from a crash or a numerical fault to the respawn (2 s).
pub const RESPAWN_TICKS: u32 = 120;
/// The gear warning shows below this height above the ground, m. // ALGO: a landing reminder
pub const GEAR_WARNING_AGL_M: f64 = 150.0;
/// ... below this calibrated airspeed, m/s ...
pub const GEAR_WARNING_CAS_MPS: f64 = 100.0;
/// ... with the throttle lever below this, and the gear up.
pub const GEAR_WARNING_THROTTLE: f64 = 0.5;
/// The red-out flag is set from this red-out stress. // ALGO: where the tint shows (physio.rs)
pub const REDOUT_FLAG_AT: f64 = 0.5;

/// The player's aircraft around the flight model's state: its life cycle, levers and pilot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AircraftSlot {
    /// The flight model's state.
    pub state: AircraftState,
    /// Hit the ground; frozen until it respawns.
    pub crashed: bool,
    /// Ticks left before the respawn while crashed or faulted, 0 otherwise.
    pub respawn_ticks: u32,
    /// Why it was destroyed (`crash_cause`; 0 while it is not).
    pub crash_cause: u8,
    /// The session the aircraft was started under.
    pub session: SessionConfig,
    /// The control system's memory (assist level included).
    pub fcs: fcs::FcsState,
    /// The pilot's physiology.
    pub physio: PhysioState,
    /// What the physiology does to the controls and the picture, as of the last tick.
    pub effects: PhysioEffects,
    /// The airframe's stress.
    pub structure: StructureState,
    /// What the structure reported at the last tick.
    pub warnings: StructureFlags,
    /// The pilot's last stick positions (pitch, roll, yaw), a ring the blackout reads from.
    pub stick_ring: [[f64; 3]; MAX_DELAY_TICKS],
    /// Where the next entry of the ring goes.
    pub ring_head: u8,
    /// The gear lever is down.
    pub gear_lever_down: bool,
    /// The flap lever: 0 up, 1 take-off, 2 landing.
    pub flap_detent: u8,
    /// The parking brake is set.
    pub parking_brake: bool,
    /// The wheel brakes as commanded at the last tick (left, right), parking brake included.
    pub brake_cmd: [f64; 2],
    /// Spawns so far, wrapping.
    pub spawn_seq: u8,
    /// The throttle lever is held here until the host acknowledges the spawn.
    pub throttle_hold: Option<f64>,
    /// The throttle lever in force at the last tick.
    pub throttle: f64,
    /// The mouse-aim direction in use at the last tick (azimuth, elevation, rad).
    pub aim: Option<[f64; 2]>,
    /// What the ground did in this tick's dynamics, for the contact stage. Empty between ticks.
    pub events: GroundEvents,
}

impl AircraftSlot {
    /// A slot holding `state`, not yet spawned: the control system on the ground at the session's
    /// assist level, everything at rest.
    #[must_use]
    pub fn new(state: AircraftState, session: SessionConfig) -> Self {
        Self {
            state,
            crashed: false,
            respawn_ticks: 0,
            crash_cause: crash_cause::NONE as u8,
            session,
            fcs: fcs::FcsState::new(session.assist.to_fcs(), false),
            physio: PhysioState::REST,
            effects: PhysioEffects::NONE,
            structure: StructureState::NEW,
            warnings: StructureFlags::default(),
            stick_ring: [[0.0; 3]; MAX_DELAY_TICKS],
            ring_head: 0,
            gear_lever_down: false,
            flap_detent: 0,
            parking_brake: false,
            brake_cmd: [0.0; 2],
            spawn_seq: 0,
            throttle_hold: None,
            throttle: 0.0,
            aim: None,
            events: GroundEvents::NONE,
        }
    }
}

/// What the player flies.
#[derive(Clone, Debug)]
pub enum Ownship {
    /// No aircraft content: the kinematic observer.
    Spectator(Observer),
    /// The aircraft. Boxed: allocated once when the core starts, then reset in place.
    Aircraft(Box<AircraftSlot>),
}

impl Ownship {
    /// The spectator, if that is what the player has.
    #[must_use]
    pub fn spectator(&self) -> Option<&Observer> {
        match self {
            Self::Spectator(o) => Some(o),
            Self::Aircraft(_) => None,
        }
    }

    /// The aircraft, if that is what the player has.
    #[must_use]
    pub fn aircraft(&self) -> Option<&AircraftSlot> {
        match self {
            Self::Spectator(_) => None,
            Self::Aircraft(a) => Some(a),
        }
    }

    /// Feeds the state into `h`. The spectator hashes exactly as the v0.0b observer did.
    pub fn hash_into(&self, h: &mut StateHasher) {
        match self {
            Self::Spectator(o) => o.hash_into(h),
            Self::Aircraft(a) => hash_aircraft(a, h),
        }
    }
}

/// Every integrated number of the aircraft, in a fixed order. Derived quantities (air data,
/// forces, mass properties) are not state and are not hashed.
fn hash_aircraft(a: &AircraftSlot, h: &mut StateHasher) {
    // ALGO: a tag so an aircraft never hashes like a spectator ("AIRCRAFT" in ASCII).
    h.write_u64(0x5446_4152_4352_4941);
    let s = &a.state;
    let b = &s.body;
    h.write_f64s(&b.pos_bf.to_array());
    h.write_f64s(&b.vel_bf.to_array());
    h.write_f64s(&b.att.to_array());
    h.write_f64s(&b.omega_body.to_array());
    let f = &s.surfaces;
    h.write_f64s(&[
        f.elevator_rad,
        f.aileron_rad,
        f.rudder_rad,
        f.lef_rad,
        f.tef_rad,
        f.airbrake,
        f.gear,
    ]);
    for e in &s.engines {
        h.write_f64s(&[e.core, e.ab_level, e.ab_wait_s, e.relight_s]);
        h.write_u64(
            u64::from(e.ab_lit)
                | (u64::from(e.running) << 1)
                | (u64::from(e.compressor_stalled) << 2),
        );
    }
    h.write_f64s(&s.fuel);
    h.write_f64(s.trim_pitch_rad);
    h.write_u64(
        u64::from(s.mass_frozen)
            | (u64::from(s.numerical_fault) << 1)
            | (u64::from(a.crashed) << 2)
            | (u64::from(a.respawn_ticks) << 32),
    );
    hash_ground(a, h);
    hash_pilot(a, h);
}

/// The ground model's memory and the wheels as applied.
fn hash_ground(a: &AircraftSlot, h: &mut StateHasher) {
    let g = &a.state.ground;
    let mut bits = u64::from(g.wow) | u64::from(g.skid) << 1;
    for (i, &c) in g.collapsed.iter().enumerate() {
        bits |= u64::from(c) << (2 + i);
    }
    for (i, &c) in g.in_contact.iter().enumerate() {
        bits |= u64::from(c) << (8 + i);
    }
    h.write_u64(bits);
    h.write_f64s(&[
        g.air_time_s,
        g.touchdown_vs_mps,
        g.hard_landing_s,
        g.steer_rad,
    ]);
    let w = &a.state.wheels;
    h.write_f64s(&w.brake);
    h.write_f64(w.steer_rad);
}

/// The control system, the pilot's body, the levers and the session.
fn hash_pilot(a: &AircraftSlot, h: &mut StateHasher) {
    for w in a.fcs.words() {
        h.write_u64(w);
    }
    let p = &a.physio;
    h.write_f64s(&[
        p.g_f,
        p.stress,
        p.fatigue,
        p.redout,
        p.gloc_s,
        p.disorient_s,
    ]);
    let e = &a.effects;
    h.write_u64(u64::from(e.blackout) | u64::from(e.gloc) << 1 | u64::from(e.gloc_started) << 2);
    h.write_u64(e.delay_ticks as u64);
    h.write_f64s(&[e.control_scale, e.grey, e.black, e.red]);
    let st = &a.structure;
    h.write_f64s(&[st.overstress_s, st.max_n_g, st.overspeed_s]);
    let w = &a.warnings;
    h.write_u64(
        u64::from(w.overstress)
            | u64::from(w.overspeed) << 1
            | u64::from(w.gear_overspeed) << 2
            | u64::from(w.flap_overspeed) << 3,
    );
    for row in &a.stick_ring {
        h.write_f64s(row);
    }
    h.write_u64(
        u64::from(a.ring_head)
            | u64::from(a.gear_lever_down) << 8
            | u64::from(a.flap_detent) << 9
            | u64::from(a.parking_brake) << 11
            | u64::from(a.spawn_seq) << 12
            | u64::from(a.throttle_hold.is_some()) << 20
            | u64::from(a.crash_cause) << 21
            | u64::from(a.aim.is_some()) << 29,
    );
    h.write_f64s(&a.brake_cmd);
    h.write_f64s(&[a.throttle_hold.unwrap_or(0.0), a.throttle]);
    h.write_f64s(&a.aim.unwrap_or([0.0; 2]));
    h.write_u64(a.session.hash_word());
}

/// The ownship as the snapshot shows it (layout `snapshot::O_*`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OwnshipView {
    /// Reference point (aircraft) or position (spectator), BF.
    pub pos_bf: DVec3,
    /// Attitude, body to BF.
    pub att: DQuat,
    /// Velocity, BF.
    pub vel_bf: DVec3,
    /// True airspeed, m/s.
    pub tas_mps: f64,
    /// Calibrated airspeed, m/s.
    pub cas_mps: f64,
    /// Mach.
    pub mach: f64,
    /// Angle of attack, rad.
    pub alpha_rad: f64,
    /// Sideslip, rad.
    pub beta_rad: f64,
    /// Body-axis load factor, g.
    pub nz_g: f64,
    /// Throttle lever, 0 to 1.
    pub throttle: f64,
    /// Thrust, N.
    pub thrust_n: f64,
    /// Fuel, kg.
    pub fuel_kg: f64,
    /// Mass, kg.
    pub mass_kg: f64,
    /// Body rates, rad/s.
    pub omega_body: DVec3,
    /// Elevator, aileron, rudder, rad.
    pub surfaces_rad: [f64; 3],
    /// Height above MSL, m.
    pub altitude_m: f64,
    /// Afterburner stages burning.
    pub ab_stage: u8,
    /// `ownship_flag` bits.
    pub flags: u8,
    /// Assist level in force (`assist_level`; 0 for the spectator).
    pub assist: u8,
    /// What the flight control system is doing (`fcs_mode`).
    pub fcs_mode: u8,
    /// `ownship_flag2` bits.
    pub flags2: u16,
    /// Spawn counter, wrapping.
    pub spawn_seq: u8,
    /// Flap detent: 0 up, 1 take-off, 2 landing.
    pub flap_detent: u8,
    /// Gear position, 0 up to 1 down.
    pub gear_pos: f64,
    /// Trailing-edge flap, rad.
    pub tef_rad: f64,
    /// Airbrake, 0 to 1.
    pub airbrake: f64,
    /// Left and right wheel brake commands, 0 to 1.
    pub brake: [f64; 2],
    /// Nose-wheel steering angle, rad.
    pub steer_rad: f64,
    /// Speed over the ground, projected on the local horizontal, m/s.
    pub ground_speed_mps: f64,
    /// Vertical speed (climb positive), m/s.
    pub vs_mps: f64,
    /// Height of the lowest wheel (or the reference point without gear) above the ground, m.
    pub agl_m: f64,
    /// Lagged g felt by the pilot ("heart-level" G), g.
    pub g_lagged: f64,
    /// G stress `S` (0.35 greyout, 0.7 blackout, 1.0 G-LOC).
    pub g_stress: f64,
    /// Red-out stress `R`.
    pub redout: f64,
    /// Load-factor command of the flight control system, g.
    pub n_cmd: f64,
    /// Sink rate at the last touchdown, m/s.
    pub touchdown_vs_mps: f64,
    /// Mouse-aim direction in use: azimuth and elevation, rad.
    pub aim_rad: [f64; 2],
    /// Why the aircraft was destroyed (`crash_cause`).
    pub crash_cause: u8,
    /// Gear bits (see the snapshot layout).
    pub gear_bits: u8,
}

impl Default for OwnshipView {
    fn default() -> Self {
        Self {
            pos_bf: DVec3::ZERO,
            att: DQuat::IDENTITY,
            vel_bf: DVec3::ZERO,
            tas_mps: 0.0,
            cas_mps: 0.0,
            mach: 0.0,
            alpha_rad: 0.0,
            beta_rad: 0.0,
            nz_g: 0.0,
            throttle: 0.0,
            thrust_n: 0.0,
            fuel_kg: 0.0,
            mass_kg: 0.0,
            omega_body: DVec3::ZERO,
            surfaces_rad: [0.0; 3],
            altitude_m: 0.0,
            ab_stage: 0,
            flags: 0,
            assist: 0,
            fcs_mode: fcs_mode::FLIGHT as u8,
            flags2: 0,
            spawn_seq: 0,
            flap_detent: 0,
            gear_pos: 0.0,
            tef_rad: 0.0,
            airbrake: 0.0,
            brake: [0.0; 2],
            steer_rad: 0.0,
            ground_speed_mps: 0.0,
            vs_mps: 0.0,
            agl_m: 0.0,
            g_lagged: 0.0,
            g_stress: 0.0,
            redout: 0.0,
            n_cmd: 0.0,
            touchdown_vs_mps: 0.0,
            aim_rad: [0.0; 2],
            crash_cause: crash_cause::NONE as u8,
            gear_bits: 0,
        }
    }
}

impl OwnshipView {
    /// The view of the ownship after a tick.
    #[must_use]
    pub fn of(ownship: &Ownship, svc: &Services, t_s: f64) -> Self {
        match (ownship, svc.aircraft.as_ref()) {
            (Ownship::Aircraft(a), Some(model)) => aircraft_view(a, model, svc, t_s),
            (Ownship::Spectator(o), _) => Self {
                pos_bf: o.pos_bf(),
                att: o.attitude(),
                vel_bf: o.vel_bf(),
                tas_mps: o.speed_mps(),
                throttle: o.throttle(),
                altitude_m: svc.geo.height_msl_m(o.pos_bf()),
                flags: ownship_flag::SPECTATOR as u8,
                ..Self::default()
            },
            // An aircraft without a model cannot happen (the model is built before the slot).
            (Ownship::Aircraft(_), None) => Self::default(),
        }
    }
}

/// `ownship_flag2` bits of an aircraft and its pilot.
fn flags2(a: &AircraftSlot, t: &flight::aircraft::Telemetry) -> u16 {
    let mut f = 0;
    let mut set = |on: bool, bit: usize| {
        if on {
            f |= bit as u16;
        }
    };
    let lim = &a.fcs.limiting;
    set(a.state.ground.wow, ownship_flag2::WOW);
    set(a.parking_brake, ownship_flag2::PARKING_BRAKE);
    set(lim.alpha, ownship_flag2::ALPHA_LIMIT);
    set(lim.g, ownship_flag2::G_LIMIT);
    set(lim.overspeed, ownship_flag2::OVERSPEED);
    set(a.warnings.overstress, ownship_flag2::OVERSTRESS);
    set(a.effects.blackout, ownship_flag2::BLACKOUT);
    set(a.effects.gloc, ownship_flag2::GLOC);
    set(a.effects.red >= REDOUT_FLAG_AT, ownship_flag2::REDOUT);
    set(
        a.state.ground.hard_landing_s > 0.0,
        ownship_flag2::HARD_LANDING,
    );
    set(a.warnings.gear_overspeed, ownship_flag2::GEAR_OVERSPEED);
    set(a.warnings.flap_overspeed, ownship_flag2::FLAP_OVERSPEED);
    set(a.fcs.departed, ownship_flag2::DEPARTED);
    set(
        !a.crashed
            && !t.wow
            && !a.gear_lever_down
            && t.agl_m < GEAR_WARNING_AGL_M
            && t.cas_mps < GEAR_WARNING_CAS_MPS
            && a.throttle < GEAR_WARNING_THROTTLE,
        ownship_flag2::GEAR_WARNING,
    );
    set(a.aim.is_some(), ownship_flag2::MOUSE_AIM);
    set(a.throttle_hold.is_some(), ownship_flag2::THROTTLE_HELD);
    f
}

/// Snapshot `O_GEAR_BITS`: per leg `i` (the first three) bit `2i` weight on wheels and `2i + 1`
/// collapsed, bit 6 the lever down, bit 7 all down and locked.
fn gear_bits(a: &AircraftSlot, model: &AircraftModel) -> u8 {
    let g = &a.state.ground;
    let legs = model.gear.legs().len().min(3);
    let mut bits = 0_u8;
    for i in 0..legs {
        bits |= u8::from(g.in_contact[i]) << (2 * i);
        bits |= u8::from(g.collapsed[i]) << (2 * i + 1);
    }
    bits |= u8::from(a.gear_lever_down) << 6;
    bits |= u8::from(a.gear_lever_down && a.state.surfaces.gear >= GEAR_DOWN_MIN) << 7;
    bits
}

fn aircraft_view(a: &AircraftSlot, model: &AircraftModel, svc: &Services, t_s: f64) -> OwnshipView {
    let env = svc.flight_env();
    let s = &a.state;
    let t = model.telemetry(s, &env, t_s);
    let mut flags = 0;
    if a.crashed {
        flags |= ownship_flag::CRASHED;
    }
    if t.flameout {
        flags |= ownship_flag::FLAMEOUT;
    }
    if t.stall_warning && !a.crashed {
        flags |= ownship_flag::STALL_WARN;
    }
    if s.numerical_fault {
        flags |= ownship_flag::NUMERICAL_FAULT;
    }
    let cause = if s.numerical_fault {
        crash_cause::NUMERICAL as u8
    } else if a.crashed {
        a.crash_cause
    } else {
        crash_cause::NONE as u8
    };
    // What the picture shows: nothing with the dial off (the pilot's state still advances).
    let shown = a.session.g_effects != GEffects::Off;
    OwnshipView {
        pos_bf: t.ref_point_bf,
        att: s.body.att,
        vel_bf: s.body.vel_bf,
        tas_mps: t.air.tas_mps,
        cas_mps: t.cas_mps,
        mach: t.air.mach,
        alpha_rad: t.air.alpha_rad,
        beta_rad: t.air.beta_rad,
        nz_g: t.nz_g,
        throttle: a.throttle_hold.unwrap_or(a.throttle),
        thrust_n: t.thrust_n,
        fuel_kg: t.fuel_kg,
        mass_kg: t.mass_kg,
        omega_body: s.body.omega_body,
        surfaces_rad: [
            s.surfaces.elevator_rad,
            s.surfaces.aileron_rad,
            s.surfaces.rudder_rad,
        ],
        altitude_m: env.height_msl_m(t.ref_point_bf),
        ab_stage: t.ab_stage,
        flags: flags as u8,
        assist: a.fcs.assist.code(),
        fcs_mode: a.fcs.mode as u8,
        flags2: flags2(a, &t),
        spawn_seq: a.spawn_seq,
        flap_detent: a.flap_detent,
        gear_pos: s.surfaces.gear,
        tef_rad: s.surfaces.tef_rad,
        airbrake: s.surfaces.airbrake,
        brake: a.brake_cmd,
        steer_rad: s.ground.steer_rad,
        ground_speed_mps: t.ground_speed_mps,
        vs_mps: t.vs_mps,
        agl_m: t.agl_m,
        g_lagged: a.physio.g_f,
        g_stress: if shown { a.physio.stress } else { 0.0 },
        redout: if shown { a.physio.redout } else { 0.0 },
        n_cmd: a.fcs.n_cmd,
        touchdown_vs_mps: s.ground.touchdown_vs_mps,
        aim_rad: a.aim.unwrap_or([0.0; 2]),
        crash_cause: cause,
        gear_bits: gear_bits(a, model),
    }
}
