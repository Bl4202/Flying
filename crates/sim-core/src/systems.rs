//! The systems that make up the tick (GDD 61.4.3). Stages with nothing to do yet have no system;
//! they are still timed, at zero.

use flight::aircraft::AircraftModel;
use flight::gear::{CrashCause, GroundEvents};

use crate::layout::{button, core_state, crash_cause, spawn_request};
use crate::ownship::{AircraftSlot, Ownship, OwnshipView, RESPAWN_TICKS};
use crate::pilot;
use crate::schedule::{Schedule, Stage, System, TickCtx};
use crate::services::Services;
use crate::session::Spawn;
use crate::snapshot::SnapshotSource;
use crate::spawn::spawn;
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
        // A scripted pilot has no throttle lever to synchronise: it acknowledges every spawn.
        if let Ownship::Aircraft(slot) = &ctx.world.ownship {
            ctx.world.input.set_script_spawn_ack(slot.spawn_seq);
        }
        ctx.world.input.ingest(ctx.tick);
    }
}

/// Respawns the aircraft in place at `kind`, counting it.
pub(crate) fn respawn(
    model: &AircraftModel,
    svc: &Services,
    kind: Spawn,
    slot: &mut AircraftSlot,
    counters: &mut Counters,
) {
    if !spawn(model, svc, kind, slot) {
        counters.trim_failures = counters.trim_failures.wrapping_add(1);
    }
    counters.respawns = counters.respawns.wrapping_add(1);
}

/// Where the RESET press respawns: the frame's request, or else the session's spawn.
fn spawn_for_reset(request: u8, session: Spawn) -> Spawn {
    match usize::from(request) {
        spawn_request::RUNWAY => Spawn::Runway,
        spawn_request::AIR => Spawn::Air,
        _ => session,
    }
}

/// Stage 5: the ownship flies. The aircraft: levers, controls, the flight control system, then
/// the flight model's tick (two RK4 sub-steps); the spectator: its kinematic step.
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
                let (Some(model), Some(fcs_model)) = (svc.aircraft.as_ref(), svc.fcs.as_ref())
                else {
                    return;
                };
                if w.input.pressed(button::RESET) {
                    let kind = spawn_for_reset(w.input.spawn_req(), slot.session.spawn);
                    respawn(model, svc, kind, slot, &mut w.counters);
                }
                slot.events = GroundEvents::NONE;
                if slot.crashed || slot.state.numerical_fault {
                    return;
                }
                pilot::fly(slot, model, fcs_model, svc, &w.input, &mut w.counters, t);
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

/// Stage 8: what the ground did to the aircraft (touchdowns, hard landings, destruction) and the
/// respawn countdown; the spectator's ground, boundary and sanity clamps.
struct TerrainContact;

/// An airframe with no gear and no contact points meets the ground when its reference point does.
fn gearless_impact(model: &AircraftModel, svc: &Services, slot: &AircraftSlot) -> bool {
    if !model.gear.is_empty() {
        return false;
    }
    let r = model.ref_point_bf(&slot.state);
    r.z < svc.geo.ground_z_bf(r.x, r.y)
}

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
                    let ev = core::mem::replace(&mut slot.events, GroundEvents::NONE);
                    let c = &mut w.counters;
                    c.touchdowns = c.touchdowns.wrapping_add(ev.touchdowns);
                    c.hard_landings = c.hard_landings.wrapping_add(ev.hard_landings);
                    let cause = ev.destroyed.map(CrashCause::code).or_else(|| {
                        gearless_impact(model, svc, slot)
                            .then_some(crash_cause::AIRFRAME_IMPACT as u8)
                    });
                    if let Some(code) = cause {
                        slot.crashed = true;
                        slot.crash_cause = code;
                        slot.respawn_ticks = RESPAWN_TICKS;
                        c.crashes = c.crashes.wrapping_add(1);
                    }
                    return;
                }
                if slot.respawn_ticks == 0 {
                    // A numerical fault: start the countdown.
                    slot.respawn_ticks = RESPAWN_TICKS;
                }
                slot.respawn_ticks -= 1;
                if slot.respawn_ticks == 0 {
                    let kind = slot.session.spawn;
                    respawn(model, svc, kind, slot, &mut w.counters);
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
    let ownship = OwnshipView::of(&world.ownship, svc, sim_time(ticks_done));
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
