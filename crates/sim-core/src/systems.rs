//! The systems that make up the tick (GDD 61.4.3). Stages with nothing to do yet have no system;
//! they are still timed, at zero.

use flight::aircraft::AircraftModel;
use flight::controls::PilotControls;

use crate::input::InputState;
use crate::layout::{axis, button, core_state};
use crate::ownship::{AircraftSlot, CRASH_HEIGHT_M, Ownship, OwnshipView, RESPAWN_TICKS};
use crate::schedule::{Schedule, Stage, System, TickCtx};
use crate::services::Services;
use crate::snapshot::SnapshotSource;
use crate::spawn::{SPAWN, spawn_trimmed};
use crate::time::sim_time;
use crate::world::{Counters, World};

/// Stage 1: apply queued and scripted input frames.
struct Ingest;

impl System for Ingest {
    fn name(&self) -> &'static str {
        "ingest-input"
    }
    fn stage(&self) -> Stage {
        Stage::Ingest
    }
    fn run(&mut self, ctx: &mut TickCtx<'_>) {
        ctx.world.input.ingest(ctx.tick);
    }
}

/// The pilot's controls from the validated input (no flap or airbrake controls yet).
fn pilot_controls(input: &InputState) -> PilotControls {
    PilotControls {
        pitch: input.axis(axis::PITCH),
        roll: input.axis(axis::ROLL),
        yaw: input.axis(axis::YAW),
        throttle: input.throttle_cmd(),
        trim_nose_up: input.held(button::TRIM_NOSE_UP),
        trim_nose_down: input.held(button::TRIM_NOSE_DOWN),
        flaps: 0.0,
        airbrake: 0.0,
        gear: 0.0,
        brake_l: input.axis(axis::BRAKE_L),
        brake_r: input.axis(axis::BRAKE_R),
    }
}

/// Respawns the aircraft in place, counting it.
pub(crate) fn respawn(
    model: &AircraftModel,
    svc: &Services,
    slot: &mut AircraftSlot,
    counters: &mut Counters,
) {
    if !spawn_trimmed(model, svc, &SPAWN, slot) {
        counters.trim_failures = counters.trim_failures.wrapping_add(1);
    }
    counters.respawns = counters.respawns.wrapping_add(1);
}

/// Stage 5: the ownship flies. The aircraft: input, the Manual law, then the flight model's tick
/// (two RK4 sub-steps); the spectator: its kinematic step.
struct OwnshipDynamics;

impl System for OwnshipDynamics {
    fn name(&self) -> &'static str {
        "ownship-dynamics"
    }
    fn stage(&self) -> Stage {
        Stage::Dynamics
    }
    fn run(&mut self, ctx: &mut TickCtx<'_>) {
        let w = &mut *ctx.world;
        let svc = ctx.svc;
        let t = sim_time(ctx.tick);
        match &mut w.ownship {
            Ownship::Spectator(o) => {
                o.integrate(&w.input, &*svc.weather, &*svc.geo, t, ctx.dt);
            }
            Ownship::Aircraft(slot) => {
                let Some(model) = svc.aircraft.as_ref() else {
                    return;
                };
                if w.input.pressed(button::RESET) {
                    respawn(model, svc, slot, &mut w.counters);
                }
                if slot.crashed || slot.state.numerical_fault {
                    return;
                }
                let cmd = model.manual_commands(&mut slot.state, &pilot_controls(&w.input));
                let out = model.step_tick(&mut slot.state, &cmd, &svc.flight_env(), t);
                w.counters.fdm_faults = w.counters.fdm_faults.wrapping_add(out.faults);
            }
        }
    }
}

/// Stage 5: the determinism probe bodies move.
struct ProbeDynamics;

impl System for ProbeDynamics {
    fn name(&self) -> &'static str {
        "probe-dynamics"
    }
    fn stage(&self) -> Stage {
        Stage::Dynamics
    }
    fn run(&mut self, ctx: &mut TickCtx<'_>) {
        ctx.world.probe.step(ctx.tick);
    }
}

/// Stage 8: the aircraft against the terrain (no gear yet, so contact is a crash) and the
/// respawn countdown; the spectator's ground, boundary and sanity clamps.
struct TerrainContact;

impl System for TerrainContact {
    fn name(&self) -> &'static str {
        "terrain-contact"
    }
    fn stage(&self) -> Stage {
        Stage::Collision
    }
    fn run(&mut self, ctx: &mut TickCtx<'_>) {
        let w = &mut *ctx.world;
        let svc = ctx.svc;
        match &mut w.ownship {
            Ownship::Spectator(o) => o.constrain(&*svc.geo, &mut w.counters),
            Ownship::Aircraft(slot) => {
                let Some(model) = svc.aircraft.as_ref() else {
                    return;
                };
                if !(slot.crashed || slot.state.numerical_fault) {
                    let r = model.ref_point_bf(&slot.state);
                    if r.z < svc.geo.ground_z_bf(r.x, r.y) + CRASH_HEIGHT_M {
                        slot.crashed = true;
                        slot.respawn_ticks = RESPAWN_TICKS;
                        w.counters.crashes = w.counters.crashes.wrapping_add(1);
                    }
                    return;
                }
                if slot.respawn_ticks == 0 {
                    // A numerical fault: start the countdown.
                    slot.respawn_ticks = RESPAWN_TICKS;
                }
                slot.respawn_ticks -= 1;
                if slot.respawn_ticks == 0 {
                    respawn(model, svc, slot, &mut w.counters);
                }
            }
        }
    }
}

/// Stage 11: write the snapshot.
struct PublishSnapshot;

impl System for PublishSnapshot {
    fn name(&self) -> &'static str {
        "publish-snapshot"
    }
    fn stage(&self) -> Stage {
        Stage::Publish
    }
    fn run(&mut self, ctx: &mut TickCtx<'_>) {
        publish(ctx.world, ctx.svc, ctx.tick + 1, core_state::RUNNING as u32);
    }
}

/// Publishes the end-of-tick snapshot and hands it to the replay sink.
pub(crate) fn publish(world: &mut World, svc: &Services, ticks_done: u64, core_state: u32) {
    write_snapshot(world, svc, ticks_done, core_state);
    world.replay.record(ticks_done, world.snapshot.bytes());
}

/// Writes the snapshot for the state after `ticks_done` completed ticks. Used on its own for
/// lifecycle changes, which are not part of the replay.
pub(crate) fn write_snapshot(world: &mut World, svc: &Services, ticks_done: u64, core_state: u32) {
    let ownship = OwnshipView::of(&world.ownship, svc, &world.input, sim_time(ticks_done));
    world.snapshot.write(&SnapshotSource {
        tick: ticks_done,
        core_state,
        last_input_seq: world.input.last_seq(),
        input_t_ms: world.input.last_applied_t_ms(),
        ownship: &ownship,
        probe: &world.probe,
    });
    world.counters.snapshots = world.counters.snapshots.wrapping_add(1);
}

/// The schedule the core runs.
#[must_use]
pub fn default_schedule() -> Schedule {
    let mut s = Schedule::new();
    s.add(Box::new(Ingest));
    s.add(Box::new(OwnshipDynamics));
    s.add(Box::new(ProbeDynamics));
    s.add(Box::new(TerrainContact));
    s.add(Box::new(PublishSnapshot));
    s
}
