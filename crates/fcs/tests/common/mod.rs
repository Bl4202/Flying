//! A rig that flies the Kestrel through the real control system: trimmed spawn, the FCS once per
//! tick, the airframe's tick, and the engine held (an autothrottle) unless a test says otherwise.
#![allow(dead_code, reason = "each test file uses part of the rig")]

use std::path::Path;

use content::Registry;
use fcs::sense::Sensed;
use fcs::{Assist, FcsModel, FcsState, PilotInput};
use flight::actuators::Surfaces;
use flight::aircraft::{AircraftModel, AircraftState};
use flight::engine::EngineCommand;
use flight::env::{FlightEnv, SphereSea};
use flight::trim::{TrimProblem, TrimSetup, engine_command, guess_level, solve, state_of};
use simmath::math::DVec3;

pub const DT: f64 = 1.0 / 60.0;

pub fn registry() -> Registry {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content/core");
    let baked = content_bake::bake::bake_dir(&dir).expect("the core pack bakes");
    Registry::from_cpack(&baked.cpack).unwrap()
}

/// The airframe and its control system.
pub fn kestrel() -> (AircraftModel, FcsModel) {
    let r = registry();
    let h = r.aircraft_handles().next().unwrap();
    let model = AircraftModel::from_registry(&r, h, DT).unwrap();
    let fcs = FcsModel::from_registry(&r, h, &model).unwrap();
    (model, fcs)
}

pub struct Rig<'a> {
    pub model: &'a AircraftModel,
    pub fcs: &'a FcsModel,
    pub ac: AircraftState,
    pub fs: FcsState,
    pub world: SphereSea,
    pub engine: EngineCommand,
    /// The engine follows the throttle lever through the control system instead of `engine`.
    pub use_lever: bool,
    pub tick: u64,
    pub faults: u32,
    pub touchdowns: u32,
    pub destroyed: Option<flight::gear::CrashCause>,
}

impl<'a> Rig<'a> {
    /// Level and trimmed at `mach` and `alt_m` with `mass_kg`, clean, under `assist`. `None` if
    /// the condition cannot be trimmed (out of the envelope).
    pub fn new(
        model: &'a AircraftModel,
        fcs: &'a FcsModel,
        assist: Assist,
        mach: f64,
        alt_m: f64,
        mass_kg: f64,
    ) -> Option<Self> {
        let world = SphereSea::open();
        let a = model
            .atmosphere()
            .sample(alt_m, flight::atmosphere::NonStandardDay::ISA)
            .a_mps;
        Self::trimmed(
            world,
            model,
            fcs,
            assist,
            mach * a,
            alt_m,
            mass_kg,
            Surfaces::NEUTRAL,
        )
    }

    /// Level and trimmed at `speed_mps` true airspeed, `alt_m` above the sea, in `world`, with
    /// the configuration `config` (flaps, airbrake and gear).
    #[allow(clippy::too_many_arguments, reason = "a test condition")]
    pub fn trimmed(
        world: SphereSea,
        model: &'a AircraftModel,
        fcs: &'a FcsModel,
        assist: Assist,
        speed_mps: f64,
        alt_m: f64,
        mass_kg: f64,
        config: Surfaces,
    ) -> Option<Self> {
        let setup = TrimSetup {
            pos_bf: DVec3::new(0.0, 0.0, alt_m),
            fuel: model.mass.load_for(mass_kg - model.mass.empty_kg()),
            config,
        };
        let guess = guess_level(model, &setup, speed_mps, 1.0, &world);
        let t = solve(model, &TrimProblem::level(), &setup, guess, &world);
        if !t.converged {
            return None;
        }
        let engine = engine_command(model, t.point.throttle);
        let ac = state_of(model, &setup, &t.point, &world);
        let mut fs = FcsState::new(assist, false);
        fs.last_cmd = [
            ac.surfaces.elevator_rad,
            ac.surfaces.aileron_rad,
            ac.surfaces.rudder_rad,
        ];
        Some(Self {
            model,
            fcs,
            ac,
            fs,
            world,
            engine,
            use_lever: false,
            tick: 0,
            faults: 0,
            touchdowns: 0,
            destroyed: None,
        })
    }

    /// At rest on the paved runway at the static pose with `mass_kg`, take-off flaps, gear down,
    /// engines at idle, the control system in ground mode. The throttle comes from the lever.
    pub fn on_runway(
        model: &'a AircraftModel,
        fcs: &'a FcsModel,
        assist: Assist,
        mass_kg: f64,
    ) -> Self {
        let world = SphereSea::default();
        let (mut ac, _) = model
            .static_state(
                mass_kg - model.mass.empty_kg(),
                0.0,
                0.0,
                DVec3::ZERO,
                &world,
                EngineCommand::IDLE,
            )
            .expect("the Kestrel stands on its gear");
        ac.surfaces.tef_rad = 0.5 * model.actuators.tef.max;
        Self {
            model,
            fcs,
            ac,
            fs: FcsState::new(assist, true),
            world,
            engine: EngineCommand::IDLE,
            use_lever: true,
            tick: 0,
            faults: 0,
            touchdowns: 0,
            destroyed: None,
        }
    }

    pub fn t(&self) -> f64 {
        self.tick as f64 * DT
    }

    /// What the control laws see now.
    pub fn sensed(&self) -> Sensed {
        let sense = self.model.sense(&self.ac, &self.world, self.t());
        Sensed::of(self.model, &self.ac, &sense, &self.world)
    }

    /// One tick: the FCS, then the airframe. Returns the sensed state before the step.
    pub fn step(&mut self, input: &PilotInput) -> Sensed {
        let s = self.sensed();
        let t = self.t();
        let mut cmd = self.fcs.step(
            self.model,
            &mut self.ac,
            &mut self.fs,
            input,
            &self.world,
            t,
        );
        if !self.use_lever {
            cmd.engine = self.engine;
        }
        let out = self.model.step_tick(&mut self.ac, &cmd, &self.world, t);
        self.faults += out.faults;
        self.touchdowns += out.events.touchdowns;
        self.destroyed = self.destroyed.or(out.events.destroyed);
        self.tick += 1;
        s
    }

    /// Flies `seconds` with the input `f(t)` and returns what was sensed each tick.
    pub fn run(
        &mut self,
        seconds: f64,
        mut f: impl FnMut(f64, &Sensed) -> PilotInput,
    ) -> Vec<Sensed> {
        let n = (seconds / DT).round() as u64;
        let mut out = Vec::with_capacity(n as usize);
        let t0 = self.t();
        for _ in 0..n {
            let s = self.sensed();
            let input = f(self.t() - t0, &s);
            out.push(self.step(&input));
        }
        out
    }

    /// Rotates the aircraft and its velocity nose-up by `dpitch_rad` about the body y axis, so
    /// the flight path steepens by the same angle (a dive entry with a negative angle).
    pub fn pitch_over(&mut self, dpitch_rad: f64) {
        let q = simmath::math::DQuat::from_axis_angle(DVec3::Y, dpitch_rad);
        let vel_body = self.ac.body.att.conjugate().rotate(self.ac.body.vel_bf);
        self.ac.body.att = (self.ac.body.att * q).normalized();
        self.ac.body.vel_bf = self.ac.body.att.rotate(vel_body);
    }

    /// Height above MSL, m.
    pub fn height(&self) -> f64 {
        self.world.height_msl_m(self.ac.body.pos_bf)
    }
}

/// A stick-only input.
pub fn stick(pitch: f64, roll: f64, yaw: f64) -> PilotInput {
    PilotInput {
        controls: flight::controls::PilotControls {
            pitch,
            roll,
            yaw,
            ..flight::controls::PilotControls::default()
        },
        ..PilotInput::default()
    }
}

/// Overshoot (percent of the commanded change) and the time after the step at which the response
/// last left the band of `band` (fraction) around `target` (the 90 % settling time for 0.1).
pub fn step_metrics(series: &[f64], t_step: f64, base: f64, target: f64, band: f64) -> (f64, f64) {
    let i0 = (t_step / DT).round() as usize;
    let change = target - base;
    let tail = &series[i0..];
    let peak = if change >= 0.0 {
        tail.iter().copied().fold(f64::MIN, f64::max)
    } else {
        tail.iter().copied().fold(f64::MAX, f64::min)
    };
    let over = ((peak - target) / change * 100.0).max(0.0);
    let mut last_out = 0.0;
    for (i, v) in tail.iter().enumerate() {
        if (v - target).abs() > band * change.abs() {
            last_out = (i + 1) as f64 * DT;
        }
    }
    (over, last_out)
}

/// The load factor at the angle-of-attack limit at a condition, g: lift over weight with the
/// angle of attack at the Standard limit.
pub fn n_at_alpha_limit(model: &AircraftModel, mach: f64, qbar_pa: f64, mass_kg: f64) -> f64 {
    let cl = model
        .aero
        .base_coefficients(model.limits.alpha_limit_rad, mach)
        .0;
    cl * qbar_pa * model.aero.wing_area_m2() / (mass_kg * flight::physical::G0)
}
