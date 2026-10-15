//! Sea-level static fuel flow (ADR-31): the engines alone on a bench at M 0, h 0, ISA, started
//! at idle, read 15 s after the throttle is set. Cross-check: the steady-state flow of the same
//! model, which is TSFC times thrust.

use flight::aircraft::AircraftModel;
use flight::atmosphere::NonStandardDay;
use flight::engine::{EngineCommand, EngineConditions, EngineModel};

/// Time from idle to the reading, s. // ALGO: ADR-31
pub const READ_AT_S: f64 = 15.0;
/// Cross-check band, percent. // ALGO: v0.1a (ADR-31 names the check, not the band)
pub const BAND_PCT: f64 = 0.1;

/// Sea-level static conditions.
fn bench(model: &AircraftModel) -> EngineConditions {
    let air = model.atmosphere().sample(0.0, NonStandardDay::ISA);
    EngineConditions {
        rho_kgpm3: air.rho_kgpm3,
        t_k: air.t_k,
        mach: 0.0,
        alpha_rad: 0.0,
        tas_mps: 0.0,
    }
}

/// Total fuel flow of all engines `READ_AT_S` after moving the throttle from idle to `cmd`,
/// stepping each engine at `sub_dt_s` (the model's sub-step), kg/s.
#[must_use]
pub fn run(model: &AircraftModel, cmd: EngineCommand, sub_dt_s: f64) -> f64 {
    let air = bench(model);
    let steps = (READ_AT_S / sub_dt_s).round() as u32;
    model
        .engines()
        .iter()
        .map(|e| {
            let mut s = e.model.steady(EngineCommand::IDLE);
            for _ in 0..steps {
                e.model.step(&mut s, cmd, &air, true);
            }
            e.model.fuel_flow_kgps(&s, e.model.thrust_n(&s, &air), &air)
        })
        .sum()
}

/// The steady-state total fuel flow at `cmd`, kg/s.
#[must_use]
pub fn steady(model: &AircraftModel, cmd: EngineCommand) -> f64 {
    let air = bench(model);
    model
        .engines()
        .iter()
        .map(|e| {
            let s = e.model.steady(cmd);
            e.model.fuel_flow_kgps(&s, e.model.thrust_n(&s, &air), &air)
        })
        .sum()
}
