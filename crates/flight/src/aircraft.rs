//! One aircraft: the immutable model built from content, its state, and the 60 Hz tick
//! (GDD 11.2, 11.10, ADR-29).
//!
//! ```text
//! tick (ZOH: commands and mass properties held for the whole tick)
//!   saved = state                                    rollback copy
//!   2 x (dt = tick / 2):
//!     actuators -> engine spool, afterburner, failures (air data at the sub-step start)
//!     RK4 over (p, v, q, w): every stage re-evaluates height, gravity, air, alpha-dot, aero,
//!                            thrust and the ground's force on the gear
//!     renormalise q; burn fuel in feed order (unless mass is frozen)
//!     fault check: non-finite, |w| > 50 rad/s, V > 3000 m/s
//!     ground events: touchdown, hard landing, strut collapse, destruction (gear.rs)
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
use crate::gear::{self, GearHeld, GearModel, GroundEvents, GroundState, WheelCommands};
use crate::mass::{FuelLoad, MassModel, MassProps};
use crate::physical::G0;
use crate::structure::StructureInput;

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

/// Operating limits (data for the FCS, the structure model and the overlay).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Limits {
    /// Positive load limit, g.
    pub n_max: f64,
    /// Negative load limit, g.
    pub n_min: f64,
    /// Positive load limit with the gear extended, g.
    pub n_max_gear: f64,
    /// Positive load limit with the flaps out, g.
    pub n_max_flaps: f64,
    /// FCS angle-of-attack limit, rad.
    pub alpha_limit_rad: f64,
    /// Largest commanded roll rate, rad/s.
    pub roll_rate_max_rps: f64,
    /// Maximum operating Mach.
    pub mmo: f64,
    /// Maximum operating speed, m/s.
    pub vmo_mps: f64,
    /// Gear-extended speed limit, m/s.
    pub gear_limit_mps: f64,
    /// Flap-extended speed limit, m/s.
    pub flap_limit_mps: f64,
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
    /// Landing gear and airframe contact points.
    pub gear: GearModel,
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
    /// Brakes and nose-wheel steering.
    pub wheels: WheelCommands,
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
    /// Brakes and steering as applied (held over the tick).
    pub wheels: WheelCommands,
    /// What the ground model remembers: contacts, collapsed struts, weight on wheels.
    pub ground: GroundState,
    /// Mass and CG are frozen: no fuel is burned (the conformance test's fixed-mass points).
    pub mass_frozen: bool,
    /// Two faults in one tick: the state is the last good one and the aircraft must respawn.
    pub numerical_fault: bool,
}

/// What a tick did.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StepOutcome {
    /// Faults detected this tick (0, 1 after a successful retry, 2 when it gave up).
    pub faults: u32,
    /// What the ground did: touchdowns, hard landings, collapsed struts, destruction.
    pub events: GroundEvents,
    /// Body-axis load factor (accelerometer `n_z`) near the end of the tick, g.
    pub nz_g: f64,
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
    /// The part of [`Telemetry::force_body_n`] that is the ground's push on the gear and the
    /// airframe, body axes, N. Zero in the air.
    pub ground_force_body_n: DVec3,
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
    /// Weight on wheels (GDD 11.13.4).
    pub wow: bool,
    /// Height of the lowest wheel or contact point above the ground, m.
    pub agl_m: f64,
    /// Speed over the ground (horizontal), m/s.
    pub ground_speed_mps: f64,
    /// Vertical speed, m/s (climb positive).
    pub vs_mps: f64,
}

/// Results of one evaluation of the forces at a stage.
#[derive(Clone, Copy, Debug)]
struct Stage {
    deriv: RigidDeriv,
    air: AirData,
    atmo: AtmoSample,
    height_msl_m: f64,
    force_body_n: DVec3,
    ground_force_body_n: DVec3,
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
                n_max_gear: f64::from(a.n_max_by_config.gear),
                n_max_flaps: f64::from(a.n_max_by_config.flaps),
                alpha_limit_rad: f64::from(ac.fcs.alpha_limit_deg).to_radians(),
                roll_rate_max_rps: f64::from(ac.fcs.roll_rate_max_dps).to_radians(),
                mmo: f64::from(a.mmo),
                vmo_mps: f64::from(a.vmo_mps),
                gear_limit_mps: f64::from(a.gear_limit_mps),
                flap_limit_mps: f64::from(a.flap_limit_mps),
            },
            gear: GearModel::from_def(ac)?,
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

    /// How far the trailing-edge flaps are out, 0 (up) to 1 (full travel).
    #[must_use]
    pub fn flap_fraction(&self, surfaces: &Surfaces) -> f64 {
        let travel = self.actuators.tef.max;
        if travel.is_nan() || travel <= 0.0 {
            return 0.0;
        }
        (surfaces.tef_rad / travel).clamp(0.0, 1.0)
    }

    /// What the structure model reads from a state and its telemetry.
    #[must_use]
    pub fn structure_input(&self, s: &AircraftState, t: &Telemetry) -> StructureInput {
        StructureInput {
            nz_g: t.nz_g,
            cas_mps: t.cas_mps,
            mach: t.air.mach,
            gear_pos: s.surfaces.gear,
            flap_fraction: self.flap_fraction(&s.surfaces),
        }
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
            wheels: WheelCommands::NONE,
            ground: GroundState::AIRBORNE,
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
        let steer = self
            .gear
            .legs()
            .iter()
            .map(|l| l.steer_max_rad)
            .fold(0.0, f64::max);
        let mut wheels = WheelCommands::NONE;
        for (b, leg) in wheels.brake.iter_mut().zip(self.gear.legs()) {
            // Each leg follows the brake of its side; a leg on the centreline takes the harder.
            *b = if leg.pos_ext_body_m.y > 0.0 {
                c.brake_r
            } else if leg.pos_ext_body_m.y < 0.0 {
                c.brake_l
            } else {
                c.brake_l.max(c.brake_r)
            };
        }
        wheels.steer_rad = c.yaw * steer;
        Commands {
            surfaces,
            engine: EngineCommand::from_lever(c.throttle, self.ab_stages()),
            wheels,
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
        let mut out = StepOutcome::default();
        if self.try_tick(s, cmd, env, t_s, &mut out) {
            return out;
        }
        *s = saved;
        let calm = Commands {
            surfaces: Surfaces::NEUTRAL,
            ..*cmd
        };
        out = StepOutcome::default();
        if self.try_tick(s, &calm, env, t_s, &mut out) {
            out.faults = 1;
            return out;
        }
        *s = saved;
        s.numerical_fault = true;
        StepOutcome {
            faults: 2,
            ..StepOutcome::default()
        }
    }

    /// One tick; false (with the state left half-advanced) on a fault. Fills `out` with what the
    /// ground did and the load factor.
    fn try_tick<E: FlightEnv>(
        &self,
        s: &mut AircraftState,
        cmd: &Commands,
        env: &E,
        t_s: f64,
        out: &mut StepOutcome,
    ) -> bool {
        let mass = self.mass.props(&s.fuel);
        let dt = self.tick_dt_s / f64::from(SUBSTEPS);
        s.wheels = cmd.wheels.sanitized();
        s.ground.steer_rad = self.applied_steer_rad(&s.wheels);
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
            let pre = s.body;
            let mut first = None;
            s.body = step_rk4(&s.body, dt, |st| {
                let stage = self.stage(st, &held, &mass, env, t);
                first.get_or_insert((stage.fuel_flow_kgps, stage.force_body_n));
                stage.deriv
            });
            let (fuel_flow, force_body_n) = first.unwrap_or((0.0, DVec3::ZERO));
            if !s.mass_frozen {
                self.mass.draw(&mut s.fuel, fuel_flow * dt);
            }
            if !healthy(s) {
                return false;
            }
            let held_gear = GearHeld {
                gear_pos: s.surfaces.gear,
                wheels: s.wheels,
                collapsed: s.ground.collapsed,
            };
            let ev = gear::events(
                &self.gear,
                &pre,
                &s.body,
                &mass,
                &held_gear,
                &mut s.ground,
                env,
                dt,
            );
            out.events.merge(&ev);
            out.nz_g = -force_body_n.z / (mass.mass_kg * G0);
        }
        true
    }

    /// The nose-wheel angle the wheels can actually take: the command within the steering leg's
    /// range (no steering leg, no steering).
    fn applied_steer_rad(&self, wheels: &WheelCommands) -> f64 {
        let max = self
            .gear
            .legs()
            .iter()
            .map(|l| l.steer_max_rad)
            .fold(0.0, f64::max);
        wheels.steer_rad.clamp(-max, max)
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
        let height_agl_m = height_msl_m - env.ground_height_m(st.pos_bf);
        let input = AeroInput {
            air,
            alpha_dot_rps: 0.0,
            surfaces: held.surfaces,
            height_agl_m,
            cg_body_m: mass.cg_body_m,
        };
        let ground = gear::forces(
            &self.gear,
            st,
            mass,
            &GearHeld {
                gear_pos: held.surfaces.gear,
                wheels: held.wheels,
                collapsed: held.ground.collapsed,
            },
            env,
            height_agl_m,
        );
        let aero = self.aero.evaluate_with(&input, |aero_force| {
            // The air-relative velocity's rate in body axes (steady wind): F/m + g - w x v. The
            // ground pushes on the aircraft, so it is in the rate too (ADR-35).
            let vdot = (aero_force + thrust_force + ground.force_body_n) / mass.mass_kg
                + gravity_body
                - st.omega_body.cross(vel_air_body);
            alpha_rate(vel_air_body, vdot)
        });
        let force_body_n = aero.force_body_n + thrust_force + ground.force_body_n;
        Stage {
            deriv: derivative(
                st,
                force_body_n,
                aero.moment_body_nm + thrust_moment + ground.moment_body_nm,
                gravity_bf,
                mass,
            ),
            air,
            atmo,
            height_msl_m,
            force_body_n,
            ground_force_body_n: ground.force_body_n,
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
            ground_force_body_n: st.ground_force_body_n,
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
            wow: s.ground.wow,
            agl_m: gear::lowest_point_agl_m(&self.gear, &s.body, &mass, env),
            ground_speed_mps: {
                let up = env.up_bf(s.body.pos_bf);
                (s.body.vel_bf - up * s.body.vel_bf.dot(up)).length()
            },
            vs_mps: s.body.vel_bf.dot(env.up_bf(s.body.pos_bf)),
        }
    }

    /// The state of the aircraft at rest on its gear, with the struts carrying `weight - lift`:
    /// the CG height and pitch that balance, heading `heading_rad`, on the ground point
    /// `ground_bf`, carrying `fuel_kg`, engines idling steadily. `None` if there is no gear or no
    /// balance. See [`gear::static_pose`].
    #[must_use]
    pub fn static_state<E: FlightEnv>(
        &self,
        fuel_kg: f64,
        lift_n: f64,
        heading_rad: f64,
        ground_bf: DVec3,
        env: &E,
        engine: EngineCommand,
    ) -> Option<(AircraftState, gear::StaticPose)> {
        let fuel = self.mass.load_for(fuel_kg);
        let mass = self.mass.props(&fuel);
        let weight_n = mass.mass_kg * env.gravity_bf(ground_bf).length();
        let pose = gear::static_pose(
            &self.gear,
            &mass,
            weight_n - lift_n,
            heading_rad,
            ground_bf,
            env,
        )?;
        let mut state = self.state(pose.body, fuel_kg, engine);
        state.surfaces.gear = 1.0;
        Some((state, pose))
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
