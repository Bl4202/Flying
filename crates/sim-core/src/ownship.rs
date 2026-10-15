//! The player's ownship: the 6-DOF aircraft when content provides one, otherwise the kinematic
//! spectator of v0.0b (ADR-29).

use flight::aircraft::{AircraftModel, AircraftState};
use flight::env::FlightEnv;

use crate::hash::StateHasher;
use crate::input::InputState;
use crate::layout::ownship_flag;
use crate::math::{DQuat, DVec3};
use crate::observer::Observer;
use crate::services::Services;

/// Ticks from a crash or a numerical fault to the respawn (2 s).
pub const RESPAWN_TICKS: u32 = 120;
/// The aircraft has crashed when its reference point is lower than this above the terrain, m.
/// There is no gear until v0.1b, so any contact is a crash.
pub const CRASH_HEIGHT_M: f64 = 1.5;

/// The player's aircraft around the flight model's state: its life cycle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AircraftSlot {
    /// The flight model's state.
    pub state: AircraftState,
    /// Hit the ground; frozen until it respawns.
    pub crashed: bool,
    /// Ticks left before the respawn while crashed or faulted, 0 otherwise.
    pub respawn_ticks: u32,
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
        }
    }
}

impl OwnshipView {
    /// The view of the ownship after a tick.
    #[must_use]
    pub fn of(ownship: &Ownship, svc: &Services, input: &InputState, t_s: f64) -> Self {
        let throttle = input.throttle_cmd();
        match (ownship, svc.aircraft.as_ref()) {
            (Ownship::Aircraft(a), Some(model)) => aircraft_view(a, model, svc, throttle, t_s),
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

fn aircraft_view(
    a: &AircraftSlot,
    model: &AircraftModel,
    svc: &Services,
    throttle: f64,
    t_s: f64,
) -> OwnshipView {
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
        throttle,
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
    }
}
