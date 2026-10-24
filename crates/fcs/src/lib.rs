//! The flight control system (GDD 11.11, ADR-34): the four assist levels and what sits between the
//! pilot's controls and the surfaces.
//!
//! One [`FcsModel`] per airframe holds the gains (the `fcs/1` entry the aircraft refers to). Its
//! [`FcsModel::step`] runs once per 60 Hz tick, before the airframe's tick, and its output is held
//! over both RK4 sub-steps. The state it keeps ([`FcsState`]) is `Copy` and hashed.
//!
//! ```text
//! pilot controls ─▶ (mouse-aim) ─▶ law ─▶ angular acceleration wanted ─▶ inversion ─▶ surfaces
//!                                   │                                        ▲
//!            Full / Standard / Reduced                    the airframe's own control power
//!                                   │
//!      weight on wheels ─▶ ground law ─▶ blended with the flight law over 0.5 s / 2 s
//! ```
//!
//! Manual is the stick moving the surfaces (`flight::controls::ManualLaw`) and nothing else.
//! Every other level inverts the angular acceleration of the airframe incrementally
//! ([`inverse`]); the laws differ in what the stick commands and what limits it.
//!
//! Layering (ADR-29, ADR-34): this crate depends on `flight`, `simmath` and `content`. It names no
//! content entry: the gains come through the aircraft's own reference.
#![deny(missing_docs)]

pub mod full;
pub mod ground;
pub mod inverse;
pub mod model;
pub mod mouse_aim;
pub mod recover;
pub mod reduced;
pub mod sense;
pub mod standard;
pub mod state;

use flight::actuators::Surfaces;
use flight::aircraft::{AircraftModel, AircraftState, Commands};
use flight::controls::{PilotControls, step_trim};
use flight::engine::EngineCommand;
use flight::env::FlightEnv;
use flight::gear::WheelCommands;

pub use model::FcsModel;
pub use mouse_aim::Aim;
pub use state::{Assist, FcsState, Limiting, Mode, RecoverPhase};

use inverse::Wanted;
use sense::Sensed;

/// What the pilot (or the virtual tester, or the AI) gives the control system in a tick.
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent buttons and conditions"
)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PilotInput {
    /// The stick, pedals, throttle, brakes and levers.
    pub controls: PilotControls,
    /// A pointing direction (mouse-aim); Full and Standard only.
    pub aim: Option<Aim>,
    /// The recovery button is held.
    pub recover: bool,
    /// The pilot is out (G-LOC): the controls are already zero and Full holds the wings level.
    pub gloc: bool,
    /// Automatic ground-collision avoidance is armed (reserved: the feature is off in v0.1b).
    pub auto_gcas: bool,
}

/// The surface commands of a blend of two sets: `(1 - w) a + w b`.
fn mix(a: [f64; 3], b: [f64; 3], w: f64) -> [f64; 3] {
    core::array::from_fn(|i| (1.0 - w) * a[i] + w * b[i])
}

/// Direct stick-to-surface commands (the Manual law), the fallback where the dynamic pressure is
/// too low for an inversion to mean anything.
fn direct(model: &AircraftModel, ac: &AircraftState, c: &PilotControls) -> [f64; 3] {
    let a = &model.actuators;
    let auth = |min: f64, max: f64| min.abs().max(max.abs());
    [
        (ac.trim_pitch_rad - c.pitch * auth(a.elevator.min, a.elevator.max))
            .clamp(a.elevator.min, a.elevator.max),
        c.roll * auth(a.aileron.min, a.aileron.max),
        -c.yaw * auth(a.rudder.min, a.rudder.max),
    ]
}

impl FcsModel {
    /// Changes the assist level. The new law starts from the present state (its filters and
    /// integrators are set at the next tick) and the surface command blends from the last one
    /// over `assist_blend_s`, so nothing jumps.
    pub fn switch_assist(&self, fs: &mut FcsState, to: Assist) {
        if to == fs.assist {
            return;
        }
        fs.assist = to;
        fs.init_pending = true;
        fs.blend_left_s = self.assist_blend_s;
        fs.blend_from = fs.last_cmd;
        fs.recover = RecoverPhase::Idle;
    }

    /// One tick of the control system: the commands for the airframe's next tick.
    ///
    /// `ac` is advanced only in its pitch trim (the trim switches); everything else in it is read.
    /// Allocates nothing.
    pub fn step<E: FlightEnv>(
        &self,
        model: &AircraftModel,
        ac: &mut AircraftState,
        fs: &mut FcsState,
        input: &PilotInput,
        env: &E,
        t_s: f64,
    ) -> Commands {
        let c = input.controls.sanitized();
        let engine = EngineCommand::from_lever(c.throttle, model.ab_stages());
        let sense = model.sense(ac, env, t_s);
        let s = Sensed::of(model, ac, &sense, env);
        ground::update_blend(self, fs, ac.ground.wow);
        fs.limiting = Limiting::default();
        // On the wheels a fast yaw is a turn, not a departure.
        fs.departed = !ac.ground.wow && !fs.on_ground && recover::departed(&s);

        let mut commands = if fs.assist == Assist::Manual {
            if fs.init_pending {
                fs.init_pending = false;
                if fs.blend_left_s > 0.0 {
                    // Switched to Manual in flight: set the trim so that the stick where it is now
                    // gives the elevator the last command had.
                    let e = &model.actuators.elevator;
                    let auth = e.min.abs().max(e.max.abs());
                    ac.trim_pitch_rad = (fs.blend_from[0] + c.pitch * auth).clamp(e.min, e.max);
                }
            }
            let mut cmd = model.manual_commands(ac, &c);
            self.blend_after_switch(fs, &mut cmd.surfaces);
            if input.recover
                && let Some(d) = recover::anti_spin(model, fs, &s)
            {
                cmd.surfaces.elevator_rad = d[0];
                cmd.surfaces.aileron_rad = d[1];
                cmd.surfaces.rudder_rad = d[2];
            } else if !input.recover {
                fs.recover = RecoverPhase::Idle;
            }
            cmd
        } else {
            ac.trim_pitch_rad = step_trim(ac.trim_pitch_rad, &c, &model.actuators, self.tick_dt_s);
            self.assisted(
                model,
                ac,
                fs,
                input,
                &c,
                &sense,
                &s,
                engine,
                env.up_bf(ac.body.pos_bf),
            )
        };
        self.interlocks(model, fs, &s, &mut commands);
        fs.last_cmd = [
            commands.surfaces.elevator_rad,
            commands.surfaces.aileron_rad,
            commands.surfaces.rudder_rad,
        ];
        fs.mode = if input.gloc {
            Mode::Gloc
        } else if fs.recover != RecoverPhase::Idle {
            Mode::Recover
        } else {
            ground::mode_for(fs)
        };
        commands
    }

    /// Flaps blow back above their speed and the gear cannot be raised on the ground.
    fn interlocks(&self, model: &AircraftModel, fs: &FcsState, s: &Sensed, cmd: &mut Commands) {
        let _ = model;
        if fs.on_ground {
            cmd.surfaces.gear = 1.0;
        }
        if fs.assist != Assist::Manual && s.cas > self.limits.flap_limit_mps {
            cmd.surfaces.tef_rad = 0.0;
        }
    }

    /// The Full, Standard and Reduced laws with the ground law blended in.
    #[allow(clippy::too_many_arguments, reason = "one tick of one law")]
    fn assisted(
        &self,
        model: &AircraftModel,
        ac: &AircraftState,
        fs: &mut FcsState,
        input: &PilotInput,
        c: &PilotControls,
        sense: &flight::aircraft::Sense,
        s: &Sensed,
        engine: EngineCommand,
        up: simmath::math::DVec3,
    ) -> Commands {
        let a = &model.actuators;
        let mut stick = *c;
        if let Some(aim) = input.aim
            && matches!(fs.assist, Assist::Full | Assist::Standard)
        {
            let (p, r) = mouse_aim::sticks(model, self, fs, s, ac.body.att, up, aim);
            stick.pitch = p;
            stick.roll = r;
        }
        if fs.ground_weight >= 1.0 {
            // Wheels down: the flight law's memory restarts when the weight comes off.
            fs.init_pending = true;
        } else if fs.init_pending {
            Self::init_flight_law(fs, s);
        }

        // From the first touch of a wheel until the ground law has the aircraft, the flight law is
        // frozen where it was: the jolt of the impact is not a disturbance for it to cancel.
        let touching = ac.ground.wow && !fs.on_ground;
        let holding = touching || (fs.on_ground && fs.ground_weight < 1.0);
        if holding && !fs.holding {
            fs.held = fs.last_cmd;
        }
        fs.holding = holding;
        let flight = if fs.ground_weight >= 1.0 {
            None
        } else if holding {
            Some(fs.held)
        } else {
            Some(self.flight_surfaces(model, ac, fs, input, &stick, sense, s))
        };
        let ground = (fs.ground_weight > 0.0)
            .then(|| ground::command(model, self, ac, c, s.tel.ground_speed_mps, s.rates.y));
        let (surf, wheels) = match (flight, ground) {
            (Some(f), Some(g)) => (
                mix(f, g.surfaces, fs.ground_weight),
                WheelCommands {
                    brake: g.wheels.brake,
                    steer_rad: g.wheels.steer_rad * fs.ground_weight,
                },
            ),
            (Some(f), None) => (
                f,
                WheelCommands {
                    brake: ground::brakes(model, c),
                    steer_rad: 0.0,
                },
            ),
            (None, Some(g)) => (g.surfaces, g.wheels),
            (None, None) => (direct(model, ac, c), WheelCommands::NONE),
        };
        let mut surfaces = Surfaces {
            elevator_rad: surf[0],
            aileron_rad: surf[1],
            rudder_rad: surf[2],
            lef_rad: 0.0,
            tef_rad: c.flaps * a.tef.max,
            airbrake: c.airbrake,
            gear: c.gear,
        };
        self.blend_after_switch(fs, &mut surfaces);
        Commands {
            surfaces,
            engine,
            wheels,
        }
    }

    /// While the blend after an assist switch runs, mixes the three primary surface commands with
    /// the last ones of the law that was flying.
    fn blend_after_switch(&self, fs: &mut FcsState, surfaces: &mut Surfaces) {
        if fs.blend_left_s > 0.0 && self.assist_blend_s > 0.0 {
            let w = (fs.blend_left_s / self.assist_blend_s).clamp(0.0, 1.0);
            fs.blend_left_s = (fs.blend_left_s - self.tick_dt_s).max(0.0);
            let m = mix(
                [
                    surfaces.elevator_rad,
                    surfaces.aileron_rad,
                    surfaces.rudder_rad,
                ],
                fs.blend_from,
                w,
            );
            surfaces.elevator_rad = m[0];
            surfaces.aileron_rad = m[1];
            surfaces.rudder_rad = m[2];
        } else {
            fs.blend_left_s = 0.0;
        }
    }

    /// Starts the flight law's filters and integrators from the present state.
    fn init_flight_law(fs: &mut FcsState, s: &Sensed) {
        let (sa, ca) = (simmath::dmath::sin(s.alpha), simmath::dmath::cos(s.alpha));
        fs.init_pending = false;
        fs.n_filt = s.n_w;
        fs.n_cmd = s.n_w;
        fs.n_int = 0.0;
        fs.qbar_prev = s.qbar_pa;
        fs.p_filt = s.rates.x * ca + s.rates.z * sa;
        fs.p_int = 0.0;
        fs.q_filt = s.rates.y;
        fs.gamma_ref = s.gamma;
        fs.r_prev = s.rates.z;
        fs.r_wash = 0.0;
        fs.aim_pitch = 0.0;
        fs.aim_roll = 0.0;
    }

    /// The flight law's surface commands `(elevator, aileron, rudder)`, rad.
    #[allow(clippy::too_many_arguments, reason = "one tick of one law")]
    fn flight_surfaces(
        &self,
        model: &AircraftModel,
        ac: &AircraftState,
        fs: &mut FcsState,
        input: &PilotInput,
        stick: &PilotControls,
        sense: &flight::aircraft::Sense,
        s: &Sensed,
    ) -> [f64; 3] {
        let mut lim = Limiting::default();
        let level = matches!(fs.assist, Assist::Full | Assist::Standard);
        let wanted: Wanted = if input.recover && level {
            recover::wanted(
                model,
                self,
                fs,
                s,
                sense,
                self.alpha_limit(fs.assist),
                &mut lim,
            )
        } else if input.recover {
            if let Some(d) = recover::anti_spin(model, fs, s) {
                fs.limiting = lim;
                return d;
            }
            self.law(model, fs, s, sense, stick, input.gloc, &mut lim)
        } else {
            if level {
                fs.recover = RecoverPhase::Idle;
            }
            self.law(model, fs, s, sense, stick, input.gloc, &mut lim)
        };
        fs.limiting = lim;
        inverse::surfaces(model, sense, &ac.surfaces, &wanted)
    }

    /// The angle-of-attack limit a law holds, rad.
    fn alpha_limit(&self, assist: Assist) -> f64 {
        match assist {
            Assist::Full => self.limits.alpha_limit_rad - self.full.alpha_margin_rad,
            _ => self.limits.alpha_limit_rad,
        }
    }

    #[allow(clippy::too_many_arguments, reason = "one tick of one law")]
    fn law(
        &self,
        model: &AircraftModel,
        fs: &mut FcsState,
        s: &Sensed,
        sense: &flight::aircraft::Sense,
        stick: &PilotControls,
        gloc: bool,
        lim: &mut Limiting,
    ) -> Wanted {
        match fs.assist {
            Assist::Full => full::wanted(model, self, fs, s, sense, stick, gloc, lim),
            Assist::Reduced => reduced::wanted(model, self, fs, s, sense, stick),
            _ => standard::wanted(model, self, fs, s, sense, stick, lim),
        }
    }
}

#[cfg(test)]
pub(crate) mod test_content {
    //! The Kestrel and its control gains, baked once for the unit tests.

    use std::sync::OnceLock;

    use content::Registry;
    use flight::aircraft::AircraftModel;

    use crate::FcsModel;

    /// The baked `content/core` pack.
    pub(crate) fn registry() -> &'static Registry {
        static PACK: OnceLock<Registry> = OnceLock::new();
        PACK.get_or_init(|| {
            let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content/core");
            let baked = content_bake::bake::bake_dir(&dir)
                .unwrap_or_else(|d| panic!("the core pack bakes: {d:?}"));
            Registry::from_cpack(&baked.cpack).expect("the baked pack reads back")
        })
    }

    /// The Kestrel airframe and its control system.
    pub(crate) fn kestrel() -> (AircraftModel, FcsModel) {
        let r = registry();
        let h = r.aircraft_handles().next().expect("an aircraft");
        let model = AircraftModel::from_registry(r, h, 1.0 / 60.0).expect("model");
        let fcs = FcsModel::from_registry(r, h, &model).expect("fcs");
        (model, fcs)
    }
}
