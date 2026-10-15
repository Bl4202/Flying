//! One aircraft: the immutable model built from content, its state, and the 60 Hz tick
//! (GDD 11.2, 11.10, ADR-29).
//!
//! ```text
//! tick (ZOH: commands and mass properties held for the whole tick)
//!   saved = state                                    rollback copy
//!   2 x (dt = tick / 2):
//!     actuators -> engine spool, afterburner, failures (air data at the sub-step start)
//!     RK4 over (p, v, q, w): every stage re-evaluates height, gravity, air, alpha-dot, aero
//!                            and thrust
//!     renormalise q; burn fuel in feed order (unless mass is frozen)
//!     fault check: non-finite, |w| > 50 rad/s, V > 3000 m/s
//!   on a fault: restore, neutralise the surface commands, retry once; a second fault restores
//!   again and flags `numerical_fault` for the caller to respawn
//! ```

use content::{ContentId, Registry};
use simmath::math::{DQuat, DVec3};

use crate::actuators::{Actuators, Surfaces};
use crate::aero::{AeroInput, TableAero};
use crate::airdata::{AirData, cas_mps};
use crate::atmosphere::{AtmoSample, Atmosphere};
use crate::body::{RigidDeriv, RigidState, derivative, step_rk4};
use crate::controls::{ControlLaw, LawInput, ManualLaw, PilotControls, step_trim};
use crate::engine::{
    EngineCommand, EngineConditions, EngineModel, EngineState, MAX_ENGINES, Turbofan,
};
use crate::env::FlightEnv;
use crate::error::ModelError;
use crate::mass::{FuelLoad, MassModel, MassProps};
use crate::physical::G0;

/// RK4 sub-steps per tick: 120 Hz in a 60 Hz tick. // ALGO: GDD 0.6.1, 11.10
pub const SUBSTEPS: u32 = 2;
/// Body rates above this are a numerical fault, rad/s. // ALGO: GDD 11.10 fault guard
pub const MAX_OMEGA_RPS: f64 = 50.0;
/// Speeds above this are a numerical fault, m/s. // ALGO: GDD 11.10 fault guard
pub const MAX_SPEED_MPS: f64 = 3000.0;
/// The stall warning sounds this close to the lift peak, degrees. // ALGO: v0.1a overlay cue
pub const STALL_WARNING_MARGIN_DEG: f64 = 3.0;

/// One engine and where its thrust acts. Thrust is along body `+x`.
#[derive(Clone, Debug, PartialEq)]
pub struct EngineInstall {
    /// The engine.
    pub model: Turbofan,
    /// Thrust point, m (body axes from the reference point).
    pub pos_body_m: DVec3,
}

/// Operating limits (data for the FCS and the overlay; nothing enforces them in v0.1a).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Limits {
    /// Positive load limit, g.
    pub n_max: f64,
    /// Negative load limit, g.
    pub n_min: f64,
    /// FCS angle-of-attack limit, rad.
    pub alpha_limit_rad: f64,
    /// Maximum operating Mach.
    pub mmo: f64,
    /// Maximum operating speed, m/s.
    pub vmo_mps: f64,
}

/// An aircraft type as the flight model sees it: built once from content, then read-only.
#[derive(Clone, Debug)]
pub struct AircraftModel {
    /// Aerodynamics.
    pub aero: TableAero,
    /// Mass and fuel.
    pub mass: MassModel,
    /// Control surfaces.
    pub actuators: Actuators,
    /// Limits.
    pub limits: Limits,
    engines: Vec<EngineInstall>,
    atmosphere: Atmosphere,
    tick_dt_s: f64,
}

/// What drives the aircraft for one tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Commands {
    /// Surface commands.
    pub surfaces: Surfaces,
    /// Engine command (all engines).
    pub engine: EngineCommand,
}

/// Everything that evolves. Copy, so the tick's rollback copy costs nothing to take.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AircraftState {
    /// Rigid body (the CG).
    pub body: RigidState,
    /// Actual surface positions.
    pub surfaces: Surfaces,
    /// Engines; entries past the engine count stay as they are.
    pub engines: [EngineState; MAX_ENGINES],
    /// Fuel per tank.
    pub fuel: FuelLoad,
    /// Held pitch trim, elevator radians.
    pub trim_pitch_rad: f64,
    /// Mass and CG are frozen: no fuel is burned (the conformance test's fixed-mass points).
    pub mass_frozen: bool,
    /// Two faults in one tick: the state is the last good one and the aircraft must respawn.
    pub numerical_fault: bool,
}

/// What a tick did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StepOutcome {
    /// Faults detected this tick (0, 1 after a successful retry, 2 when it gave up).
    pub faults: u32,
}

/// Derived quantities for display and measurement. Not state: recomputed from it on demand.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Telemetry {
    /// Air data at the CG.
    pub air: AirData,
    /// Calibrated airspeed, m/s.
    pub cas_mps: f64,
    /// Height above MSL of the CG, m.
    pub height_msl_m: f64,
    /// Body-axis load factor (accelerometer `n_z`), g; +1 in level flight.
    pub nz_g: f64,
    /// Every non-gravitational force, body axes, N.
    pub force_body_n: DVec3,
    /// Total net thrust, N.
    pub thrust_n: f64,
    /// Total fuel flow, kg/s.
    pub fuel_flow_kgps: f64,
    /// Fuel on board, kg.
    pub fuel_kg: f64,
    /// Mass, kg.
    pub mass_kg: f64,
    /// Afterburner stages burning (the most of any engine).
    pub ab_stage: u8,
    /// Angle of attack within the warning margin of the lift peak.
    pub stall_warning: bool,
    /// An engine has flamed out.
    pub flameout: bool,
    /// The aerodynamic reference point in BF (where the camera and the model are anchored).
    pub ref_point_bf: DVec3,
}

/// Results of one evaluation of the forces at a stage.
#[derive(Clone, Copy, Debug)]
struct Stage {
    deriv: RigidDeriv,
    air: AirData,
    atmo: AtmoSample,
    height_msl_m: f64,
    force_body_n: DVec3,
    thrust_n: f64,
    fuel_flow_kgps: f64,
}

/// `d(alpha)/dt` from the air-relative body velocity and its rate, rad/s: `alpha = atan2(w, u)`.
fn alpha_rate(v: DVec3, vdot: DVec3) -> f64 {
    let d = v.x * v.x + v.z * v.z;
    if d < 1.0 {
        return 0.0;
    }
    (v.x * vdot.z - v.z * vdot.x) / d
}

impl AircraftModel {
    /// Builds the model of the aircraft behind `handle`, ticking every `tick_dt_s` seconds.
    ///
    /// # Errors
    /// [`ModelError`] if an entry is missing, of the wrong kind, malformed, or has more engines
    /// or tanks than the model supports.
    pub fn from_registry(
        registry: &Registry,
        handle: ContentId,
        tick_dt_s: f64,
    ) -> Result<Self, ModelError> {
        let ac = registry
            .aircraft(handle)
            .ok_or(ModelError::Missing("aircraft"))?;
        let aero_def = registry
            .aero(ac.aero.tables)
            .ok_or(ModelError::Missing("aero tables"))?;
        let sub_dt = tick_dt_s / f64::from(SUBSTEPS);
        let mut engines = Vec::new();
        for inst in &ac.engines {
            let def = registry
                .engine(inst.engine)
                .ok_or(ModelError::Missing("engine"))?;
            let model = Turbofan::from_def(def, sub_dt)?;
            let p = inst.position_m.map(f64::from);
            for _ in 0..inst.count {
                engines.push(EngineInstall {
                    model: model.clone(),
                    pos_body_m: DVec3::new(p[0], p[1], p[2]),
                });
            }
        }
        if engines.len() > MAX_ENGINES {
            return Err(ModelError::TooMany {
                what: "engines",
                found: engines.len(),
                max: MAX_ENGINES,
            });
        }
        let a = &ac.aero;
        Ok(Self {
            aero: TableAero::from_defs(ac, aero_def)?,
            mass: MassModel::from_def(&ac.mass)?,
            actuators: Actuators::from_fcs(&ac.fcs, sub_dt),
            limits: Limits {
                n_max: f64::from(a.n_max),
                n_min: f64::from(a.n_min),
                alpha_limit_rad: f64::from(ac.fcs.alpha_limit_deg).to_radians(),
                mmo: f64::from(a.mmo),
                vmo_mps: f64::from(a.vmo_mps),
            },
            engines,
            atmosphere: Atmosphere::new(),
            tick_dt_s,
        })
    }

    /// The engines.
    #[must_use]
    pub fn engines(&self) -> &[EngineInstall] {
        &self.engines
    }

    /// The most afterburner stages of any engine.
    #[must_use]
    pub fn ab_stages(&self) -> u8 {
        self.engines
            .iter()
            .map(|e| e.model.ab_stages())
            .max()
            .unwrap_or(0)
    }

    /// The tick length, s.
    #[must_use]
    pub fn tick_dt_s(&self) -> f64 {
        self.tick_dt_s
    }

    /// The atmosphere table.
    #[must_use]
    pub fn atmosphere(&self) -> &Atmosphere {
        &self.atmosphere
    }

    /// A state with the CG at `pos_bf` moving at `vel_bf` with attitude `att` and rates
    /// `omega_body`, carrying `fuel_kg` of fuel (loaded in reverse feed order), engines running
    /// steadily under `engine`, surfaces neutral and no trim.
    #[must_use]
    pub fn state(&self, body: RigidState, fuel_kg: f64, engine: EngineCommand) -> AircraftState {
        let mut engines = [EngineState::running_at(0.0); MAX_ENGINES];
        for (slot, e) in engines.iter_mut().zip(&self.engines) {
            *slot = e.model.steady(engine);
        }
        AircraftState {
            body,
            surfaces: Surfaces::NEUTRAL,
            engines,
            fuel: self.mass.load_for(fuel_kg),
            trim_pitch_rad: 0.0,
            mass_frozen: false,
            numerical_fault: false,
        }
    }

    /// Commands from the pilot's controls under the Manual law, advancing the held trim by one
    /// tick.
    pub fn manual_commands(&self, s: &mut AircraftState, controls: &PilotControls) -> Commands {
        let c = controls.sanitized();
        s.trim_pitch_rad = step_trim(s.trim_pitch_rad, &c, &self.actuators, self.tick_dt_s);
        let surfaces = ManualLaw.commands(
            &LawInput {
                controls: c,
                trim_pitch_rad: s.trim_pitch_rad,
            },
            &self.actuators,
        );
        Commands {
            surfaces,
            engine: EngineCommand::from_lever(c.throttle, self.ab_stages()),
        }
    }

    /// Advances the aircraft by one tick. The world is sampled through `env` at time `t_s` (the
    /// start of the tick). Allocates nothing.
    pub fn step_tick<E: FlightEnv>(
        &self,
        s: &mut AircraftState,
        cmd: &Commands,
        env: &E,
        t_s: f64,
    ) -> StepOutcome {
        if s.numerical_fault {
            return StepOutcome::default();
        }
        let saved = *s;
        if self.try_tick(s, cmd, env, t_s) {
            return StepOutcome { faults: 0 };
        }
        *s = saved;
        let calm = Commands {
            surfaces: Surfaces::NEUTRAL,
            ..*cmd
        };
        if self.try_tick(s, &calm, env, t_s) {
            return StepOutcome { faults: 1 };
        }
        *s = saved;
        s.numerical_fault = true;
        StepOutcome { faults: 2 }
    }

    /// One tick; false (with the state left half-advanced) on a fault.
    fn try_tick<E: FlightEnv>(
        &self,
        s: &mut AircraftState,
        cmd: &Commands,
        env: &E,
        t_s: f64,
    ) -> bool {
        let mass = self.mass.props(&s.fuel);
        let dt = self.tick_dt_s / f64::from(SUBSTEPS);
        for k in 0..SUBSTEPS {
            let t = t_s + f64::from(k) * dt;
            s.surfaces = self.actuators.step(&s.surfaces, &cmd.surfaces);
            let (air, atmo, _, _) = self.air_at(&s.body, env, t);
            let air = engine_conditions(&air, &atmo);
            let fuel_available = self.mass.fuel_kg(&s.fuel) > 0.0;
            for (e, state) in self.engines.iter().zip(s.engines.iter_mut()) {
                e.model.step(state, cmd.engine, &air, fuel_available);
            }
            // Thrust, surfaces and mass are held over the sub-step; everything else is
            // re-evaluated in each stage. The fuel burned is the first stage's flow.
            let held = *s;
            let mut fuel_flow = None;
            s.body = step_rk4(&s.body, dt, |st| {
                let stage = self.stage(st, &held, &mass, env, t);
                fuel_flow.get_or_insert(stage.fuel_flow_kgps);
                stage.deriv
            });
            if !s.mass_frozen {
                self.mass.draw(&mut s.fuel, fuel_flow.unwrap_or(0.0) * dt);
            }
            if !healthy(s) {
                return false;
            }
        }
        true
    }

    /// Air data, the local air, height MSL and the air-relative body velocity at a state.
    fn air_at<E: FlightEnv>(
        &self,
        st: &RigidState,
        env: &E,
        t_s: f64,
    ) -> (AirData, AtmoSample, f64, DVec3) {
        let height_msl_m = env.height_msl_m(st.pos_bf);
        let atmo = self.atmosphere.sample(height_msl_m, env.day());
        let vel_air_body = st
            .att
            .conjugate()
            .rotate(st.vel_bf - env.wind_bf(st.pos_bf, t_s));
        let air = AirData::compute(
            vel_air_body,
            st.omega_body,
            &atmo,
            self.aero.span_m(),
            self.aero.mac_m(),
        );
        (air, atmo, height_msl_m, vel_air_body)
    }

    /// Forces, moments and the state derivative at a stage state `st`, with the surfaces and
    /// engines of `held`.
    fn stage<E: FlightEnv>(
        &self,
        st: &RigidState,
        held: &AircraftState,
        mass: &MassProps,
        env: &E,
        t_s: f64,
    ) -> Stage {
        let (air, atmo, height_msl_m, vel_air_body) = self.air_at(st, env, t_s);
        let gravity_bf = env.gravity_bf(st.pos_bf);
        let cond = engine_conditions(&air, &atmo);
        let mut thrust_force = DVec3::ZERO;
        let mut thrust_moment = DVec3::ZERO;
        let mut thrust_n = 0.0;
        let mut fuel_flow_kgps = 0.0;
        for (e, state) in self.engines.iter().zip(held.engines.iter()) {
            let t = e.model.thrust_n(state, &cond);
            let f = DVec3::new(t, 0.0, 0.0);
            thrust_force += f;
            thrust_moment += (e.pos_body_m - mass.cg_body_m).cross(f);
            thrust_n += t;
            fuel_flow_kgps += e.model.fuel_flow_kgps(state, t, &cond);
        }

        let gravity_body = st.att.conjugate().rotate(gravity_bf);
        let input = AeroInput {
            air,
            alpha_dot_rps: 0.0,
            surfaces: held.surfaces,
            height_agl_m: height_msl_m - env.ground_height_m(st.pos_bf),
            cg_body_m: mass.cg_body_m,
        };
        let aero = self.aero.evaluate_with(&input, |aero_force| {
            // The air-relative velocity's rate in body axes (steady wind): F/m + g - w x v.
            let vdot = (aero_force + thrust_force) / mass.mass_kg + gravity_body
                - st.omega_body.cross(vel_air_body);
            alpha_rate(vel_air_body, vdot)
        });
        let force_body_n = aero.force_body_n + thrust_force;
        Stage {
            deriv: derivative(
                st,
                force_body_n,
                aero.moment_body_nm + thrust_moment,
                gravity_bf,
                mass,
            ),
            air,
            atmo,
            height_msl_m,
            force_body_n,
            thrust_n,
            fuel_flow_kgps,
        }
    }

    /// The rigid-body derivative and every non-gravitational force (body axes, N) at a state,
    /// with its surfaces, engines and fuel as they are. For trim, linearisation and test pilots.
    #[must_use]
    pub fn derivative<E: FlightEnv>(
        &self,
        s: &AircraftState,
        env: &E,
        t_s: f64,
    ) -> (RigidDeriv, DVec3) {
        let mass = self.mass.props(&s.fuel);
        let st = self.stage(&s.body, s, &mass, env, t_s);
        (st.deriv, st.force_body_n)
    }

    /// Display and measurement quantities at the current state.
    #[must_use]
    pub fn telemetry<E: FlightEnv>(&self, s: &AircraftState, env: &E, t_s: f64) -> Telemetry {
        let mass = self.mass.props(&s.fuel);
        let st = self.stage(&s.body, s, &mass, env, t_s);
        let ab_stage = self
            .engines
            .iter()
            .zip(s.engines.iter())
            .map(|(e, state)| e.model.ab_stage(state))
            .max()
            .unwrap_or(0);
        let alpha_deg = st.air.alpha_rad.to_degrees();
        Telemetry {
            air: st.air,
            cas_mps: cas_mps(st.atmo.p_pa, st.air.mach),
            height_msl_m: st.height_msl_m,
            nz_g: -st.force_body_n.z / (mass.mass_kg * G0),
            force_body_n: st.force_body_n,
            thrust_n: st.thrust_n,
            fuel_flow_kgps: st.fuel_flow_kgps,
            fuel_kg: self.mass.fuel_kg(&s.fuel),
            mass_kg: mass.mass_kg,
            ab_stage,
            stall_warning: alpha_deg
                > self.aero.alpha_peak_deg(st.air.mach) - STALL_WARNING_MARGIN_DEG,
            flameout: s
                .engines
                .iter()
                .take(self.engines.len())
                .any(|e| !e.running),
            ref_point_bf: self.ref_point_bf(s),
        }
    }

    /// The aerodynamic reference point in BF: `p_cg + R(q) (r_ref - r_cg)`.
    #[must_use]
    pub fn ref_point_bf(&self, s: &AircraftState) -> DVec3 {
        let cg = self.mass.props(&s.fuel).cg_body_m;
        s.body.pos_bf + s.body.att.rotate(self.aero.ref_point_body_m() - cg)
    }

    /// The CG position that puts the reference point at `ref_bf` for a fuel load and attitude.
    #[must_use]
    pub fn cg_for_ref_point(&self, ref_bf: DVec3, fuel: &FuelLoad, att: DQuat) -> DVec3 {
        let cg = self.mass.props(fuel).cg_body_m;
        ref_bf - att.rotate(self.aero.ref_point_body_m() - cg)
    }
}

fn engine_conditions(air: &AirData, atmo: &AtmoSample) -> EngineConditions {
    EngineConditions {
        rho_kgpm3: atmo.rho_kgpm3,
        t_k: atmo.t_k,
        mach: air.mach,
        alpha_rad: air.alpha_rad,
        tas_mps: air.tas_mps,
    }
}

/// The fault check (GDD 11.10).
fn healthy(s: &AircraftState) -> bool {
    s.body.is_finite()
        && s.surfaces.is_finite()
        && s.engines.iter().all(EngineState::is_finite)
        && s.fuel.iter().all(|f| f.is_finite())
        && s.body.omega_body.length() <= MAX_OMEGA_RPS
        && s.body.vel_bf.length() <= MAX_SPEED_MPS
}

#[cfg(test)]
mod tests;
