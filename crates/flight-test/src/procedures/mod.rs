//! The flight-test procedures (ADR-31) and the harness they share.

pub mod fuel_flow;
pub mod level_accel;
pub mod stall;
pub mod turn;

use content::EnvelopeConfig;
use content::aircraft::EnvelopePoint;
use flight::actuators::Surfaces;
use flight::aircraft::{AircraftModel, AircraftState, Commands};
use flight::engine::EngineCommand;
use flight::env::SphereSea;
use flight::trim::TrimSetup;
use simmath::math::DVec3;

use crate::pilot::{Observed, Pilot, Target, Vertical, observe};

/// The tick the procedures fly at: the game's 60 Hz (ADR-07).
pub const TICK_DT_S: f64 = 1.0 / 60.0;
/// Trace rows are written every this many ticks (10 Hz).
pub const TRACE_EVERY: u64 = 6;
/// Columns of a trace.
pub const TRACE_COLUMNS: [&str; 18] = [
    "t_s",
    "h_m",
    "tas_mps",
    "mach",
    "alpha_deg",
    "n",
    "bank_deg",
    "throttle",
    "thrust_n",
    "fuel_flow_kgps",
    "ps_mps",
    "beta_deg",
    "elevator_deg",
    "aileron_deg",
    "rudder_deg",
    "p_dps",
    "q_dps",
    "r_dps",
];

/// A 10 Hz time history of one run.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Trace {
    /// File stem (the run's name).
    pub name: String,
    /// Rows of [`TRACE_COLUMNS`].
    pub rows: Vec<[f64; 18]>,
}

impl Trace {
    /// The trace as CSV with a header line.
    #[must_use]
    pub fn csv(&self) -> String {
        let mut s = TRACE_COLUMNS.join(",");
        s.push('\n');
        for row in &self.rows {
            let cells: Vec<String> = row.iter().map(|v| format!("{v:.6}")).collect();
            s += &cells.join(",");
            s.push('\n');
        }
        s
    }
}

/// The world every procedure flies in: the Earth-sized sphere, still air, standard day, and no
/// surface, so nothing is ever in ground effect (ADR-31).
#[must_use]
pub fn world() -> SphereSea {
    SphereSea::open()
}

/// The fixed part of a point: CG at its altitude over the BF origin, fuel for its mass in feed
/// order, surfaces for its configuration.
#[must_use]
pub fn setup_for(model: &AircraftModel, p: &EnvelopePoint) -> TrimSetup {
    let fuel_kg = f64::from(p.mass_kg) - model.mass.empty_kg();
    TrimSetup {
        pos_bf: DVec3::new(0.0, 0.0, f64::from(p.alt_m)),
        fuel: model.mass.load_for(fuel_kg),
        config: config_surfaces(model, p.config),
    }
}

/// Flaps, airbrake and gear of a configuration.
#[must_use]
pub fn config_surfaces(model: &AircraftModel, c: EnvelopeConfig) -> Surfaces {
    let tef = model.actuators.tef.max;
    match c {
        EnvelopeConfig::ToFlapsAb => Surfaces {
            tef_rad: 0.5 * tef,
            gear: 1.0,
            ..Surfaces::NEUTRAL
        },
        EnvelopeConfig::LdgBrakes => Surfaces {
            tef_rad: tef,
            gear: 1.0,
            ..Surfaces::NEUTRAL
        },
        _ => Surfaces::NEUTRAL,
    }
}

/// The power setting of a configuration: `(command, throttle tau)`.
#[must_use]
pub fn config_power(model: &AircraftModel, c: EnvelopeConfig) -> (EngineCommand, f64) {
    let stages = model.ab_stages();
    match c {
        EnvelopeConfig::CleanAb | EnvelopeConfig::RefAb | EnvelopeConfig::ToFlapsAb
            if stages > 0 =>
        {
            (EngineCommand::max_ab(stages), 2.0)
        }
        EnvelopeConfig::Clean | EnvelopeConfig::LdgBrakes => (EngineCommand::IDLE, 0.0),
        _ => (EngineCommand::MILITARY, 1.0),
    }
}

/// One aircraft being flown by the test pilot.
pub struct Flight<'a> {
    /// The aircraft.
    pub model: &'a AircraftModel,
    /// The world.
    pub env: SphereSea,
    /// The state.
    pub state: AircraftState,
    /// Engine command held for the run.
    pub engine: EngineCommand,
    /// Throttle `tau` for the trace.
    pub tau: f64,
    /// Flaps, airbrake and gear.
    pub config: Surfaces,
    /// The pilot.
    pub pilot: Pilot,
    /// Ticks flown.
    pub tick: u64,
    /// The time history.
    pub trace: Trace,
    /// Integrator faults seen (any is a failed run).
    pub faults: u32,
}

impl<'a> Flight<'a> {
    /// A run from `state` with mass frozen and the engine held at `engine`.
    #[must_use]
    pub fn new(
        model: &'a AircraftModel,
        mut state: AircraftState,
        engine: EngineCommand,
        tau: f64,
        config: Surfaces,
        name: &str,
    ) -> Self {
        state.mass_frozen = true;
        Self {
            model,
            env: world(),
            state,
            engine,
            tau,
            config,
            pilot: Pilot,
            tick: 0,
            trace: Trace {
                name: name.to_owned(),
                rows: Vec::new(),
            },
            faults: 0,
        }
    }

    /// Simulated time, s.
    #[must_use]
    pub fn t(&self) -> f64 {
        self.tick as f64 * TICK_DT_S
    }

    /// Reads the state without flying.
    #[must_use]
    pub fn observe(&self) -> Observed {
        observe(self.model, &self.state, &self.env, self.t())
    }

    /// One tick: observe, let the pilot command the surfaces, step. Returns what was observed
    /// before the step.
    pub fn fly(&mut self, vertical: Vertical, bank_rad: f64) -> Observed {
        let o = self.observe();
        if self.tick.is_multiple_of(TRACE_EVERY) {
            let air = &o.tel.air;
            self.trace.rows.push([
                self.t(),
                o.tel.height_msl_m,
                air.tas_mps,
                air.mach,
                air.alpha_rad.to_degrees(),
                o.n_wind,
                o.bank_rad.to_degrees(),
                self.tau,
                o.tel.thrust_n,
                o.tel.fuel_flow_kgps,
                o.ps_mps,
                air.beta_rad.to_degrees(),
                self.state.surfaces.elevator_rad.to_degrees(),
                self.state.surfaces.aileron_rad.to_degrees(),
                self.state.surfaces.rudder_rad.to_degrees(),
                self.state.body.omega_body.x.to_degrees(),
                self.state.body.omega_body.y.to_degrees(),
                self.state.body.omega_body.z.to_degrees(),
            ]);
        }
        let target = Target {
            vertical,
            bank_rad,
            config: self.config,
        };
        let surfaces = self
            .pilot
            .command(self.model, &self.state, &o, &target, &self.env);
        let cmd = Commands {
            surfaces,
            engine: self.engine,
        };
        let t = self.t();
        self.faults += self
            .model
            .step_tick(&mut self.state, &cmd, &self.env, t)
            .faults;
        self.tick += 1;
        o
    }
}

/// Least-squares line `y = a + b x` through the points; `None` with fewer than two distinct x.
#[must_use]
pub fn fit_line(points: &[(f64, f64)]) -> Option<(f64, f64)> {
    let n = points.len() as f64;
    if points.len() < 2 {
        return None;
    }
    let (sx, sy) = points
        .iter()
        .fold((0.0, 0.0), |(sx, sy), (x, y)| (sx + x, sy + y));
    let (mx, my) = (sx / n, sy / n);
    let (mut sxx, mut sxy) = (0.0, 0.0);
    for (x, y) in points {
        sxx += (x - mx) * (x - mx);
        sxy += (x - mx) * (y - my);
    }
    if sxx <= 0.0 {
        return None;
    }
    let b = sxy / sxx;
    Some((my - b * mx, b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_through_points() {
        let (a, b) = fit_line(&[(0.0, 1.0), (1.0, 3.0), (2.0, 5.0)]).unwrap();
        assert!((a - 1.0).abs() < 1e-12 && (b - 2.0).abs() < 1e-12);
        assert!(fit_line(&[(1.0, 1.0)]).is_none());
        assert!(fit_line(&[(1.0, 1.0), (1.0, 2.0)]).is_none());
    }

    #[test]
    fn a_trace_is_csv() {
        let t = Trace {
            name: "x".into(),
            rows: vec![[1.0; 18]],
        };
        let csv = t.csv();
        assert!(csv.starts_with("t_s,h_m,"));
        assert_eq!(csv.lines().count(), 2);
    }
}
