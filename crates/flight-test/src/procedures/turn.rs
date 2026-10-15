//! Sustained turn: load factor and turn rate at full afterburner (ADR-31).
//!
//! The static throttle-fixed turn trim gives the sustained load factor at the point's speed; the
//! dynamic run starts from it and holds the turn for 40 s at full power, controlling the load
//! factor through speed (`n = n_trim + 0.05 (V - V_target)`), the bank through altitude, and the
//! ball centred. The last 20 s are averaged. `n` is the wind-axis load factor
//! `sqrt(1 + (V w / g0)^2)` from the measured turn rate `w`; both are capped at `n_max` and at the
//! `C_L,max` limit, as C.7 does. Cross-check: the static trim, within 0.5 %.

use flight::aircraft::AircraftModel;
use flight::energy::level_turn_load_factor;
use flight::engine::EngineCommand;
use flight::env::FlightEnv;
use flight::physical::G0;
use flight::trim::{TrimPoint, TrimProblem, TrimResult, TrimSetup, guess_level, solve, state_of};
use serde_json::{Value, json};
use simmath::dmath;

use super::{Flight, Trace, world};
use crate::pilot::{K_GAMMA, K_H, MAX_CLIMB_MPS, Vertical};

/// Length of the dynamic hold, s. // ALGO: ADR-31
pub const HOLD_S: f64 = 40.0;
/// The average is taken over the last this many seconds. // ALGO: ADR-31
pub const AVERAGE_S: f64 = 20.0;
/// Speed feedback into the load factor, g per m/s. // ALGO: plan WP7
pub const K_SPEED: f64 = 0.05;
/// Cross-check band, percent. // ALGO: ADR-31
pub const BAND_PCT: f64 = 0.5;

/// Static throttle-fixed turn trim at `speed_mps`, trying a few starting load factors.
#[must_use]
pub fn static_turn(
    model: &AircraftModel,
    setup: &TrimSetup,
    speed_mps: f64,
    tau: f64,
) -> Option<TrimResult> {
    let env = world();
    for n in [6.0, 4.0, 8.0, 3.0] {
        let mut guess: TrimPoint = guess_level(model, setup, speed_mps, n, &env);
        guess.turn_rate_rps = G0 * dmath::sqrt(n * n - 1.0) / speed_mps;
        guess.bank_rad = dmath::acos(1.0 / n);
        guess.throttle = tau;
        let r = solve(model, &TrimProblem::turn_at_throttle(), setup, guess, &env);
        if r.converged && r.point.turn_rate_rps > 0.0 && r.point.alpha_rad > 0.0 {
            return Some(r);
        }
    }
    None
}

/// The load-factor ceiling at a speed: `n_max` and the `C_L,max` limit.
#[must_use]
pub fn load_cap(model: &AircraftModel, setup: &TrimSetup, speed_mps: f64) -> f64 {
    let env = world();
    let air = model
        .atmosphere()
        .sample(env.height_msl_m(setup.pos_bf), env.day());
    let mach = speed_mps / air.a_mps;
    let qs = 0.5 * air.rho_kgpm3 * speed_mps * speed_mps * model.aero.wing_area_m2();
    let weight = model.mass.props(&setup.fuel).mass_kg * G0;
    model
        .limits
        .n_max
        .min(qs * model.aero.cl_max(mach) / weight)
}

/// The dynamic run's outcome.
#[derive(Clone, Debug, PartialEq)]
pub struct Run {
    /// Mean wind-axis load factor over the averaging window.
    pub n: f64,
    /// Mean turn rate over the window, rad/s.
    pub turn_rate_rps: f64,
    /// Mean speed error over the window, m/s.
    pub speed_error_mps: f64,
    /// Largest altitude error over the window, m.
    pub max_alt_error_m: f64,
    /// Simulated time, s.
    pub sim_seconds: f64,
    /// Integrator faults.
    pub faults: u32,
    /// The time history.
    pub trace: Trace,
}

/// Flies the 40 s hold from the static trim `start` at full power.
#[must_use]
pub fn hold(
    model: &AircraftModel,
    setup: &TrimSetup,
    start: &TrimPoint,
    engine: EngineCommand,
    tau: f64,
    name: &str,
) -> Run {
    let env = world();
    let state = state_of(model, setup, start, &env);
    let h0 = env.height_msl_m(setup.pos_bf);
    let v_target = start.speed_mps;
    let n_trim = level_turn_load_factor(v_target, start.turn_rate_rps);
    let mut f = Flight::new(model, state, engine, tau, setup.config, name);
    let (mut sum_n, mut sum_w, mut sum_dv, mut count) = (0.0, 0.0, 0.0, 0.0);
    let mut max_dh = 0.0_f64;
    while f.t() < HOLD_S {
        let o = f.observe();
        let v = o.tel.air.tas_mps;
        let n_cmd = n_trim + K_SPEED * (v - v_target);
        // The bank that leaves the vertical load the altitude loop asks for.
        let climb = (K_H * (h0 - o.tel.height_msl_m)).clamp(-MAX_CLIMB_MPS, MAX_CLIMB_MPS);
        let gamma_cmd = dmath::asin((climb / v).clamp(-1.0, 1.0));
        let n_vertical = dmath::cos(o.gamma_rad) + v / G0 * K_GAMMA * (gamma_cmd - o.gamma_rad);
        let bank = dmath::acos((n_vertical / n_cmd).clamp(-1.0, 1.0));
        let o = f.fly(Vertical::LoadFactor(n_cmd), bank);
        if f.faults > 0 {
            break;
        }
        if f.t() > HOLD_S - AVERAGE_S {
            sum_n += level_turn_load_factor(v, o.turn_rate_rps);
            sum_w += o.turn_rate_rps;
            sum_dv += v - v_target;
            count += 1.0;
            max_dh = max_dh.max((o.tel.height_msl_m - h0).abs());
        }
    }
    let mean = |s: f64| if count > 0.0 { s / count } else { f64::NAN };
    Run {
        n: mean(sum_n),
        turn_rate_rps: mean(sum_w),
        speed_error_mps: mean(sum_dv),
        max_alt_error_m: max_dh,
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
            "mean_n": self.n,
            "mean_turn_rate_dps": self.turn_rate_rps.to_degrees(),
            "mean_speed_error_mps": self.speed_error_mps,
            "max_alt_error_m": self.max_alt_error_m,
            "faults": self.faults,
        })
    }
}
