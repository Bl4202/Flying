//! The bandit (ADR-47, ADR-48): a second aircraft of the player's type, in its own slot, flown by
//! `pilot::fly` from its own input state. Its brain writes one input frame a tick, exactly as a
//! host would: the sticks, the throttle, the airbrake, the assist request (Manual once it is a
//! wreck) and the spawn acknowledgement.
//!
//! Its life: ALIVE until destroyed; then a WRECK (falling under Manual, idle) until it meets the
//! ground or for at most 30 s; then ABSENT; a new bandit 20 s later.

use ai_pilot::{BanditBrain, BrainKind, BrainOut};
use flight::aircraft::AircraftModel;
use flight::body::RigidState;
use flight::engine::EngineCommand;

use super::AirStart;
use super::models::CombatModels;
use super::player::GunState;
use crate::input::{InputFrame, InputState};
use crate::layout::{assist_level, axis, bandit_life, button};
use crate::math::{DQuat, DVec3};
use crate::ownship::AircraftSlot;
use crate::services::Services;
use crate::session::{Assist, GEffects, Scenario, SessionConfig, Spawn};
use crate::world::Counters;

/// Where the bandit is in its life.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum BanditLife {
    /// There is none (between a wreck and the next spawn).
    #[default]
    Absent = bandit_life::ABSENT as u8,
    /// Flying and fighting.
    Alive = bandit_life::ALIVE as u8,
    /// Destroyed, falling.
    Wreck = bandit_life::WRECK as u8,
}

impl BanditLife {
    /// The snapshot's code.
    #[must_use]
    pub const fn code(self) -> u8 {
        self as u8
    }
}

/// The bandit.
#[derive(Clone, Debug)]
pub struct Bandit {
    /// Its aircraft.
    pub slot: AircraftSlot,
    /// Its controls, fed by the brain's frames.
    pub input: InputState,
    /// Its brain.
    pub brain: BanditBrain,
    /// Its life.
    pub life: BanditLife,
    /// Ticks in the current life (a wreck's fall, the absence before a respawn).
    pub life_ticks: u32,
    /// Hit points left.
    pub health_hp: f64,
    /// Bandits spawned, wrapping.
    pub spawn_seq: u16,
    /// Its gun.
    pub gun: GunState,
    /// The brain holds the trigger this tick.
    pub fire_gun: bool,
    /// The tick the player last damaged it.
    pub damaged_by_player_tick: Option<u64>,
    /// Its own diagnostic counters (so the player's profile counters stay the player's).
    pub counters: Counters,
    /// The sequence number of its last frame.
    pub frame_seq: u32,
}

impl Bandit {
    /// Not yet spawned: absent, parked at the origin.
    #[must_use]
    pub fn new(model: &AircraftModel, session: &SessionConfig, models: &CombatModels) -> Self {
        let parked = RigidState {
            pos_bf: DVec3::ZERO,
            vel_bf: DVec3::ZERO,
            att: DQuat::IDENTITY,
            omega_body: DVec3::ZERO,
        };
        Self {
            slot: AircraftSlot::new(
                model.state(parked, 0.0, EngineCommand::IDLE),
                bandit_session(session),
            ),
            input: InputState::new(),
            brain: BanditBrain::new(BrainKind::Normal, 1.0),
            life: BanditLife::Absent,
            life_ticks: 0,
            health_hp: models.hp,
            spawn_seq: 0,
            gun: GunState::full(models.gun_rounds),
            fire_gun: false,
            damaged_by_player_tick: None,
            counters: Counters::default(),
            frame_seq: 0,
        }
    }

    /// A new bandit at `s`, trimmed level, under a fresh brain of `kind` (clockwise orbit).
    pub(crate) fn spawn(
        &mut self,
        s: &AirStart,
        kind: BrainKind,
        model: &AircraftModel,
        svc: &Services,
        models: &CombatModels,
    ) {
        if !crate::spawn::spawn_air_at(
            model,
            svc,
            s.east_m,
            s.north_m,
            s.alt_msl_m,
            s.heading_rad,
            s.tas_mps,
            &mut self.slot,
        ) {
            self.counters.trim_failures = self.counters.trim_failures.wrapping_add(1);
        }
        self.counters.respawns = self.counters.respawns.wrapping_add(1);
        // The control system starts at the session's Standard, whatever the last bandit ended in.
        self.slot.fcs = fcs::FcsState::new(Assist::Standard.to_fcs(), false);
        let sf = &self.slot.state.surfaces;
        self.slot.fcs.last_cmd = [sf.elevator_rad, sf.aileron_rad, sf.rudder_rad];
        self.brain = BanditBrain::new(kind, 1.0);
        self.life = BanditLife::Alive;
        self.life_ticks = 0;
        self.spawn_seq = self.spawn_seq.wrapping_add(1);
        self.fire_gun = false;
        self.damaged_by_player_tick = None;
        self.health_hp = models.hp;
        self.gun = GunState::full(models.gun_rounds);
    }

    /// Its frame for this tick from the brain's output, applied through its input state.
    pub(crate) fn apply(&mut self, out: &BrainOut, tick: u64) {
        self.frame_seq = self.frame_seq.wrapping_add(1).max(1);
        let mut f = InputFrame::neutral(self.frame_seq);
        let c = &out.controls;
        f.axes[axis::PITCH] = c.pitch as f32;
        f.axes[axis::ROLL] = c.roll as f32;
        f.axes[axis::YAW] = c.yaw as f32;
        f.axes[axis::THROTTLE] = c.throttle as f32;
        if c.airbrake > 0.5 {
            f.buttons |= 1 << button::AIRBRAKE;
        }
        f.assist_req = if out.manual {
            assist_level::MANUAL as u8
        } else {
            0
        };
        f.spawn_ack = self.slot.spawn_seq;
        self.input.enqueue(f);
        self.input.ingest(tick);
    }

    /// It exists (alive or a wreck).
    #[must_use]
    pub fn present(&self) -> bool {
        self.life != BanditLife::Absent
    }
}

/// The bandit's session: an air start under Standard assist with the g-effects off, in the
/// player's arena, no combat of its own.
#[must_use]
pub fn bandit_session(player: &SessionConfig) -> SessionConfig {
    SessionConfig {
        spawn: Spawn::Air,
        assist: Assist::Standard,
        g_effects: GEffects::Off,
        arena: player.arena,
        scenario: Scenario::Free,
        skill: 0,
    }
}
