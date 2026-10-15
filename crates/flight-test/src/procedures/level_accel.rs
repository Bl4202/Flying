//! Level acceleration: maximum level Mach and maximum rate of climb (ADR-31).
//!
//! The aircraft is trimmed level at a starting Mach, the engine goes straight to the point's
//! power, and the pilot holds altitude with the wings level while it accelerates. After 3 s of
//! settling every tick is a sample of Mach and `P_s`. A sample is valid while the altitude error
//! stays below 15 m and the load factor within 0.01 of 1.
//!
//! - **`M_max`:** once `P_s` falls below 5 m/s, a line through `P_s(M)` over the last 0.03 Mach is
//!   extrapolated to zero. Cross-check: the static level trim at the same throttle (0.3 %).
//! - **Maximum rate of climb:** the largest mean `P_s` in 0.005 Mach bins (C.7's 283.1 m/s is the
//!   peak of 1 g level `P_s`). Cross-check: the largest trimmed `P_s` from M 0.5 to 1.1 (0.5 %).

use flight::aircraft::AircraftModel;
use flight::engine::EngineCommand;
use flight::env::FlightEnv;
use flight::trim::{TrimProblem, TrimSetup, engine_states, guess_level, solve, state_of};
use serde_json::{Value, json};

use super::{Flight, Trace, fit_line, world};
use crate::pilot::Vertical;

/// Settling time before samples count, s. // ALGO: ADR-31
pub const SETTLE_S: f64 = 3.0;
/// The run ends once valid `P_s` is below this, m/s. // ALGO: ADR-31
pub const STOP_PS_MPS: f64 = 5.0;
/// The `M_max` line is fitted over this much Mach before the end. // ALGO: ADR-31
pub const FIT_WINDOW_MACH: f64 = 0.03;
/// Width of the `P_s` bins for the climb rate. // ALGO: ADR-31
pub const PS_BIN_MACH: f64 = 0.005;
/// Largest altitude error of a valid sample, m. // ALGO: ADR-31
pub const MAX_ALT_ERROR_M: f64 = 15.0;
/// Largest load-factor error of a valid sample. // ALGO: ADR-31
pub const MAX_N_ERROR: f64 = 0.01;
/// Cross-check band for `M_max`, percent. // ALGO: ADR-31
pub const VMAX_BAND_PCT: f64 = 0.3;
/// Cross-check band for the climb rate, percent. // ALGO: v0.1a (ADR-31 names the check, not the band)
pub const ROC_BAND_PCT: f64 = 0.5;

/// One sample of an acceleration run.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sample {
    /// Mach.
    pub mach: f64,
    /// Specific excess power, m/s.
    pub ps_mps: f64,
    /// Inside the altitude and load-factor bands.
    pub valid: bool,
}

/// An acceleration run.
#[derive(Clone, Debug, PartialEq)]
pub struct Run {
    /// Samples after settling.
    pub samples: Vec<Sample>,
    /// Simulated time flown, s.
    pub sim_seconds: f64,
    /// The time cap ended it.
    pub timed_out: bool,
    /// The start trim failed.
    pub trim_failed: bool,
    /// Integrator faults.
    pub faults: u32,
    /// Largest altitude error after settling, m.
    pub max_alt_error_m: f64,
    /// Largest load-factor error after settling.
    pub max_n_error: f64,
    /// The time history.
    pub trace: Trace,
}

/// Flies a level acceleration from `start_mach` at full `engine` power (`tau`), until `P_s` is
/// below the stop value or `cap_s` has passed.
#[must_use]
pub fn accelerate(
    model: &AircraftModel,
    setup: &TrimSetup,
    start_mach: f64,
    engine: EngineCommand,
    tau: f64,
    cap_s: f64,
    name: &str,
) -> Run {
    let env = world();
    let h0 = env.height_msl_m(setup.pos_bf);
    let a = model.atmosphere().sample(h0, env.day()).a_mps;
    let guess = guess_level(model, setup, start_mach * a, 1.0, &env);
    let trim = solve(model, &TrimProblem::level(), setup, guess, &env);
    let mut state = state_of(model, setup, &trim.point, &env);
    state.engines = engine_states(model, tau);
    let mut f = Flight::new(model, state, engine, tau, setup.config, name);
    let mut samples = Vec::new();
    let (mut max_dh, mut max_dn) = (0.0_f64, 0.0_f64);
    let mut timed_out = true;
    while f.t() < cap_s {
        let o = f.fly(Vertical::Altitude(h0), 0.0);
        if f.t() < SETTLE_S {
            continue;
        }
        let dh = (o.tel.height_msl_m - h0).abs();
        let dn = (o.n_wind - 1.0).abs();
        max_dh = max_dh.max(dh);
        max_dn = max_dn.max(dn);
        let valid = dh < MAX_ALT_ERROR_M && dn < MAX_N_ERROR;
        samples.push(Sample {
            mach: o.tel.air.mach,
            ps_mps: o.ps_mps,
            valid,
        });
        if valid && o.ps_mps < STOP_PS_MPS {
            timed_out = false;
            break;
        }
        if f.faults > 0 {
            timed_out = false;
            break;
        }
    }
    Run {
        samples,
        sim_seconds: f.t(),
        timed_out,
        trim_failed: !trim.converged,
        faults: f.faults,
        max_alt_error_m: max_dh,
        max_n_error: max_dn,
        trace: f.trace,
    }
}

impl Run {
    /// `M_max` by extrapolating `P_s(M)` to zero over the last [`FIT_WINDOW_MACH`]; `None` if the
    /// run did not get there or the fit is not falling.
    #[must_use]
    pub fn max_mach(&self) -> Option<f64> {
        let last = self.samples.last()?;
        if !last.valid || last.ps_mps >= STOP_PS_MPS {
            return None;
        }
        let window: Vec<(f64, f64)> = self
            .samples
            .iter()
            .filter(|s| s.valid && s.mach >= last.mach - FIT_WINDOW_MACH)
            .map(|s| (s.mach, s.ps_mps))
            .collect();
        let (a, b) = fit_line(&window)?;
        (b < 0.0).then(|| -a / b)
    }

    /// The largest mean `P_s` over [`PS_BIN_MACH`] bins of valid samples, with the bin's centre
    /// Mach.
    #[must_use]
    pub fn max_ps(&self) -> Option<(f64, f64)> {
        let mut best: Option<(f64, f64)> = None;
        let mut i = 0;
        let valid: Vec<&Sample> = self.samples.iter().filter(|s| s.valid).collect();
        let bin_of = |m: f64| (m / PS_BIN_MACH).floor() as i64;
        while i < valid.len() {
            let bin = bin_of(valid[i].mach);
            let mut sum = 0.0;
            let mut n = 0.0;
            while i < valid.len() && bin_of(valid[i].mach) == bin {
                sum += valid[i].ps_mps;
                n += 1.0;
                i += 1;
            }
            let mean = sum / n;
            if best.is_none_or(|(b, _)| mean > b) {
                best = Some((mean, (bin as f64 + 0.5) * PS_BIN_MACH));
            }
        }
        best
    }

    /// Measurement quality figures for the report.
    #[must_use]
    pub fn stats(&self) -> Value {
        let valid = self.samples.iter().filter(|s| s.valid).count();
        json!({
            "samples": self.samples.len(),
            "valid_samples": valid,
            "max_alt_error_m": self.max_alt_error_m,
            "max_n_error": self.max_n_error,
            "end_mach": self.samples.last().map(|s| s.mach),
            "end_ps_mps": self.samples.last().map(|s| s.ps_mps),
            "faults": self.faults,
            "start_trim_converged": !self.trim_failed,
        })
    }
}

/// Static cross-check for `M_max`: level trim at the throttle, solving for speed from `guess_mach`.
#[must_use]
pub fn static_max_mach(
    model: &AircraftModel,
    setup: &TrimSetup,
    tau: f64,
    guess_mach: f64,
) -> Option<f64> {
    let env = world();
    let a = model
        .atmosphere()
        .sample(env.height_msl_m(setup.pos_bf), env.day())
        .a_mps;
    let mut guess = guess_level(model, setup, guess_mach * a, 1.0, &env);
    guess.throttle = tau;
    let r = solve(model, &TrimProblem::level_at_throttle(), setup, guess, &env);
    r.converged.then(|| r.point.speed_mps / a)
}

/// Static cross-check for the climb rate: the largest 1 g level `P_s` of the trimmed aircraft at
/// the throttle, over M 0.5 to 1.1 in [`PS_BIN_MACH`] steps.
#[must_use]
pub fn static_max_ps(model: &AircraftModel, setup: &TrimSetup, tau: f64) -> Option<f64> {
    let env = world();
    let a = model
        .atmosphere()
        .sample(env.height_msl_m(setup.pos_bf), env.day())
        .a_mps;
    let mut best: Option<f64> = None;
    let steps = ((1.1 - 0.5) / PS_BIN_MACH).round() as u32;
    for i in 0..=steps {
        let mach = 0.5 + f64::from(i) * PS_BIN_MACH;
        let mut guess = guess_level(model, setup, mach * a, 1.0, &env);
        guess.throttle = tau;
        let r = solve(model, &TrimProblem::lift_and_moment(), setup, guess, &env);
        if !r.converged {
            continue;
        }
        let s = state_of(model, setup, &r.point, &env);
        let tel = model.telemetry(&s, &env, 0.0);
        let v_body = s.body.att.conjugate().rotate(s.body.vel_bf);
        let ps = tel.force_body_n.dot(v_body) / (tel.mass_kg * flight::physical::G0);
        best = Some(best.map_or(ps, |b: f64| b.max(ps)));
    }
    best
}
