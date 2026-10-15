//! The Performance Conformance Test (GDD 11.19, ADR-31).
//!
//! Flies an aircraft through scripted flight-test procedures against the performance envelope in
//! its data, with a static cross-check for each point, and reports `conformance_report/1`. It is
//! a test instrument: it uses the same flight model as the game, headless, in a still-air sphere
//! world with no surface.
#![deny(missing_docs)]

pub mod pilot;
pub mod procedures;
pub mod report;

use content::aircraft::EnvelopePoint;
use content::{ContentId, Metric, Registry};
use flight::ModelError;
use flight::aircraft::{AircraftModel, SUBSTEPS};
use flight::env::FlightEnv;
use serde_json::{Value, json};

use procedures::{
    TICK_DT_S, Trace, config_power, fuel_flow, level_accel, setup_for, stall, turn, world,
};
use report::{CrossCheck, PointResult, Report, Status};

/// Simulated-time cap of an acceleration run, s.
pub const ACCEL_CAP_S: f64 = 300.0;
/// Simulated-time cap of the stall ramp, s.
pub const STALL_CAP_S: f64 = 60.0;
/// The stall is flown this far above the point's altitude, m (its speed is equivalent airspeed,
/// so the height does not change it below M 0.6). // ALGO: ADR-31
pub const STALL_HEIGHT_M: f64 = 1000.0;

/// What to run.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Options {
    /// Only these point ids (all when empty).
    pub only: Vec<String>,
}

/// A run's report and its traces.
#[derive(Clone, Debug, PartialEq)]
pub struct Outcome {
    /// The report.
    pub report: Report,
    /// One trace per dynamic run.
    pub traces: Vec<Trace>,
}

/// Hex of a hash.
fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

/// Runs every envelope point of the aircraft behind `handle`.
///
/// # Errors
/// [`ModelError`] if the aircraft cannot be built from the pack.
pub fn run(
    registry: &Registry,
    handle: ContentId,
    build: &str,
    options: &Options,
) -> Result<Outcome, ModelError> {
    let model = AircraftModel::from_registry(registry, handle, TICK_DT_S)?;
    let ac = registry
        .aircraft(handle)
        .ok_or(ModelError::Missing("aircraft"))?;
    let mut runner = Runner {
        model: &model,
        accel: Vec::new(),
        traces: Vec::new(),
    };
    let points = ac
        .envelope
        .iter()
        .filter(|p| options.only.is_empty() || options.only.contains(&p.id))
        .map(|p| runner.point(p))
        .collect();
    Ok(Outcome {
        report: Report {
            aircraft: registry.id(handle).unwrap_or("?").to_owned(),
            content_hash: hex(registry.content_hash()),
            build: build.to_owned(),
            points,
        },
        traces: runner.traces,
    })
}

/// Key of an acceleration run that several points can share: mass, height, configuration and
/// start Mach, by bit pattern.
type AccelKey = (u32, u32, u8, u64);

struct Runner<'a> {
    model: &'a AircraftModel,
    accel: Vec<(AccelKey, level_accel::Run)>,
    traces: Vec<Trace>,
}

impl Runner<'_> {
    fn point(&mut self, p: &EnvelopePoint) -> PointResult {
        let mut r = PointResult {
            id: p.id.clone(),
            metric: p.metric.name().to_owned(),
            target: f64::from(p.value),
            measured: f64::NAN,
            tol_pct: f64::from(p.tol_pct),
            status: Status::Pass,
            method: String::new(),
            cross_check: None,
            sim_seconds: 0.0,
            settle_stats: Value::Null,
            notes: Vec::new(),
        };
        if let Some(req) = p.requires {
            r.status = Status::Deferred;
            r.method = "deferred".into();
            r.notes.push(format!("requires {}", req.name()));
            return r;
        }
        let fuel = f64::from(p.mass_kg) - self.model.mass.empty_kg();
        if fuel < -1e-6 || fuel > self.model.mass.capacity_kg() + 1e-6 {
            r.status = Status::Fail;
            r.notes.push(format!(
                "{} kg needs {fuel:.0} kg of fuel; the tanks hold 0 to {:.0} kg",
                p.mass_kg,
                self.model.mass.capacity_kg()
            ));
            return r;
        }
        match p.metric {
            Metric::MaxMach => self.max_mach(p, &mut r),
            Metric::RocMps => self.climb(p, &mut r),
            Metric::StallMps => self.stall(p, &mut r),
            Metric::NSust | Metric::TurnDps => self.turn(p, &mut r),
            Metric::FfKgps => self.fuel_flow(p, &mut r),
            Metric::RollM => {
                r.status = Status::Fail;
                r.notes
                    .push("no procedure: ground rolls need the gear model (v0.1b)".into());
            }
        }
        r.grade();
        r
    }

    /// The shared acceleration run for a point.
    fn accelerate(&mut self, p: &EnvelopePoint, start_mach: f64) -> &level_accel::Run {
        let key = (
            p.mass_kg.to_bits(),
            p.alt_m.to_bits(),
            p.config.code(),
            start_mach.to_bits(),
        );
        if let Some(i) = self.accel.iter().position(|(k, _)| *k == key) {
            return &self.accel[i].1;
        }
        let (engine, tau) = config_power(self.model, p.config);
        let setup = setup_for(self.model, p);
        let name = format!("accel_{}", p.id);
        let run = level_accel::accelerate(
            self.model,
            &setup,
            start_mach,
            engine,
            tau,
            ACCEL_CAP_S,
            &name,
        );
        self.traces.push(run.trace.clone());
        self.accel.push((key, run));
        &self.accel.last().expect("just pushed").1
    }

    fn max_mach(&mut self, p: &EnvelopePoint, r: &mut PointResult) {
        // ALGO: start one Mach below the target (never below M 0.5), so the run is long enough
        // to settle and to cover the climb-rate peak at sea level.
        let start = (f64::from(p.value) - 1.0).max(0.5);
        let model = self.model;
        let run = self.accelerate(p, start);
        r.method = format!("level acceleration from M {start:.2}, P_s(M) extrapolated to 0");
        r.sim_seconds = run.sim_seconds;
        r.settle_stats = run.stats();
        if run.timed_out {
            r.status = Status::Timeout;
            return;
        }
        let Some(m) = run.max_mach() else {
            r.status = Status::NoConvergence;
            return;
        };
        r.measured = m;
        let (_, tau) = config_power(model, p.config);
        match level_accel::static_max_mach(model, &setup_for(model, p), tau, m) {
            Some(s) => r.cross_check = Some(CrossCheck::new(m, s, level_accel::VMAX_BAND_PCT)),
            None => r
                .notes
                .push("static level trim at full power did not converge".into()),
        }
    }

    fn climb(&mut self, p: &EnvelopePoint, r: &mut PointResult) {
        let model = self.model;
        let run = self.accelerate(p, 0.5);
        r.method = "maximum 1 g level P_s over 0.005 Mach bins of a level acceleration".into();
        r.sim_seconds = run.sim_seconds;
        r.settle_stats = run.stats();
        let Some((ps, mach)) = run.max_ps() else {
            r.status = Status::NoConvergence;
            return;
        };
        r.measured = ps;
        r.notes.push(format!("maximum at M {mach:.3}"));
        let (_, tau) = config_power(model, p.config);
        match level_accel::static_max_ps(model, &setup_for(model, p), tau) {
            Some(s) => r.cross_check = Some(CrossCheck::new(ps, s, level_accel::ROC_BAND_PCT)),
            None => r.notes.push("no trimmed P_s from M 0.5 to 1.1".into()),
        }
    }

    fn stall(&mut self, p: &EnvelopePoint, r: &mut PointResult) {
        let model = self.model;
        let mut setup = setup_for(model, p);
        setup.pos_bf.z += STALL_HEIGHT_M;
        let env = world();
        let mass = model.mass.props(&setup.fuel).mass_kg;
        let s = model.aero.wing_area_m2();
        // Start speed from the static C_L,max at the data's C_L,max speed.
        let first = stall::stall_speed(mass, s, model.aero.cl_max(0.0));
        let rho = model
            .atmosphere()
            .sample(env.height_msl_m(setup.pos_bf), env.day())
            .rho_kgpm3;
        let to_tas = simmath::dmath::sqrt(flight::physical::RHO0 / rho);
        let cl_static = stall::static_cl_max(model, &setup, stall::START_FACTOR * first * to_tas);
        let v_static = stall::stall_speed(mass, s, cl_static);
        let run = stall::ramp(
            model,
            &setup,
            stall::START_FACTOR * v_static * to_tas,
            STALL_CAP_S,
        );
        self.traces.push(run.trace.clone());
        r.method = format!(
            "alpha ramp at 0.5 deg/s from 1.3 V_s at {:.0} m, idle; V_s from max C_L",
            setup.pos_bf.z
        );
        r.sim_seconds = run.sim_seconds;
        r.settle_stats = run.stats();
        if run.faults > 0 {
            r.status = Status::NoConvergence;
            return;
        }
        if !run.completed {
            r.status = Status::Timeout;
            return;
        }
        r.measured = stall::stall_speed(mass, s, run.cl_max);
        r.cross_check = Some(CrossCheck::new(r.measured, v_static, stall::BAND_PCT));
    }

    fn turn(&mut self, p: &EnvelopePoint, r: &mut PointResult) {
        let model = self.model;
        let setup = setup_for(model, p);
        let env = world();
        let Some(mach) = p.mach.map(f64::from) else {
            r.status = Status::Fail;
            r.notes.push("the point has no mach".into());
            return;
        };
        let a = model
            .atmosphere()
            .sample(env.height_msl_m(setup.pos_bf), env.day())
            .a_mps;
        let v = mach * a;
        let (engine, tau) = config_power(model, p.config);
        r.method = format!("throttle-fixed turn at M {mach}: trim, then a 40 s hold, last 20 s");
        let Some(trim) = turn::static_turn(model, &setup, v, tau) else {
            r.status = Status::NoConvergence;
            r.notes.push("the static turn trim did not converge".into());
            return;
        };
        let run = turn::hold(
            model,
            &setup,
            &trim.point,
            engine,
            tau,
            &format!("turn_{}", p.id),
        );
        self.traces.push(run.trace.clone());
        r.sim_seconds = run.sim_seconds;
        r.settle_stats = run.stats();
        if run.faults > 0 || !run.n.is_finite() {
            r.status = Status::NoConvergence;
            return;
        }
        let cap = turn::load_cap(model, &setup, v);
        let static_n = flight::energy::level_turn_load_factor(v, trim.point.turn_rate_rps);
        let capped = |n: f64| n.min(cap);
        let rate_of = |n: f64| flight::physical::G0 * simmath::dmath::sqrt(n * n - 1.0) / v;
        let (measured, cross) = if p.metric == Metric::NSust {
            (capped(run.n), capped(static_n))
        } else {
            (
                rate_of(capped(run.n)).to_degrees(),
                rate_of(capped(static_n)).to_degrees(),
            )
        };
        if run.n > cap {
            r.notes
                .push(format!("capped at {cap:.2} g (n_max or C_L,max)"));
        }
        r.measured = measured;
        r.cross_check = Some(CrossCheck::new(measured, cross, turn::BAND_PCT));
        r.settle_stats["static_n"] = json!(static_n);
    }

    fn fuel_flow(&mut self, p: &EnvelopePoint, r: &mut PointResult) {
        let (engine, _) = config_power(self.model, p.config);
        let sub_dt = TICK_DT_S / f64::from(SUBSTEPS);
        r.method = "engines on a sea-level static bench, idle to the setting, read at 15 s".into();
        r.sim_seconds = fuel_flow::READ_AT_S;
        r.measured = fuel_flow::run(self.model, engine, sub_dt);
        let steady = fuel_flow::steady(self.model, engine);
        r.cross_check = Some(CrossCheck::new(r.measured, steady, fuel_flow::BAND_PCT));
    }
}
