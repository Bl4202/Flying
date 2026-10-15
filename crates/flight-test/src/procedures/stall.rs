//! 1 g stall speed by an angle-of-attack ramp (ADR-31).
//!
//! The aircraft is trimmed level at 1.3 `V_s` (equivalent airspeed, so the start height does not
//! matter below M 0.6), the engine goes to idle, and the pilot ramps the angle of attack at
//! 0.5 deg/s with the wings level until 4 deg past the lift peak or until the lift has fallen 5 %
//! below its maximum. The aerodynamic lift coefficient (elevator lift included, thrust excluded)
//! is recorded every tick, and `V_s = sqrt(2 m g0 / (rho0 S max C_L))`, the FAR 25.103-style
//! `C_LMAX` definition. Holding altitude through the stall instead would need thrust and bias `V_s`
//! low (ADR-31). Cross-check: the zero-thrust minimum-speed trim, the largest `C_L` of a pitch
//! trim scanned through the stall.

use flight::aero::AeroInput;
use flight::aircraft::{AircraftModel, AircraftState};
use flight::engine::EngineCommand;
use flight::env::FlightEnv;
use flight::physical::{G0, RHO0};
use flight::trim::{TrimProblem, TrimSetup, engine_states, guess_level, solve, state_of};
use serde_json::{Value, json};
use simmath::dmath;

use super::{Flight, Trace, world};
use crate::pilot::Vertical;

/// Start speed as a multiple of the stall speed. // ALGO: ADR-31
pub const START_FACTOR: f64 = 1.3;
/// Angle-of-attack ramp, rad/s (0.5 deg/s). // ALGO: ADR-31
pub const RAMP_RPS: f64 = 0.5 * core::f64::consts::PI / 180.0;
/// The ramp ends this far past the lift peak, degrees. // ALGO: ADR-31
pub const PAST_PEAK_DEG: f64 = 4.0;
/// ...or once lift has fallen this far below its maximum. // ALGO: ADR-31
pub const LIFT_DROP: f64 = 0.05;
/// Settling before the ramp starts, s. // ALGO: ADR-31
pub const SETTLE_S: f64 = 3.0;
/// Cross-check band, percent. // ALGO: v0.1a (ADR-31 names the check, not the band)
pub const BAND_PCT: f64 = 0.5;
/// Pitch-trim scan of the cross-check: from this fraction of the peak alpha, in these steps. // ALGO
const SCAN_FROM: f64 = 0.7;
const SCAN_STEP_DEG: f64 = 0.05;

/// The outcome of a stall run.
#[derive(Clone, Debug, PartialEq)]
pub struct Run {
    /// Largest aerodynamic lift coefficient seen.
    pub cl_max: f64,
    /// Angle of attack there, degrees.
    pub alpha_at_max_deg: f64,
    /// The ramp reached its end condition.
    pub completed: bool,
    /// Simulated time, s.
    pub sim_seconds: f64,
    /// Integrator faults.
    pub faults: u32,
    /// The time history.
    pub trace: Trace,
}

/// Stall speed for a lift coefficient: `sqrt(2 m g0 / (rho0 S C_L))`, m/s (EAS).
#[must_use]
pub fn stall_speed(mass_kg: f64, wing_area_m2: f64, cl: f64) -> f64 {
    dmath::sqrt(2.0 * mass_kg * G0 / (RHO0 * wing_area_m2 * cl))
}

/// The aerodynamic lift coefficient of a state.
fn lift_coefficient<E: FlightEnv>(model: &AircraftModel, s: &AircraftState, env: &E) -> f64 {
    let tel = model.telemetry(s, env, 0.0);
    let input = AeroInput {
        air: tel.air,
        alpha_dot_rps: 0.0,
        surfaces: s.surfaces,
        height_agl_m: tel.height_msl_m - env.ground_height_m(s.body.pos_bf),
        cg_body_m: model.mass.props(&s.fuel).cg_body_m,
    };
    model.aero.coefficients(&input).cl
}

/// The static cross-check: the largest lift coefficient of the pitch-trimmed aircraft at `speed`,
/// scanning the angle of attack through the stall.
#[must_use]
pub fn static_cl_max(model: &AircraftModel, setup: &TrimSetup, speed_mps: f64) -> f64 {
    let env = world();
    let mach = speed_mps
        / model
            .atmosphere()
            .sample(env.height_msl_m(setup.pos_bf), env.day())
            .a_mps;
    let peak = model.aero.alpha_peak_deg(mach);
    let mut best = 0.0_f64;
    let mut deg = SCAN_FROM * peak;
    let mut guess = guess_level(model, setup, speed_mps, 1.0, &env);
    guess.throttle = 0.0;
    while deg <= peak + PAST_PEAK_DEG {
        guess.alpha_rad = deg.to_radians();
        let r = solve(model, &TrimProblem::elevator(), setup, guess, &env);
        if r.converged {
            guess.elevator_rad = r.point.elevator_rad;
            let s = state_of(model, setup, &r.point, &env);
            best = best.max(lift_coefficient(model, &s, &env));
        }
        deg += SCAN_STEP_DEG;
    }
    best
}

/// Flies the ramp from a level trim at `speed_mps` (TAS at the setup's height).
#[must_use]
pub fn ramp(model: &AircraftModel, setup: &TrimSetup, speed_mps: f64, cap_s: f64) -> Run {
    let env = world();
    let guess = guess_level(model, setup, speed_mps, 1.0, &env);
    let trim = solve(model, &TrimProblem::level(), setup, guess, &env);
    let mut state = state_of(model, setup, &trim.point, &env);
    state.engines = engine_states(model, 0.0);
    let mut f = Flight::new(
        model,
        state,
        EngineCommand::IDLE,
        0.0,
        setup.config,
        "stall_ramp",
    );
    let alpha0 = trim.point.alpha_rad;
    let (mut cl_max, mut alpha_at_max) = (0.0_f64, 0.0_f64);
    let mut completed = false;
    while f.t() < cap_s {
        let t = f.t();
        // Settle holding the trimmed angle of attack (the engine has just gone to idle), then
        // ramp.
        let vertical = if t < SETTLE_S {
            Vertical::Alpha(alpha0, 0.0)
        } else {
            Vertical::Alpha(alpha0 + RAMP_RPS * (t - SETTLE_S), RAMP_RPS)
        };
        let o = f.fly(vertical, 0.0);
        if f.faults > 0 {
            break;
        }
        if t < SETTLE_S {
            continue;
        }
        let cl = lift_coefficient(model, &f.state, &f.env);
        let alpha_deg = o.tel.air.alpha_rad.to_degrees();
        if cl > cl_max {
            cl_max = cl;
            alpha_at_max = alpha_deg;
        }
        let past_peak = alpha_deg > model.aero.alpha_peak_deg(o.tel.air.mach) + PAST_PEAK_DEG;
        let dropped = alpha_deg > alpha_at_max && cl < (1.0 - LIFT_DROP) * cl_max;
        if past_peak || dropped {
            completed = true;
            break;
        }
    }
    Run {
        cl_max,
        alpha_at_max_deg: alpha_at_max,
        completed,
        sim_seconds: f.t(),
        faults: f.faults,
        trace: f.trace,
    }
}

impl Run {
    /// Measurement figures for the report.
    #[must_use]
    pub fn stats(&self) -> Value {
        json!({
            "cl_max": self.cl_max,
            "alpha_at_cl_max_deg": self.alpha_at_max_deg,
            "completed": self.completed,
            "faults": self.faults,
        })
    }
}
