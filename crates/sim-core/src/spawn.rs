//! Where and how the player's aircraft starts: trimmed in level flight (ADR-29).

use flight::actuators::Surfaces;
use flight::aircraft::AircraftModel;
use flight::frames::{Euler, attitude};
use flight::trim::{TrimPoint, TrimProblem, TrimSetup, guess_level, solve, state_of};

use crate::math::DVec3;
use crate::ownship::AircraftSlot;
use crate::services::Services;

/// Every number that shapes the spawn, in one place.
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
    /// Fuel as a fraction of capacity.
    pub fuel_fraction: f64,
    /// Angle of attack if the trim fails, rad.
    pub fallback_alpha_rad: f64,
    /// Throttle `tau` if the trim fails.
    pub fallback_throttle: f64,
}

/// The spawn in force: 6 km south of the patch centre, 2 000 m above the terrain, heading north
/// at 200 m/s with 60 % fuel.
pub const SPAWN: SpawnTuning = SpawnTuning {
    south_m: 6_000.0,
    agl_m: 2_000.0,
    heading_rad: 0.0,
    tas_mps: 200.0,
    fuel_fraction: 0.6,
    fallback_alpha_rad: 2.0 * core::f64::consts::PI / 180.0,
    fallback_throttle: 0.6,
};

/// Resets `slot` in place to a trimmed level spawn. Allocates nothing. Returns false if the trim
/// solver failed and the fallback attitude and throttle were used instead.
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
        .load_for(tuning.fuel_fraction * model.mass.capacity_kg());
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
    slot.crashed = false;
    slot.respawn_ticks = 0;
    ok
}
