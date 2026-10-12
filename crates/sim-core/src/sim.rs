//! The simulation core: tick counter, lifecycle, the tick pipeline and its inputs and outputs.

use core::fmt;

use content::{CpackError, Registry};

use crate::hash::{StateHasher, SubsystemHashes};
use crate::input::{ErraticPilot, FRAME_BYTES, InputFrame};
use crate::layout::{core_state, input_script, profile};
use crate::observer::Observer;
use crate::probe::ProbeWorld;
use crate::profile::{ClockFn, Profiler, counter_array, write_counters};
use crate::rng::RngStreams;
use crate::schedule::{Schedule, TickCtx};
use crate::seams::replay::{NullReplaySink, ReplaySink};
use crate::services::Services;
use crate::snapshot::SnapshotBuffer;
use crate::systems::{default_schedule, write_snapshot};
use crate::terrain;
use crate::time::SIM_DT;
use crate::world::{Counters, World};

/// Lifecycle state of the core (GDD 61.4.1). `Idle -> Running -> Paused -> Running -> Ended` is
/// wired; loading content and missions arrive with the mission system.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum CoreState {
    /// Created, not stepping.
    Idle = 0,
    /// A mission is loading.
    Loading = 1,
    /// Stepping.
    Running = 2,
    /// Paused.
    Paused = 3,
    /// Finished.
    Ended = 4,
}

impl CoreState {
    /// The number exported over the ABI (see [`crate::layout::core_state`]).
    #[must_use]
    pub const fn code(self) -> u32 {
        self as u32
    }
}

const _: () = {
    assert!(CoreState::Idle as usize == core_state::IDLE);
    assert!(CoreState::Loading as usize == core_state::LOADING);
    assert!(CoreState::Running as usize == core_state::RUNNING);
    assert!(CoreState::Paused as usize == core_state::PAUSED);
    assert!(CoreState::Ended as usize == core_state::ENDED);
};

/// Why a content pack was not loaded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LoadContentError {
    /// Content can only be loaded before the core starts.
    NotIdle,
    /// The bytes are not a valid pack.
    Invalid(CpackError),
}

impl fmt::Display for LoadContentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotIdle => write!(f, "content can only be loaded while the core is idle"),
            Self::Invalid(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for LoadContentError {}

/// The deterministic simulation.
pub struct SimCore {
    tick: u64,
    state: CoreState,
    rng: RngStreams,
    world: World,
    schedule: Schedule,
    svc: Services,
    profiler: Option<Profiler>,
    profile_block: [u8; profile::SIZE],
    terrain: Option<Box<[u8]>>,
}

impl fmt::Debug for SimCore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SimCore")
            .field("tick", &self.tick)
            .field("state", &self.state)
            .field("seed", &self.rng.master())
            .finish_non_exhaustive()
    }
}

impl SimCore {
    /// Creates an idle core from a master seed, with the default services.
    #[must_use]
    pub fn new(seed: u64) -> Self {
        Self::with_services(seed, Services::defaults())
    }

    /// Creates an idle core with explicit services (tests, alternative worlds).
    #[must_use]
    pub fn with_services(seed: u64, svc: Services) -> Self {
        let rng = RngStreams::new(seed);
        let probe = ProbeWorld::new(
            rng.stream("probe/init"),
            rng.stream("probe/gust"),
            rng.stream("probe/reseed"),
        );
        let mut world = World {
            observer: Observer::spawn(&*svc.geo),
            probe,
            input: crate::input::InputState::new(),
            counters: Counters::default(),
            snapshot: SnapshotBuffer::new(),
            replay: Box::new(NullReplaySink),
        };
        write_snapshot(&mut world, 0, CoreState::Idle.code());
        Self {
            tick: 0,
            state: CoreState::Idle,
            rng,
            world,
            schedule: default_schedule(),
            svc,
            profiler: None,
            profile_block: [0; profile::SIZE],
            terrain: None,
        }
    }

    fn set_state(&mut self, to: CoreState) {
        self.state = to;
        // The snapshot always says what state the core is in, even when no tick ran. It is not
        // offered to the replay sink: replays record ticks, and this snapshot repeats one.
        write_snapshot(&mut self.world, self.tick, to.code());
    }

    /// Loads a baked content pack (a `.cpack`). Only an idle core accepts one, and a pack that
    /// fails to load changes nothing. The pack's identity becomes part of the state hash.
    ///
    /// # Errors
    /// [`LoadContentError::NotIdle`] once the core has started, [`LoadContentError::Invalid`] if
    /// the bytes are not a valid pack.
    pub fn load_content(&mut self, bytes: &[u8]) -> Result<(), LoadContentError> {
        if self.state != CoreState::Idle {
            return Err(LoadContentError::NotIdle);
        }
        self.svc.registry = Registry::from_cpack(bytes).map_err(LoadContentError::Invalid)?;
        Ok(())
    }

    /// Replaces the replay sink, which receives the snapshot of every completed tick.
    pub fn set_replay_sink(&mut self, sink: Box<dyn ReplaySink>) {
        self.world.replay = sink;
    }

    /// `Idle -> Running`. Returns false (and changes nothing) from any other state.
    pub fn start(&mut self) -> bool {
        self.transition(CoreState::Idle, CoreState::Running)
    }

    /// `Running -> Paused`.
    pub fn pause(&mut self) -> bool {
        self.transition(CoreState::Running, CoreState::Paused)
    }

    /// `Paused -> Running`.
    pub fn resume(&mut self) -> bool {
        self.transition(CoreState::Paused, CoreState::Running)
    }

    /// `Running` or `Paused` to `Ended`.
    pub fn end(&mut self) -> bool {
        self.transition(CoreState::Running, CoreState::Ended)
            || self.transition(CoreState::Paused, CoreState::Ended)
    }

    fn transition(&mut self, from: CoreState, to: CoreState) -> bool {
        if self.state == from {
            self.set_state(to);
            true
        } else {
            false
        }
    }

    /// Runs up to `max_ticks` ticks if `Running`; returns how many ran.
    pub fn step(&mut self, max_ticks: u32) -> u32 {
        if self.state != CoreState::Running {
            return 0;
        }
        for _ in 0..max_ticks {
            if let Some(p) = self.profiler.as_mut() {
                p.begin_tick();
            }
            let mut ctx = TickCtx {
                world: &mut self.world,
                svc: &self.svc,
                dt: SIM_DT,
                tick: self.tick,
            };
            self.schedule.run_tick(&mut ctx, self.profiler.as_mut());
            self.tick += 1;
            if let Some(p) = self.profiler.as_mut() {
                p.end_tick();
            }
        }
        max_ticks
    }

    /// Queues encoded input frames (96-byte records) for the next tick. A trailing partial
    /// record is ignored. Returns how many frames were queued.
    pub fn push_inputs(&mut self, records: &[u8]) -> u32 {
        let mut queued = 0;
        for record in records.as_chunks::<FRAME_BYTES>().0 {
            if let Some(frame) = InputFrame::decode(record)
                && self.world.input.enqueue(frame)
            {
                queued += 1;
            }
        }
        queued
    }

    /// Selects the scripted pilot (see [`crate::layout::input_script`]). Returns false for an
    /// unknown kind, leaving the current script in place.
    pub fn set_input_script(&mut self, kind: u32) -> bool {
        match kind as usize {
            input_script::NONE => self.world.input.set_script(None),
            input_script::ERRATIC => self
                .world
                .input
                .set_script(Some(ErraticPilot::new(self.rng.stream("input/erratic")))),
            _ => return false,
        }
        true
    }

    /// Turns stage profiling on (with a clock) or off.
    pub fn set_profiler(&mut self, clock: Option<ClockFn>) {
        self.profiler = clock.map(Profiler::new);
    }

    /// True if stage profiling is on.
    #[must_use]
    pub fn profiling(&self) -> bool {
        self.profiler.is_some()
    }

    /// Ticks executed so far.
    #[must_use]
    pub fn tick(&self) -> u64 {
        self.tick
    }

    /// Current lifecycle state.
    #[must_use]
    pub fn core_state(&self) -> CoreState {
        self.state
    }

    /// The master seed.
    #[must_use]
    pub fn seed(&self) -> u64 {
        self.rng.master()
    }

    /// Read access to the probe world.
    #[must_use]
    pub fn probe(&self) -> &ProbeWorld {
        &self.world.probe
    }

    /// Read access to the observer.
    #[must_use]
    pub fn observer(&self) -> &Observer {
        &self.world.observer
    }

    /// Read access to the input state.
    #[must_use]
    pub fn input(&self) -> &crate::input::InputState {
        &self.world.input
    }

    /// Diagnostic counters of the non-input systems.
    #[must_use]
    pub fn counters(&self) -> Counters {
        self.world.counters
    }

    /// The services in force.
    #[must_use]
    pub fn services(&self) -> &Services {
        &self.svc
    }

    /// `(stage, system)` in execution order.
    #[must_use]
    pub fn schedule_description(&self) -> Vec<(crate::schedule::Stage, &'static str)> {
        self.schedule.describe()
    }

    /// The newest snapshot, [`crate::layout::snapshot::SIZE`] bytes. The host stamps its own
    /// publish time into its copy.
    #[must_use]
    pub fn snapshot_bytes(&self) -> &[u8] {
        self.world.snapshot.bytes()
    }

    /// The profile block, [`crate::layout::profile::SIZE`] bytes: counters always, timings only
    /// while profiling is on.
    pub fn profile_bytes(&mut self) -> &[u8] {
        let counters = counter_array(self.world.input.counters(), self.world.counters);
        if let Some(p) = &self.profiler {
            p.write_block(&mut self.profile_block, &counters);
        } else {
            self.profile_block.fill(0);
            write_counters(&mut self.profile_block, &counters);
        }
        &self.profile_block
    }

    /// Generates the terrain patch around BF `(centre_east_m, centre_north_m)` into an internal
    /// buffer and returns its size, or 0 if the arguments are unusable.
    pub fn fill_terrain(
        &mut self,
        centre_east_m: f64,
        centre_north_m: f64,
        spacing_m: f64,
    ) -> usize {
        let buf = self
            .terrain
            .get_or_insert_with(|| vec![0_u8; terrain::PATCH_BYTES].into_boxed_slice());
        terrain::fill_patch(
            &*self.svc.geo,
            buf,
            centre_east_m,
            centre_north_m,
            spacing_m,
        )
    }

    /// The most recently generated terrain patch (empty before the first fill).
    #[must_use]
    pub fn terrain_bytes(&self) -> &[u8] {
        self.terrain.as_deref().unwrap_or(&[])
    }

    /// Per-subsystem hashes and the combined hash.
    #[must_use]
    pub fn subsystem_hashes(&self) -> SubsystemHashes {
        let mut t = StateHasher::new();
        t.write_u64(self.tick);
        t.write_u64(u64::from(self.state as u8));
        t.write_u64(self.rng.master());
        let mut o = StateHasher::new();
        self.world.observer.hash_into(&mut o);
        let mut i = StateHasher::new();
        self.world.input.hash_into(&mut i);
        let mut c = StateHasher::new();
        hash_registry(&self.svc.registry, &mut c);
        SubsystemHashes::new(
            t.finish(),
            self.world.probe.hash_rng(),
            self.world.probe.hash_bodies(),
            o.finish(),
            i.finish(),
            c.finish(),
        )
    }

    /// The combined state hash (GDD 61.4.4 `state_hash`).
    #[must_use]
    pub fn state_hash(&self) -> u64 {
        self.subsystem_hashes().combined
    }
}

/// The registry's identity: entry count and the pack's SHA-256, which covers every byte of content.
fn hash_registry(reg: &Registry, h: &mut StateHasher) {
    h.write_u64(reg.len() as u64);
    h.write_bytes(reg.content_hash());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::{axis, button, snapshot as snap};

    fn running(seed: u64) -> SimCore {
        let mut c = SimCore::new(seed);
        assert!(c.start());
        c
    }

    fn scripted(seed: u64) -> SimCore {
        let mut c = SimCore::new(seed);
        assert!(c.set_input_script(input_script::ERRATIC as u32));
        assert!(c.start());
        c
    }

    fn frame_bytes(seq: u32, f: impl FnOnce(&mut InputFrame)) -> [u8; FRAME_BYTES] {
        let mut fr = InputFrame::neutral(seq);
        fr.axes[axis::THROTTLE] = 0.5;
        f(&mut fr);
        let mut b = [0_u8; FRAME_BYTES];
        fr.encode(&mut b);
        b
    }

    #[test]
    fn idle_core_does_not_step() {
        let mut c = SimCore::new(1);
        assert_eq!(c.step(10), 0);
        assert_eq!(c.tick(), 0);
        assert!(c.start());
        assert!(!c.start());
        assert_eq!(c.step(10), 10);
    }

    #[test]
    fn lifecycle_follows_the_state_machine() {
        let mut c = SimCore::new(1);
        assert_eq!(c.core_state(), CoreState::Idle);
        assert!(
            !c.pause() && !c.resume() && !c.end(),
            "nothing to do while idle"
        );
        assert!(c.start());
        assert!(c.pause());
        assert_eq!(c.step(5), 0, "paused cores do not step");
        assert!(!c.pause());
        assert!(c.resume());
        assert_eq!(c.step(5), 5);
        assert!(c.end());
        assert_eq!(c.core_state(), CoreState::Ended);
        assert_eq!(c.step(5), 0);
        assert!(!c.start() && !c.resume());
    }

    #[test]
    fn same_seed_same_hash_different_seed_different_hash() {
        let (mut a, mut b, mut c) = (running(5), running(5), running(6));
        a.step(2_000);
        b.step(2_000);
        c.step(2_000);
        assert_eq!(a.state_hash(), b.state_hash());
        assert_ne!(a.state_hash(), c.state_hash());
    }

    #[test]
    fn stepping_in_pieces_equals_stepping_at_once() {
        let (mut a, mut b) = (scripted(9), scripted(9));
        a.step(1_500);
        b.step(400);
        b.step(1);
        b.step(1_099);
        assert_eq!(a.tick(), b.tick());
        assert_eq!(a.subsystem_hashes(), b.subsystem_hashes());
        assert_eq!(a.snapshot_bytes(), b.snapshot_bytes());
    }

    #[test]
    fn hash_changes_every_tick() {
        let mut c = running(3);
        let mut last = c.state_hash();
        for _ in 0..50 {
            c.step(1);
            let h = c.state_hash();
            assert_ne!(h, last);
            last = h;
        }
    }

    #[test]
    fn state_stays_finite_and_quaternions_normalised() {
        let mut c = scripted(11);
        // 80,000 ticks: well past many reseed events, so respawned bodies are covered too.
        for _ in 0..80 {
            c.step(1_000);
            assert!(c.probe().is_finite());
            assert!(c.probe().max_quat_norm_error() < 1e-12);
            assert!(c.observer().pos_bf().is_finite());
            assert!((c.observer().attitude().norm_squared() - 1.0).abs() < 1e-12);
        }
    }

    #[test]
    fn bodies_stay_near_the_airspace() {
        let mut c = running(21);
        c.step(20_000);
        for i in 0..crate::probe::BODY_COUNT {
            let [x, y, z] = c.probe().pos_bf(i);
            assert!(
                x.abs() < 5.0e6 && y.abs() < 5.0e6,
                "body {i} left the area: {x} {y}"
            );
            assert!((-1.0e5..1.0e5).contains(&z), "body {i} altitude {z}");
        }
    }

    #[test]
    fn the_probe_world_is_untouched_since_v0_0a() {
        // These are the v0.0a golden values (tests/golden at tag v0.0a) for the columns that
        // belong to code v0.0b must not change: the tick hash, the probe RNG state and the
        // probe bodies. Only the combined hash may differ, because it now also covers the
        // observer and input.
        let expected: [(u64, &str, &str, &str); 4] = [
            (
                1,
                "31bd056f0e7d93cf",
                "433aab633ba8839c",
                "684582d5b16004d7",
            ),
            (
                10,
                "5d2e594364159ddc",
                "e7e55c3609204982",
                "caeede549d4708a6",
            ),
            (
                100,
                "53bc07060afee32e",
                "f80ab2364f97d779",
                "0031eade40a903b3",
            ),
            (
                1_000,
                "d3d7bcc9910dd0e7",
                "136366a631e14f37",
                "38e4d57909916103",
            ),
        ];
        let mut c = running(crate::DEFAULT_SEED);
        for (tick, tick_h, rng, bodies) in expected {
            c.step(u32::try_from(tick - c.tick()).unwrap());
            let h = c.subsystem_hashes();
            assert_eq!(format!("{:016x}", h.tick), tick_h, "tick hash @ {tick}");
            assert_eq!(format!("{:016x}", h.rng), rng, "rng hash @ {tick}");
            assert_eq!(format!("{:016x}", h.bodies), bodies, "bodies hash @ {tick}");
        }
    }

    #[test]
    fn host_input_steers_the_observer_and_a_stale_frame_does_not() {
        let mut c = running(1);
        let before = c.observer().attitude();
        let mut records = Vec::new();
        records.extend_from_slice(&frame_bytes(1, |f| f.axes[axis::PITCH] = 1.0));
        records.extend_from_slice(&frame_bytes(1, |f| f.axes[axis::PITCH] = -1.0)); // duplicate seq
        assert_eq!(
            c.push_inputs(&records),
            2,
            "both queued; the second is dropped as stale"
        );
        c.step(30);
        assert_ne!(c.observer().attitude(), before);
        assert!(
            c.observer().attitude().rotate(crate::math::DVec3::X).z > 0.2,
            "nose went up"
        );
        assert_eq!(c.input().counters().stale, 1);
        assert_eq!(c.input().counters().applied, 1);
        // A trailing partial record is ignored.
        assert_eq!(c.push_inputs(&records[..FRAME_BYTES + 5]), 1);
    }

    #[test]
    fn the_reset_button_works_end_to_end() {
        let mut c = scripted(4);
        c.step(600);
        let moved = c.observer().pos_bf();
        assert!((moved.y + 6_000.0).abs() > 100.0 || moved.x.abs() > 100.0);
        // The scripted pilot presses reset on tick 1799.
        c.step(1_200);
        assert_eq!(c.tick(), 1_800);
        let p = c.observer().pos_bf();
        // One tick after the reset the observer is back near its spawn point, 6 km south.
        assert!(p.x.abs() < 10.0 && (p.y + 6_000.0).abs() < 10.0, "{p:?}");
        let _ = button::RESET;
    }

    #[test]
    fn the_snapshot_follows_the_core() {
        let mut c = scripted(2);
        let word = |c: &SimCore, o: usize| {
            u32::from_le_bytes(c.snapshot_bytes()[o..o + 4].try_into().unwrap())
        };
        assert_eq!(word(&c, snap::H_CORE_STATE), 2, "started: running");
        c.step(7);
        let b = c.snapshot_bytes();
        assert_eq!(
            u64::from_le_bytes(b[snap::H_TICK..snap::H_TICK + 8].try_into().unwrap()),
            7
        );
        assert_eq!(
            word(&c, snap::H_LAST_INPUT_SEQ),
            7,
            "the script's frame number tracks the tick"
        );
        c.pause();
        assert_eq!(word(&c, snap::H_CORE_STATE), 3);
        c.resume();
        c.step(1);
        let p = c.observer().pos_bf();
        let b = c.snapshot_bytes();
        assert_eq!(
            f64::from_le_bytes(b[snap::O_POS_BF..snap::O_POS_BF + 8].try_into().unwrap()),
            p.x
        );
        assert_eq!(
            c.counters().snapshots,
            1 + 1 + 7 + 1 + 1 + 1,
            "initial, start, 7 ticks, pause, resume, 1 tick"
        );
    }

    /// A pack with one aircraft; sim-core cannot depend on the baker, so build the bytes here.
    fn pack(empty_kg: f32) -> Vec<u8> {
        use content::{AircraftDef, Category, Era};
        let def = AircraftDef {
            name_key: "ac.test.name".into(),
            category: Category::LightFighter,
            era: Era::G4,
            crew: 1,
            engine_count: 1,
            hardpoints: 2,
            sas_required: false,
            empty_kg,
            internal_fuel_kg: 1000.0,
            max_takeoff_kg: 6000.0,
            wing_area_m2: 20.0,
            span_m: 9.0,
            mac_m: 2.5,
            inertia_xx_kgm2: 10_000.0,
            inertia_yy_kgm2: 50_000.0,
            inertia_zz_kgm2: 55_000.0,
            inertia_xz_kgm2: 0.0,
            n_max: 9.0,
            n_min: -3.0,
            mmo: 2.0,
            vmo_ms: 350.0,
            service_ceiling_m: 15_000.0,
            thrust_mil_n: 50_000.0,
            thrust_ab_n: 80_000.0,
            cd0: 0.02,
            induced_k: 0.13,
            cl_alpha_per_rad: 3.5,
            cl_max: 1.5,
            static_margin_mac: 0.05,
        };
        let fake_hash = |body: &[u8]| {
            let mut h = StateHasher::new();
            h.write_bytes(body);
            let mut out = [0_u8; 32];
            for (i, chunk) in out.chunks_mut(8).enumerate() {
                chunk.copy_from_slice(&(h.finish() ^ i as u64).to_le_bytes());
            }
            out
        };
        content::cpack::write(&[("test:ac/one".into(), def)], fake_hash).unwrap()
    }

    #[test]
    fn content_loads_while_idle_and_joins_the_state_hash() {
        let mut c = SimCore::new(5);
        let before = c.subsystem_hashes();
        assert!(c.services().registry.is_empty());
        c.load_content(&pack(3000.0)).unwrap();
        assert_eq!(c.services().registry.len(), 1);
        let after = c.subsystem_hashes();
        assert_ne!(before.content, after.content);
        assert_ne!(before.combined, after.combined);
        // Content does not touch any other subsystem.
        assert_eq!(
            (
                before.tick,
                before.rng,
                before.bodies,
                before.observer,
                before.input
            ),
            (
                after.tick,
                after.rng,
                after.bodies,
                after.observer,
                after.input
            )
        );
        // Different content, different identity; the same content, the same.
        let mut d = SimCore::new(5);
        d.load_content(&pack(3001.0)).unwrap();
        assert_ne!(d.subsystem_hashes().content, after.content);
        let mut e = SimCore::new(5);
        e.load_content(&pack(3000.0)).unwrap();
        assert_eq!(e.subsystem_hashes(), after);
    }

    #[test]
    fn content_is_refused_once_started_and_when_invalid() {
        let mut c = SimCore::new(5);
        c.load_content(&pack(3000.0)).unwrap();
        let loaded = c.subsystem_hashes();

        let mut damaged = pack(3100.0);
        let last = damaged.len() - 1;
        damaged[last] ^= 1;
        assert!(matches!(
            c.load_content(&damaged),
            Err(LoadContentError::Invalid(CpackError::Checksum { .. }))
        ));
        assert!(matches!(
            c.load_content(&[1, 2, 3]),
            Err(LoadContentError::Invalid(CpackError::Truncated))
        ));
        assert_eq!(
            c.subsystem_hashes(),
            loaded,
            "a refused pack changes nothing"
        );

        c.start();
        assert_eq!(
            c.load_content(&pack(3100.0)),
            Err(LoadContentError::NotIdle)
        );
        assert_eq!(c.services().registry.len(), 1);
    }

    #[test]
    fn the_replay_sink_sees_each_tick_once_and_no_lifecycle_snapshots() {
        use std::sync::{Arc, Mutex};

        struct Log(Arc<Mutex<Vec<(u64, usize)>>>);
        impl ReplaySink for Log {
            fn record(&mut self, tick: u64, snapshot: &[u8]) {
                self.0.lock().unwrap().push((tick, snapshot.len()));
            }
        }

        let seen = Arc::new(Mutex::new(Vec::new()));
        let mut c = SimCore::new(3);
        c.set_replay_sink(Box::new(Log(Arc::clone(&seen))));
        c.start();
        c.step(3);
        c.pause();
        c.resume();
        c.step(2);
        let seen = seen.lock().unwrap();
        let size = crate::layout::snapshot::SIZE;
        assert_eq!(*seen, (1..=5).map(|t| (t, size)).collect::<Vec<_>>());
    }

    #[test]
    fn the_schedule_runs_in_stage_order() {
        let c = SimCore::new(1);
        let d = c.schedule_description();
        assert!(d.windows(2).all(|w| w[0].0 <= w[1].0));
        assert_eq!(d.first().map(|x| x.1), Some("ingest-input"));
        assert_eq!(d.last().map(|x| x.1), Some("publish-snapshot"));
    }

    #[test]
    fn the_profile_block_has_counters_without_a_profiler_and_timings_with_one() {
        use core::sync::atomic::{AtomicU64, Ordering};
        static T: AtomicU64 = AtomicU64::new(0);
        fn clock() -> f64 {
            // Advances 10 microseconds per read so every stage has a nonzero time.
            T.fetch_add(10, Ordering::SeqCst) as f64 / 1000.0
        }
        let mut c = scripted(8);
        c.step(5);
        let at = |b: &[u8], o: usize| f32::from_le_bytes(b[o..o + 4].try_into().unwrap());
        let block = c.profile_bytes().to_vec();
        assert_eq!(block.len(), profile::SIZE);
        assert_eq!(
            at(&block, profile::TICK_MS_LAST),
            0.0,
            "no timings without a profiler"
        );
        assert_eq!(
            u32::from_le_bytes(
                block[profile::COUNTERS..profile::COUNTERS + 4]
                    .try_into()
                    .unwrap()
            ),
            5,
            "input_applied counts the five scripted frames"
        );

        c.set_profiler(Some(clock));
        assert!(c.profiling());
        c.step(5);
        let block = c.profile_bytes().to_vec();
        assert_eq!(u64::from_le_bytes(block[0..8].try_into().unwrap()), 5);
        assert!(at(&block, profile::TICK_MS_LAST) > 0.0);
        for stage in [0, 4, 7, 10] {
            assert!(
                at(&block, profile::STAGE_MS_LAST + 4 * stage) > 0.0,
                "stage {stage} was timed"
            );
        }
        assert_eq!(
            at(&block, profile::STAGE_MS_LAST + 4 * 3),
            0.0,
            "the AI stage has no system yet"
        );
    }

    #[test]
    fn profiling_does_not_change_the_simulation() {
        fn clock() -> f64 {
            0.0
        }
        let (mut a, mut b) = (scripted(12), scripted(12));
        b.set_profiler(Some(clock));
        a.step(3_000);
        b.step(3_000);
        assert_eq!(a.subsystem_hashes(), b.subsystem_hashes());
    }

    #[test]
    fn terrain_fill_produces_a_patch() {
        let mut c = SimCore::new(1);
        assert!(matches!(c.terrain_bytes(), []));
        assert_eq!(c.fill_terrain(0.0, 0.0, 250.0), terrain::PATCH_BYTES);
        assert_eq!(c.terrain_bytes().len(), terrain::PATCH_BYTES);
        assert_eq!(c.fill_terrain(0.0, 0.0, -1.0), 0);
    }

    #[test]
    fn unknown_input_script_is_rejected() {
        let mut c = SimCore::new(1);
        assert!(!c.set_input_script(99));
        assert!(c.set_input_script(input_script::NONE as u32));
    }
}
