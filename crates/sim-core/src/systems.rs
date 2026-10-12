//! The systems that make up the v0.0b tick (GDD 61.4.3). Stages with nothing to do yet have no
//! system; they are still timed, at zero.

use crate::layout::core_state;
use crate::schedule::{Schedule, Stage, System, TickCtx};
use crate::snapshot::SnapshotSource;
use crate::time::sim_time;

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

/// Stage 5: the observer flies.
struct ObserverDynamics;

impl System for ObserverDynamics {
    fn name(&self) -> &'static str {
        "observer-dynamics"
    }
    fn stage(&self) -> Stage {
        Stage::Dynamics
    }
    fn run(&mut self, ctx: &mut TickCtx<'_>) {
        let w = &mut *ctx.world;
        w.observer.integrate(
            &w.input,
            &*ctx.svc.weather,
            &*ctx.svc.geo,
            sim_time(ctx.tick),
            ctx.dt,
        );
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

/// Stage 8: ground, boundary and sanity constraints on the observer.
struct ObserverConstraints;

impl System for ObserverConstraints {
    fn name(&self) -> &'static str {
        "observer-constraints"
    }
    fn stage(&self) -> Stage {
        Stage::Collision
    }
    fn run(&mut self, ctx: &mut TickCtx<'_>) {
        let w = &mut *ctx.world;
        w.observer.constrain(&*ctx.svc.geo, &mut w.counters);
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
        publish(ctx.world, ctx.tick + 1, core_state::RUNNING as u32);
    }
}

/// Publishes the end-of-tick snapshot and hands it to the replay sink.
pub(crate) fn publish(world: &mut crate::world::World, ticks_done: u64, core_state: u32) {
    write_snapshot(world, ticks_done, core_state);
    world.replay.record(ticks_done, world.snapshot.bytes());
}

/// Writes the snapshot for the state after `ticks_done` completed ticks. Used on its own for
/// lifecycle changes, which are not part of the replay.
pub(crate) fn write_snapshot(world: &mut crate::world::World, ticks_done: u64, core_state: u32) {
    world.snapshot.write(&SnapshotSource {
        tick: ticks_done,
        core_state,
        last_input_seq: world.input.last_seq(),
        input_t_ms: world.input.last_applied_t_ms(),
        observer: &world.observer,
        probe: &world.probe,
    });
    world.counters.snapshots = world.counters.snapshots.wrapping_add(1);
}

/// The schedule the core runs.
#[must_use]
pub fn default_schedule() -> Schedule {
    let mut s = Schedule::new();
    s.add(Box::new(Ingest));
    s.add(Box::new(ObserverDynamics));
    s.add(Box::new(ProbeDynamics));
    s.add(Box::new(ObserverConstraints));
    s.add(Box::new(PublishSnapshot));
    s
}
