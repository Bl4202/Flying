//! Where and how the player's aircraft starts: lined up on the runway, or trimmed in level flight
//! above the airfield (ADR-29, ADR-36).

use flight::actuators::Surfaces;
use flight::aircraft::AircraftModel;
use flight::engine::EngineCommand;
use flight::frames::{Euler, attitude};
use flight::trim::{TrimPoint, TrimProblem, TrimSetup, guess_level, solve, state_of};

use crate::layout::{airfield, crash_cause};
use crate::math::DVec3;
use crate::ownship::AircraftSlot;
use crate::services::Services;
use crate::session::Spawn;

/// The throttle lever's military detent (`flight::engine`).
const MILITARY_DETENT: f64 = 0.8;

/// The flap lever positions as a fraction of the flap travel: up, take-off (half), landing (full).
pub const FLAP_DETENT_FRACTION: [f64; 3] = [0.0, 0.5, 1.0];

/// Every number that shapes the spawn in the air, in one place.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpawnTuning {
    /// Distance south of the terrain patch centre, m.
    pub south_m: f64,
    /// Height above the terrain there, m.
    pub agl_m: f64,
    /// Heading, rad (0 is north).
    pub heading_rad: f64,
    /// True airspeed, m/s.
    pub tas_mps: f64,
    /// Angle of attack if the trim fails, rad.
    pub fallback_alpha_rad: f64,
    /// Throttle `tau` if the trim fails.
    pub fallback_throttle: f64,
}

/// The air spawn: 6 km south of the patch centre (the runway threshold), 2 000 m above the
/// ground there, heading north at 200 m/s, gear and flaps up (the fuel is the slot's).
pub const AIR_SPAWN: SpawnTuning = SpawnTuning {
    south_m: airfield::THRESHOLD_SOUTH_M as f64,
    agl_m: 2_000.0,
    heading_rad: 0.0,
    tas_mps: 200.0,
    fallback_alpha_rad: 2.0 * core::f64::consts::PI / 180.0,
    fallback_throttle: 0.6,
};

/// Every number that shapes the spawn on the runway.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RunwayTuning {
    /// Distance from the threshold along the centreline, m.
    pub along_m: f64,
    /// Heading, rad: the runway's (000).
    pub heading_rad: f64,
    /// Flap lever: 1 is the take-off detent.
    pub flap_detent: u8,
}

/// The runway spawn (GDD 48.2 "runway hot"): 150 m past the threshold on the centreline, nose
/// north, engine at idle, parking brake on, gear down, take-off flaps (the fuel is the slot's).
pub const RUNWAY_SPAWN: RunwayTuning = RunwayTuning {
    along_m: 150.0,
    heading_rad: 0.0,
    flap_detent: 1,
};

/// The throttle lever that holds the engine setting `tau` (0 idle, 1 military, then the
/// afterburner stages): the inverse of the engine's lever mapping on the dry range.
#[must_use]
pub fn lever_for_tau(tau: f64) -> f64 {
    if tau <= 1.0 {
        tau.max(0.0) * MILITARY_DETENT
    } else {
        (MILITARY_DETENT + (tau - 1.0).min(1.0) * (1.0 - MILITARY_DETENT)).min(1.0)
    }
}

/// Resets `slot` in place to a new life at `kind`, under the assist level the control system is
/// at now. Allocates nothing. Returns false if the trim or the static pose could not be solved
/// and a fallback was used instead.
pub fn spawn(model: &AircraftModel, svc: &Services, kind: Spawn, slot: &mut AircraftSlot) -> bool {
    match kind {
        Spawn::Runway if !model.gear.legs().is_empty() => {
            // A runway that cannot be had is an air spawn, and counts as a failed spawn.
            spawn_runway(model, svc, &RUNWAY_SPAWN, slot) || {
                spawn_trimmed(model, svc, &AIR_SPAWN, slot);
                false
            }
        }
        // Without gear there is nothing to roll on.
        _ => spawn_trimmed(model, svc, &AIR_SPAWN, slot),
    }
}

/// What every new life starts from, whatever the spawn: the pilot rested, the airframe
/// unstressed, nothing pending. The counter that tells the host to hand the throttle over moves.
fn new_life(slot: &mut AircraftSlot) {
    slot.crashed = false;
    slot.respawn_ticks = 0;
    slot.crash_cause = crash_cause::NONE as u8;
    slot.physio = flight::physio::PhysioState::REST;
    slot.effects = flight::physio::PhysioEffects::NONE;
    slot.structure = flight::structure::StructureState::NEW;
    slot.warnings = flight::structure::StructureFlags::default();
    slot.stick_ring = [[0.0; 3]; flight::physio::MAX_DELAY_TICKS];
    slot.ring_head = 0;
    slot.brake_cmd = [0.0; 2];
    slot.aim = None;
    slot.events = flight::gear::GroundEvents::NONE;
    slot.spawn_seq = slot.spawn_seq.wrapping_add(1);
}

/// The control system for a new life: the assist level the player has, the ground or the flight
/// law, and the surfaces where they are so nothing steps.
fn new_fcs(slot: &mut AircraftSlot, on_ground: bool) {
    let s = &slot.state.surfaces;
    let mut fs = fcs::FcsState::new(slot.fcs.assist, on_ground);
    fs.last_cmd = [s.elevator_rad, s.aileron_rad, s.rudder_rad];
    slot.fcs = fs;
}

/// Lines the aircraft up on the runway at rest on its gear, engine idling, parking brake on, gear
/// down, take-off flaps; the throttle is held at idle until the host acknowledges. False if the
/// static pose does not exist (no gear or load the struts cannot carry), with `slot` untouched.
fn spawn_runway(
    model: &AircraftModel,
    svc: &Services,
    t: &RunwayTuning,
    slot: &mut AircraftSlot,
) -> bool {
    let env = svc.flight_env();
    let (x, y) = (0.0, -(airfield::THRESHOLD_SOUTH_M as f64) + t.along_m);
    let ground = DVec3::new(x, y, svc.geo.ground_z_bf(x, y));
    let fuel_kg = slot.spawn_fuel * model.mass.capacity_kg();
    let Some((mut state, pose)) = model.static_state(
        fuel_kg,
        0.0,
        t.heading_rad,
        ground,
        &env,
        EngineCommand::IDLE,
    ) else {
        return false;
    };
    let detent = usize::from(t.flap_detent.min(2));
    state.surfaces.tef_rad = FLAP_DETENT_FRACTION[detent] * model.actuators.tef.max;
    // At rest on the struts: in contact and on the wheels, so the first tick is no touchdown.
    for (c, &x) in state.ground.in_contact.iter_mut().zip(&pose.compression_m) {
        *c = x > 0.0;
    }
    state.ground.wow = true;
    state.ground.air_time_s = 0.0;
    slot.state = state;
    new_life(slot);
    slot.gear_lever_down = true;
    slot.flap_detent = t.flap_detent.min(2);
    slot.parking_brake = true;
    slot.throttle_hold = Some(0.0);
    slot.throttle = 0.0;
    new_fcs(slot, true);
    true
}

/// Resets `slot` in place to a trimmed level spawn, gear and flaps up; the throttle is held at the
/// trim setting until the host acknowledges. Allocates nothing. Returns false if the trim solver
/// failed and the fallback attitude and throttle were used instead.
pub fn spawn_trimmed(
    model: &AircraftModel,
    svc: &Services,
    tuning: &SpawnTuning,
    slot: &mut AircraftSlot,
) -> bool {
    let env = svc.flight_env();
    let (x, y) = (0.0, -tuning.south_m);
    let ref_bf = DVec3::new(x, y, svc.geo.ground_z_bf(x, y) + tuning.agl_m);
    let fuel = model
        .mass
        .load_for(slot.spawn_fuel * model.mass.capacity_kg());
    let level = attitude(
        svc.geo.up_bf(ref_bf),
        Euler {
            heading_rad: tuning.heading_rad,
            ..Euler::default()
        },
    );
    let setup = TrimSetup {
        pos_bf: model.cg_for_ref_point(ref_bf, &fuel, level),
        fuel,
        config: Surfaces::NEUTRAL,
    };
    let mut guess = guess_level(model, &setup, tuning.tas_mps, 1.0, &env);
    guess.heading_rad = tuning.heading_rad;
    let trim = solve(model, &TrimProblem::level(), &setup, guess, &env);
    let ok = trim.converged;
    let point = if ok {
        trim.point
    } else {
        TrimPoint {
            speed_mps: tuning.tas_mps,
            heading_rad: tuning.heading_rad,
            alpha_rad: tuning.fallback_alpha_rad,
            throttle: tuning.fallback_throttle,
            ..TrimPoint::default()
        }
    };
    slot.state = state_of(model, &setup, &point, &env);
    new_life(slot);
    slot.gear_lever_down = false;
    slot.flap_detent = 0;
    slot.parking_brake = false;
    let lever = lever_for_tau(point.throttle);
    slot.throttle_hold = Some(lever);
    slot.throttle = lever;
    new_fcs(slot, false);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_lever_that_holds_a_throttle_setting_is_the_engines_inverse() {
        use flight::engine::EngineCommand;
        for tau in [0.0, 0.25, 0.5, 0.9, 1.0] {
            let cmd = EngineCommand::from_lever(lever_for_tau(tau), 4);
            assert!((cmd.dry - tau).abs() < 1e-6, "tau {tau}: {cmd:?}");
            assert_eq!(cmd.ab_stage, 0);
        }
        assert_eq!(lever_for_tau(-1.0), 0.0);
        assert_eq!(lever_for_tau(2.0), 1.0);
        assert!(EngineCommand::from_lever(lever_for_tau(1.5), 4).ab_stage > 0);
    }

    #[test]
    fn the_flap_detents_run_from_up_to_the_full_travel() {
        assert_eq!(FLAP_DETENT_FRACTION, [0.0, 0.5, 1.0]);
    }
}
