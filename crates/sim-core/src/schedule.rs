//! The tick pipeline: eleven fixed stages, systems registered into them (GDD 61.4.2, 61.4.3).
//!
//! Order is part of determinism. Stages run in the order below; within a stage, systems run in
//! registration order. A system that is not due this tick (see [`Rate`]) is skipped.

use crate::profile::Profiler;
use crate::services::Services;
use crate::world::World;

/// The stages of one 60 Hz tick, in execution order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum Stage {
    /// Drain input frames and commands at the tick boundary.
    Ingest = 0,
    /// Weather, wind, time of day.
    Environment = 1,
    /// Tier assignment and spatial hash updates.
    Relevance = 2,
    /// Mission, squadron and pilot AI.
    Ai = 3,
    /// Control and dynamics.
    Dynamics = 4,
    /// Launches, missiles, guns, countermeasures.
    Weapons = 5,
    /// Radar, IRST, RWR, datalink.
    Sensing = 6,
    /// Collisions, hits, damage; the observer's ground and boundary constraints.
    Collision = 7,
    /// Fuel, stores, airbase operations.
    Systems = 8,
    /// Mission triggers, objectives, reinforcements.
    Director = 9,
    /// Event drain, snapshot, replay record.
    Publish = 10,
}

impl Stage {
    /// Number of stages.
    pub const COUNT: usize = 11;

    /// All stages in execution order.
    pub const ALL: [Self; Self::COUNT] = [
        Self::Ingest,
        Self::Environment,
        Self::Relevance,
        Self::Ai,
        Self::Dynamics,
        Self::Weapons,
        Self::Sensing,
        Self::Collision,
        Self::Systems,
        Self::Director,
        Self::Publish,
    ];

    /// Position in [`Stage::ALL`]; also the index into the profile block.
    #[must_use]
    pub const fn index(self) -> usize {
        self as usize
    }

    /// Short lower-case name, as shown in the debug overlay.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Ingest => "ingest",
            Self::Environment => "environment",
            Self::Relevance => "relevance",
            Self::Ai => "ai",
            Self::Dynamics => "dynamics",
            Self::Weapons => "weapons",
            Self::Sensing => "sensing",
            Self::Collision => "collision",
            Self::Systems => "systems",
            Self::Director => "director",
            Self::Publish => "publish",
        }
    }
}

/// How often a system runs, as a divisor of the master tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rate {
    /// Every tick (60 Hz).
    EveryTick,
    /// Every `n`th tick, starting at tick 0 (`n` of 0 is treated as 1).
    Divisor(u32),
}

impl Rate {
    /// True if a system with this rate runs on `tick`.
    #[must_use]
    pub fn due(self, tick: u64) -> bool {
        match self {
            Self::EveryTick => true,
            Self::Divisor(n) => tick.is_multiple_of(u64::from(n.max(1))),
        }
    }
}

/// What a system sees while it runs.
pub struct TickCtx<'a> {
    /// The mutable world.
    pub world: &'a mut World,
    /// Read-only services (geo, weather, ...).
    pub svc: &'a Services,
    /// Seconds per tick.
    pub dt: f64,
    /// Index of the tick being executed.
    pub tick: u64,
}

/// One unit of per-tick work.
pub trait System: Send {
    /// Stable name, used in profiles and logs.
    fn name(&self) -> &'static str;
    /// The stage this system belongs to.
    fn stage(&self) -> Stage;
    /// How often it runs.
    fn rate(&self) -> Rate {
        Rate::EveryTick
    }
    /// Does the work for one tick.
    fn run(&mut self, ctx: &mut TickCtx<'_>);
}

/// Systems in execution order.
#[derive(Default)]
pub struct Schedule {
    systems: Vec<Box<dyn System>>,
}

impl Schedule {
    /// An empty schedule.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a system after every system already in its stage (and any earlier stage).
    pub fn add(&mut self, system: Box<dyn System>) {
        let stage = system.stage();
        let at = self.systems.partition_point(|s| s.stage() <= stage);
        self.systems.insert(at, system);
    }

    /// Number of registered systems.
    #[must_use]
    pub fn len(&self) -> usize {
        self.systems.len()
    }

    /// True if nothing is registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.systems.is_empty()
    }

    /// `(stage, name)` of every system, in execution order.
    #[must_use]
    pub fn describe(&self) -> Vec<(Stage, &'static str)> {
        self.systems.iter().map(|s| (s.stage(), s.name())).collect()
    }

    /// Runs one tick. With a profiler, each system's wall time is added to its stage.
    pub fn run_tick(&mut self, ctx: &mut TickCtx<'_>, mut profiler: Option<&mut Profiler>) {
        for system in &mut self.systems {
            if !system.rate().due(ctx.tick) {
                continue;
            }
            if let Some(p) = profiler.as_deref_mut() {
                let t0 = p.now();
                system.run(ctx);
                let elapsed = p.now() - t0;
                p.add_stage_ms(system.stage(), elapsed);
            } else {
                system.run(ctx);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::stage as l;

    struct Recorder {
        name: &'static str,
        stage: Stage,
        rate: Rate,
    }

    impl System for Recorder {
        fn name(&self) -> &'static str {
            self.name
        }
        fn stage(&self) -> Stage {
            self.stage
        }
        fn rate(&self) -> Rate {
            self.rate
        }
        fn run(&mut self, ctx: &mut TickCtx<'_>) {
            // The tick counter in the world counters doubles as a log of what ran.
            ctx.world.counters.snapshots = ctx
                .world
                .counters
                .snapshots
                .wrapping_mul(31)
                .wrapping_add(u32::from(self.name.as_bytes()[0]));
        }
    }

    fn rec(name: &'static str, stage: Stage, rate: Rate) -> Box<dyn System> {
        Box::new(Recorder { name, stage, rate })
    }

    #[test]
    fn stage_numbers_match_the_shared_layout() {
        for (stage, expected) in Stage::ALL.into_iter().zip([
            l::INGEST,
            l::ENVIRONMENT,
            l::RELEVANCE,
            l::AI,
            l::DYNAMICS,
            l::WEAPONS,
            l::SENSING,
            l::COLLISION,
            l::SYSTEMS,
            l::DIRECTOR,
            l::PUBLISH,
        ]) {
            assert_eq!(stage.index(), expected, "{}", stage.name());
        }
        assert_eq!(Stage::COUNT, crate::layout::profile::STAGE_COUNT);
        assert!(Stage::ALL.windows(2).all(|w| w[0] < w[1]));
    }

    #[test]
    fn registration_sorts_by_stage_and_keeps_order_within_a_stage() {
        let mut s = Schedule::new();
        s.add(rec("publish", Stage::Publish, Rate::EveryTick));
        s.add(rec("dyn-b", Stage::Dynamics, Rate::EveryTick));
        s.add(rec("ingest", Stage::Ingest, Rate::EveryTick));
        s.add(rec("dyn-c", Stage::Dynamics, Rate::EveryTick));
        s.add(rec("collide", Stage::Collision, Rate::EveryTick));
        let order: Vec<_> = s.describe().into_iter().map(|(_, n)| n).collect();
        assert_eq!(order, ["ingest", "dyn-b", "dyn-c", "collide", "publish"]);
        assert_eq!(s.len(), 5);
        assert!(!s.is_empty());
    }

    #[test]
    fn divisors_run_on_multiples_of_the_tick_only() {
        assert!(Rate::EveryTick.due(7));
        assert!(Rate::Divisor(6).due(0));
        assert!(!Rate::Divisor(6).due(5));
        assert!(Rate::Divisor(6).due(12));
        assert!(Rate::Divisor(0).due(3), "zero is treated as every tick");
    }
}
