//! The combat systems of the tick (ADR-48, plan 7.1). Each returns at once without combat: no
//! state changes and no random draws, so a free-flight session runs exactly as before.
//!
//! ```text
//! Ai         CombatAi        where both aircraft start; activation; perception; the brain decides
//!                            (5 Hz) and flies (every tick): one input frame into the bandit's input
//! Dynamics   BanditDynamics  pilot::fly for the bandit's slot (after the ownship and the probe)
//! Weapons    CombatWeapons   rounds and missiles move; the player's gun, seeker, launches and cues;
//!                            the bandit's gun
//! Collision  CombatCollision rounds, fuzes, damage, kills and deaths; the aircraft colliding; the
//!                            bandit meeting the ground (after the player's terrain contact)
//! Systems    CombatLifecycle the wreck's fall, the respawn, bursts and explosions ageing
//! ```

use ai_pilot::Airframe;

use super::hits::collision_tick;
use super::player::{Kin, TargetSource, weapons_tick};
use super::rounds::{fire, rounds_this_tick};
use super::{
    ACTIVATE_AIRBORNE_S, BANDIT_RESPAWN_S, BanditLife, DECISION_EVERY_TICKS, DECISION_PHASE,
    EXPLOSION_LIFE_S, Explosion, PrevPositions, WRECK_MAX_S, missiles, perception, ticks,
};
use crate::layout::combat_actor;
use crate::ownship::Ownship;
use crate::pilot;
use crate::schedule::{Stage, System, TickCtx};
use crate::time::sim_time;

/// Stage 4 (Ai): the bandit's brain.
pub(crate) struct CombatAi;

impl System for CombatAi {
    fn name(&self) -> &'static str {
        "combat-ai"
    }
    fn stage(&self) -> Stage {
        Stage::Ai
    }
    fn run(&mut self, ctx: &mut TickCtx<'_>) {
        let w = &mut *ctx.world;
        let svc = ctx.svc;
        let Some(c) = w.combat.as_deref_mut() else {
            return;
        };
        let (Some(models), Some(model), Some(fcs_model), Ownship::Aircraft(player)) = (
            svc.combat.as_ref(),
            svc.aircraft.as_ref(),
            svc.fcs.as_ref(),
            &w.ownship,
        ) else {
            return;
        };
        let tick = ctx.tick;
        let t = sim_time(tick);
        let player_alive = !(player.crashed || player.state.numerical_fault);
        c.prev = PrevPositions {
            player_ref_bf: model.ref_point_bf(&player.state),
            player_spawn_seq: player.spawn_seq,
            bandit_ref_bf: model.ref_point_bf(&c.bandit.slot.state),
            bandit_spawn_seq: c.bandit.spawn_seq,
        };
        if player_alive && !player.state.ground.wow {
            c.airborne_ticks = c.airborne_ticks.saturating_add(1);
        }
        if c.airborne_ticks >= ticks(ACTIVATE_AIRBORNE_S) {
            c.activated = true;
        }
        if !c.bandit.present() || c.bandit.slot.crashed {
            return;
        }
        let pk = player_alive.then(|| Kin::of(model, player));
        let bk = Kin::of(model, &c.bandit.slot);
        perception::update(c, models, svc, pk.as_ref(), &bk, tick);
        let env = svc.flight_env();
        let tel = model.telemetry_light(&c.bandit.slot.state, &env, t);
        let p = perception::percept(
            c,
            models,
            model,
            svc,
            &bk,
            (tel.cas_mps, tel.air.tas_mps, tel.agl_m),
            pk.as_ref(),
            tick,
        );
        if tick % DECISION_EVERY_TICKS == DECISION_PHASE {
            c.bandit.brain.decide(&p, &c.skill);
        }
        let b = &mut c.bandit;
        let af = Airframe {
            model,
            fcs: fcs_model,
            ac: &b.slot.state,
            fs: &b.slot.fcs,
            env: &env,
            t_s: t,
        };
        let out = b.brain.act(&p, &c.skill, &af);
        b.fire_gun = out.fire_gun && b.life == BanditLife::Alive;
        b.apply(&out, tick);
    }
}

/// Stage 5 (Dynamics): the bandit flies, after the ownship and the probe.
pub(crate) struct BanditDynamics;

impl System for BanditDynamics {
    fn name(&self) -> &'static str {
        "bandit-dynamics"
    }
    fn stage(&self) -> Stage {
        Stage::Dynamics
    }
    fn run(&mut self, ctx: &mut TickCtx<'_>) {
        let svc = ctx.svc;
        let Some(c) = ctx.world.combat.as_deref_mut() else {
            return;
        };
        let (Some(model), Some(fcs_model)) = (svc.aircraft.as_ref(), svc.fcs.as_ref()) else {
            return;
        };
        let b = &mut c.bandit;
        b.slot.events = flight::gear::GroundEvents::NONE;
        if !b.present() || b.slot.crashed || b.slot.state.numerical_fault {
            return;
        }
        let t = sim_time(ctx.tick);
        pilot::fly(
            &mut b.slot,
            model,
            fcs_model,
            svc,
            &b.input,
            &mut b.counters,
            t,
        );
    }
}

/// Stage 6 (Weapons): rounds and missiles move, then the guns fire and the player's weapons work.
pub(crate) struct CombatWeapons;

impl System for CombatWeapons {
    fn name(&self) -> &'static str {
        "combat-weapons"
    }
    fn stage(&self) -> Stage {
        Stage::Weapons
    }
    fn run(&mut self, ctx: &mut TickCtx<'_>) {
        let w = &mut *ctx.world;
        let svc = ctx.svc;
        let Some(c) = w.combat.as_deref_mut() else {
            return;
        };
        let (Some(models), Some(model), Ownship::Aircraft(player)) =
            (svc.combat.as_ref(), svc.aircraft.as_ref(), &w.ownship)
        else {
            return;
        };
        let tick = ctx.tick;
        let dt = ctx.dt;
        let t = sim_time(tick);
        let env = svc.flight_env();
        c.rounds
            .advance(&models.gun, &env, model.atmosphere(), t, dt);
        let alive = c.bandit.life == BanditLife::Alive;
        let target = alive.then(|| TargetSource::of(model, svc, &c.bandit.slot, t + dt));
        missiles::advance(c, models, model, svc, target.as_ref(), tick);
        weapons_tick(
            c,
            models,
            model,
            svc,
            player,
            &w.input,
            target.as_ref(),
            tick,
        );
        // The bandit's gun.
        let trigger = alive && c.activated && c.skill.fires_gun && c.bandit.fire_gun;
        let n = rounds_this_tick(&mut c.bandit.gun, trigger, models.gun.rate_rps, dt);
        if n > 0 {
            let shooter = Kin::of(model, &c.bandit.slot).shooter();
            let added = fire(
                &mut c.rounds,
                &models.gun,
                &models.mount,
                &shooter,
                &mut c.bandit.gun,
                n,
                combat_actor::BANDIT as u8,
                &mut c.rng.bandit_gun,
            );
            c.stats.bandit_rounds = c.stats.bandit_rounds.wrapping_add(added);
        }
    }
}

/// Stage 8 (Collision): hits, fuzes, collisions and the bandit's ground contact, after the
/// player's terrain contact.
pub(crate) struct CombatCollision;

impl System for CombatCollision {
    fn name(&self) -> &'static str {
        "combat-collision"
    }
    fn stage(&self) -> Stage {
        Stage::Collision
    }
    fn run(&mut self, ctx: &mut TickCtx<'_>) {
        let w = &mut *ctx.world;
        let svc = ctx.svc;
        let Some(c) = w.combat.as_deref_mut() else {
            return;
        };
        let (Some(models), Some(model)) = (svc.combat.as_ref(), svc.aircraft.as_ref()) else {
            return;
        };
        let player = match &mut w.ownship {
            Ownship::Aircraft(s) => Some(&mut **s),
            Ownship::Spectator(_) => None,
        };
        collision_tick(c, models, model, svc, player, ctx.tick);
    }
}

/// Stage 9 (Systems): the bandit's life cycle, and bursts and explosions ageing.
pub(crate) struct CombatLifecycle;

impl System for CombatLifecycle {
    fn name(&self) -> &'static str {
        "combat-lifecycle"
    }
    fn stage(&self) -> Stage {
        Stage::Systems
    }
    fn run(&mut self, ctx: &mut TickCtx<'_>) {
        let svc = ctx.svc;
        let Some(c) = ctx.world.combat.as_deref_mut() else {
            return;
        };
        let (Some(models), Some(model)) = (svc.combat.as_ref(), svc.aircraft.as_ref()) else {
            return;
        };
        let b = &mut c.bandit;
        match b.life {
            BanditLife::Alive => {}
            BanditLife::Wreck => {
                b.life_ticks = b.life_ticks.saturating_add(1);
                if b.life_ticks >= ticks(WRECK_MAX_S) {
                    b.life = BanditLife::Absent;
                    b.life_ticks = 0;
                    b.slot.crashed = true;
                }
            }
            BanditLife::Absent => {
                b.life_ticks = b.life_ticks.saturating_add(1);
                if b.life_ticks >= ticks(BANDIT_RESPAWN_S) {
                    c.spawn_bandit(None, models, model, svc, ctx.tick);
                }
            }
        }
        missiles::age_bursts(c);
        for e in &mut c.explosions {
            if e.kind != 0 {
                e.age_s += ctx.dt;
                if e.age_s >= EXPLOSION_LIFE_S {
                    *e = Explosion::default();
                }
            }
        }
    }
}
