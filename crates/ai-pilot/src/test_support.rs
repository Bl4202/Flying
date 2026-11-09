//! The core pack, the Kestrel and a cockpit that flies it through the real control system, for the
//! unit tests.

use std::sync::OnceLock;

use content::{Registry, SkillDef};
use fcs::tester::{Levers, Presses};
use fcs::{Assist, FcsModel, FcsState, PilotInput};
use flight::actuators::Surfaces;
use flight::aircraft::{AircraftModel, AircraftState};
use flight::controls::PilotControls;
use flight::engine::EngineCommand;
use flight::env::{FlightEnv, SphereSea};
use flight::frames::{Euler, attitude};
use flight::trim::{TrimProblem, TrimSetup, guess_level, solve, state_of};
use simmath::math::DVec3;

use crate::maneuver::Airframe;

pub(crate) const DT: f64 = 1.0 / 60.0;

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
pub(crate) fn kestrel() -> &'static (AircraftModel, FcsModel) {
    static K: OnceLock<(AircraftModel, FcsModel)> = OnceLock::new();
    K.get_or_init(|| {
        let r = registry();
        let h = r.aircraft_handles().next().expect("an aircraft");
        let model = AircraftModel::from_registry(r, h, DT).expect("model");
        let fcs = FcsModel::from_registry(r, h, &model).expect("fcs");
        (model, fcs)
    })
}

/// The skill of a rank.
pub(crate) fn skill_def(rank: u8) -> &'static SkillDef {
    registry()
        .skill_by_rank(rank)
        .expect("a skill of that rank")
}

/// A Kestrel in flight or on the ground, its levers and the world.
pub(crate) struct Cockpit {
    pub(crate) ac: AircraftState,
    pub(crate) fs: FcsState,
    pub(crate) levers: Levers,
    pub(crate) world: SphereSea,
    pub(crate) tick: u64,
    pub(crate) destroyed: bool,
}

impl Cockpit {
    /// Level and trimmed, clean, at `pos` (BF, m) on `heading_rad` at `tas` m/s, `mass_kg`.
    pub(crate) fn air(
        assist: Assist,
        pos: DVec3,
        heading_rad: f64,
        tas: f64,
        mass_kg: f64,
    ) -> Self {
        let (model, _) = kestrel();
        let world = SphereSea::default();
        let setup = TrimSetup {
            pos_bf: pos,
            fuel: model.mass.load_for(mass_kg - model.mass.empty_kg()),
            config: Surfaces::NEUTRAL,
        };
        let guess = guess_level(model, &setup, tas, 1.0, &world);
        let t = solve(model, &TrimProblem::level(), &setup, guess, &world);
        assert!(t.converged, "the condition trims");
        let mut ac = state_of(model, &setup, &t.point, &world);
        // Turn the trimmed state onto the heading.
        let up = world.up_bf(pos);
        let e = flight::frames::euler(up, ac.body.att);
        let att = attitude(up, Euler { heading_rad, ..e });
        let vel_body = ac.body.att.conjugate().rotate(ac.body.vel_bf);
        ac.body.att = att;
        ac.body.vel_bf = att.rotate(vel_body);
        let mut fs = FcsState::new(assist, false);
        fs.last_cmd = [
            ac.surfaces.elevator_rad,
            ac.surfaces.aileron_rad,
            ac.surfaces.rudder_rad,
        ];
        Self {
            ac,
            fs,
            levers: Levers::default(),
            world,
            tick: 0,
            destroyed: false,
        }
    }

    /// At rest on the ground at the origin facing north, take-off flaps, gear down, idle, the
    /// parking brake set.
    pub(crate) fn runway(assist: Assist, mass_kg: f64) -> Self {
        let (model, _) = kestrel();
        let world = SphereSea::default();
        let (mut ac, pose) = model
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
        // At rest on the struts, as sim-core's runway spawn leaves it.
        for (c, &x) in ac.ground.in_contact.iter_mut().zip(&pose.compression_m) {
            *c = x > 0.0;
        }
        ac.ground.wow = true;
        ac.ground.air_time_s = 0.0;
        let levers = Levers {
            gear_down: true,
            flap_detent: 1,
            parking_brake: true,
        };
        Self {
            ac,
            fs: FcsState::new(assist, true),
            levers,
            world,
            tick: 0,
            destroyed: false,
        }
    }

    pub(crate) fn t(&self) -> f64 {
        self.tick as f64 * DT
    }

    /// The airframe as the brains see it.
    pub(crate) fn airframe(&self) -> Airframe<'_, SphereSea> {
        let (model, fcs) = kestrel();
        Airframe {
            model,
            fcs,
            ac: &self.ac,
            fs: &self.fs,
            env: &self.world,
            t_s: self.t(),
        }
    }

    /// One tick with these controls and presses: the levers as `sim-core::pilot` works them, the
    /// control system, the airframe.
    pub(crate) fn step(&mut self, c: &PilotControls, p: Presses) {
        let (model, fcs) = kestrel();
        if p.gear {
            if !self.levers.gear_down {
                self.levers.gear_down = true;
            } else if !(self.ac.ground.wow || self.fs.on_ground) {
                self.levers.gear_down = false;
            }
        }
        if p.flaps_down {
            self.levers.flap_detent = (self.levers.flap_detent + 1).min(2);
        }
        if p.flaps_up {
            self.levers.flap_detent = self.levers.flap_detent.saturating_sub(1);
        }
        if p.parking_brake {
            self.levers.parking_brake = !self.levers.parking_brake;
        }
        let park = if self.levers.parking_brake { 1.0 } else { 0.0 };
        let controls = PilotControls {
            gear: if self.levers.gear_down { 1.0 } else { 0.0 },
            flaps: [0.0, 0.5, 1.0][usize::from(self.levers.flap_detent)],
            brake_l: c.brake_l.max(park),
            brake_r: c.brake_r.max(park),
            ..*c
        };
        let t = self.t();
        let input = PilotInput {
            controls,
            ..PilotInput::default()
        };
        let cmd = fcs.step(model, &mut self.ac, &mut self.fs, &input, &self.world, t);
        let out = model.step_tick(&mut self.ac, &cmd, &self.world, t);
        self.destroyed |= out.events.destroyed.is_some();
        self.tick += 1;
    }

    /// Height above MSL, m.
    pub(crate) fn height(&self) -> f64 {
        self.world.height_msl_m(self.ac.body.pos_bf)
    }

    /// Heading of the velocity, rad (0 north, clockwise).
    pub(crate) fn track(&self) -> f64 {
        let v = self.ac.body.vel_bf;
        simmath::dmath::atan2(v.x, v.y)
    }
}
