//! The snapshot v5 combat block (ADR-48, layout `snapshot::C_*`, `BD_*`, `MS_*`, `TR_*`, `XP_*`,
//! `EV_*`), written at publication from the combat state without allocating. Nothing in it is
//! hashed; the latched flags come from the stamps of when things happened.

use flight::aircraft::AircraftModel;
use weapons::missile::MissilePhase;

use super::models::CombatModels;
use super::{Combat, HIT_LATCH_S, KILL_LATCH_S, PLAYER_HIT_LATCH_S, TRACER_CAPACITY, ticks};
use crate::layout::{combat_flag as f, snapshot as s};
use crate::math::DVec3;
use crate::services::Services;
use crate::time::SIM_DT;

fn put_u16(b: &mut [u8], at: usize, v: u16) {
    b[at..at + 2].copy_from_slice(&v.to_le_bytes());
}

fn put_u32(b: &mut [u8], at: usize, v: u32) {
    b[at..at + 4].copy_from_slice(&v.to_le_bytes());
}

fn put_f32(b: &mut [u8], at: usize, v: f64) {
    put_u32(b, at, (v as f32).to_bits());
}

fn put_f64x3(b: &mut [u8], at: usize, v: DVec3) {
    for (k, c) in [v.x, v.y, v.z].into_iter().enumerate() {
        b[at + 8 * k..at + 8 * k + 8].copy_from_slice(&c.to_bits().to_le_bytes());
    }
}

fn put_f32x3(b: &mut [u8], at: usize, v: DVec3) {
    for (k, c) in [v.x, v.y, v.z].into_iter().enumerate() {
        put_f32(b, at + 4 * k, c);
    }
}

/// Zeroes the combat block (no combat).
pub fn clear_block(b: &mut [u8]) {
    b[s::COMBAT..s::SIZE].fill(0);
}

/// The combat state as the snapshot shows it after a tick.
#[derive(Clone, Copy, Debug)]
pub struct CombatView<'a> {
    /// The state.
    pub combat: &'a Combat,
    /// The tick that just ran (`ticks_done − 1`; the stamps' clock).
    pub tick: u64,
    /// The player's and the bandit's hit points when new.
    pub hp_max: f64,
    /// The bandit's reference point, BF.
    pub bandit_ref_bf: DVec3,
    /// Its true airspeed, m/s.
    pub bandit_tas_mps: f64,
}

impl<'a> CombatView<'a> {
    /// The view after `ticks_done` ticks.
    #[must_use]
    pub fn of(
        combat: &'a Combat,
        models: &CombatModels,
        model: &AircraftModel,
        svc: &Services,
        ticks_done: u64,
    ) -> Self {
        let b = &combat.bandit.slot.state.body;
        let bandit_ref_bf = model.ref_point_bf(&combat.bandit.slot.state);
        let t = crate::time::sim_time(ticks_done);
        Self {
            combat,
            tick: ticks_done.saturating_sub(1),
            hp_max: models.hp,
            bandit_ref_bf,
            bandit_tas_mps: (b.vel_bf - svc.weather.wind_bf(bandit_ref_bf, t)).length(),
        }
    }

    fn latched(&self, at: Option<u64>, hold_s: f64) -> bool {
        at.is_some_and(|t| self.tick.saturating_sub(t) < u64::from(ticks(hold_s)))
    }

    /// The `combat_flag` bits.
    #[must_use]
    pub fn flags(&self) -> u16 {
        let c = self.combat;
        let mut bits = f::ACTIVE;
        for (on, bit) in [
            (c.player.armed, f::ARMED),
            (c.player.gun.firing, f::GUN_FIRING),
            (c.bandit.gun.firing, f::BANDIT_GUN_FIRING),
            (c.cues.target, f::TARGET),
            (c.cues.pipper_bf.is_some(), f::PIPPER),
            (c.cues.shoot_gun, f::SHOOT_GUN),
            (c.cues.in_range, f::IN_RANGE),
            (c.cues.shoot_missile, f::SHOOT_MISSILE),
            (self.latched(c.stamps.hit_tick, HIT_LATCH_S), f::HIT_RECENT),
            (
                self.latched(c.stamps.kill_tick, KILL_LATCH_S),
                f::KILL_RECENT,
            ),
            (
                self.latched(c.stamps.player_hit_tick, PLAYER_HIT_LATCH_S),
                f::PLAYER_HIT,
            ),
        ] {
            if on {
                bits |= bit;
            }
        }
        bits as u16
    }

    /// Writes the block into a snapshot.
    pub fn write(&self, b: &mut [u8]) {
        clear_block(b);
        let c = self.combat;
        put_u16(b, s::C_FLAGS, self.flags());
        b[s::C_SEEKER] = c.player.seeker.mode.code();
        b[s::C_INHIBIT] = c.stamps.inhibit.map_or(0, |(_, code)| code);
        put_u16(b, s::C_ROUNDS, c.player.gun.rounds);
        b[s::C_MISSILES] = c.player.missiles_left;
        b[s::C_BANDITS] = u8::from(c.bandit.present());
        put_u32(b, s::C_EVENT_SEQ, c.events.seq);
        put_f64x3(b, s::C_PIPPER_BF, c.cues.pipper_bf.unwrap_or(DVec3::ZERO));
        put_f32(b, s::C_SIGNAL, c.player.signal);
        if c.cues.target {
            put_f32(b, s::C_TGT_RANGE, c.cues.range_m);
            put_f32(b, s::C_TGT_CLOSURE, c.cues.closure_mps);
            put_f32(b, s::C_TGT_ASPECT, c.cues.aspect_rad);
        }
        put_f32(
            b,
            s::C_HEALTH,
            (c.player.health_hp / self.hp_max).clamp(0.0, 1.0),
        );
        let age = c
            .stamps
            .inhibit
            .map_or(0.0, |(t, _)| self.tick.saturating_sub(t) as f64 * SIM_DT);
        put_f32(b, s::C_INHIBIT_AGE, age);
        let st = &c.stats;
        for (k, v) in [
            st.rounds_fired,
            st.round_hits,
            st.missiles_fired,
            st.missile_hits,
            st.kills,
            st.gun_kills,
            st.missile_kills,
            st.deaths,
            st.hits_taken,
            st.bandit_rounds,
            st.explosions,
            st.bandit_spawns,
        ]
        .into_iter()
        .enumerate()
        {
            put_u32(b, s::C_ROUNDS_FIRED + 4 * k, v);
        }
        self.write_bandit(b);
        self.write_flying(b);
    }

    fn write_bandit(&self, b: &mut [u8]) {
        let c = self.combat;
        let bd = &c.bandit;
        let st = &bd.slot.state;
        put_f64x3(b, s::BD_POS_BF, self.bandit_ref_bf);
        let q = st.body.att;
        for (k, v) in [q.w, q.x, q.y, q.z].into_iter().enumerate() {
            put_f32(b, s::BD_QUAT + 4 * k, v);
        }
        put_f32x3(b, s::BD_VEL_BF, st.body.vel_bf);
        let sf = &st.surfaces;
        put_f32x3(
            b,
            s::BD_SURFACES,
            DVec3::new(sf.elevator_rad, sf.aileron_rad, sf.rudder_rad),
        );
        put_f32(b, s::BD_TEF, sf.tef_rad);
        put_f32(b, s::BD_AIRBRAKE, sf.airbrake);
        put_f32(b, s::BD_GEAR_POS, sf.gear);
        let engine = &st.engines[0];
        put_f32(b, s::BD_AB_LEVEL, engine.ab_level);
        put_f32(
            b,
            s::BD_HEALTH,
            (bd.health_hp / self.hp_max).clamp(0.0, 1.0),
        );
        put_f32(b, s::BD_TAS, self.bandit_tas_mps);
        b[s::BD_LIFE] = bd.life.code();
        b[s::BD_AI] = bd.brain.state.code();
        b[s::BD_SKILL] = c.rank;
        let defending = matches!(
            bd.brain.state,
            ai_pilot::BanditState::DefendGun | ai_pilot::BanditState::DefendMissile
        );
        let mut flags = 0;
        for (on, bit) in [
            (bd.gun.firing, crate::layout::bandit_flag::GUN_FIRING),
            (engine.ab_lit, crate::layout::bandit_flag::AB_LIT),
            (c.perception.sees, crate::layout::bandit_flag::SEES_PLAYER),
            (defending, crate::layout::bandit_flag::DEFENDING),
        ] {
            if on {
                flags |= bit;
            }
        }
        b[s::BD_FLAGS] = flags as u8;
        put_u16(b, s::BD_SPAWN_SEQ, bd.spawn_seq);
    }

    fn write_flying(&self, b: &mut [u8]) {
        let c = self.combat;
        for (i, m) in c.missiles.iter().enumerate() {
            if m.m.phase == MissilePhase::Free {
                continue;
            }
            let at = s::MISSILES + i * s::MISSILE_STRIDE;
            put_f64x3(b, at + s::MS_POS_BF, m.m.pos);
            put_f32x3(b, at + s::MS_VEL_BF, m.m.vel);
            b[at + s::MS_STATE] = m.m.phase.code();
            b[at + s::MS_OWNER] = m.owner;
        }
        let mut n = 0;
        for (i, r) in c.rounds.slots.iter().enumerate() {
            if n == TRACER_CAPACITY {
                break;
            }
            if !(r.live() && r.tracer) {
                continue;
            }
            let at = s::TR_FIRST + n * s::TRACER_STRIDE;
            put_f64x3(b, at + s::TR_POS_BF, r.round.pos);
            put_f32x3(b, at + s::TR_VEL_BF, r.round.vel);
            b[at + s::TR_OWNER] = r.owner;
            b[at + s::TR_ID] = i as u8;
            n += 1;
        }
        put_u32(b, s::TR_COUNT, n as u32);
        for (i, e) in c.explosions.iter().enumerate() {
            if e.kind == 0 {
                continue;
            }
            let at = s::EXPLOSIONS + i * s::EXPLOSION_STRIDE;
            put_f64x3(b, at + s::XP_POS_BF, e.pos_bf);
            put_f32(b, at + s::XP_AGE, e.age_s);
            b[at + s::XP_KIND] = e.kind;
        }
        for (i, e) in c.events.entries.iter().enumerate() {
            let at = s::EVENTS + i * s::EVENT_STRIDE;
            put_u32(b, at + s::EV_SEQ, e.seq);
            put_u32(b, at + s::EV_TICK, e.tick);
            b[at + s::EV_KIND] = e.kind;
            b[at + s::EV_ACTOR] = e.actor;
            b[at + s::EV_TARGET] = e.target;
            b[at + s::EV_ARG] = e.arg;
            put_u32(b, at + s::EV_VALUE, e.value.to_bits());
        }
    }
}
