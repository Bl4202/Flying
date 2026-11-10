//! Hits, damage, kills and deaths (ADR-46, ADR-48, GDD 18, 65.2).
//!
//! - **Rounds**: each round's path this tick, taken into the target's body axes relative to its
//!   reference point's path, against its capsules (swept: nothing tunnels). A hit delivers
//!   `½ m |v_round − v_target|²` and removes the round.
//! - **Missiles**: an armed fuze at its first closest approach within the trigger radius; the
//!   warhead delivers `damage_j (1 − d / R)²` inside the lethal radius.
//! - **Collision**: the two aircraft's capsules overlapping destroys both; nobody is credited.
//! - **The ground**: a living bandit meeting it is a `BANDIT_CRASH`, and the player's kill if the
//!   player damaged it within the last 30 s; a wreck meeting it is gone.
//!
//! The player shot down (or collided) is frozen and respawns like a crash (`SHOT_DOWN`,
//! `COLLISION`).

use flight::aircraft::AircraftModel;
use flight::gear::GroundEvents;
use weapons::capsule::{capsules_distance, relative_segment, segment_hits_capsules};
use weapons::fuze::{damage_energy, fuze_armed, fuze_check};

use super::models::CombatModels;
use super::player::Kin;
use super::rounds::RoundSlot;
use super::{BanditLife, Combat, EXPLOSION_CAPACITY, Explosion, KILL_CREDIT_WINDOW_S};
use crate::layout::{combat_actor, combat_event, crash_cause, explosion_kind, kill_weapon};
use crate::math::DVec3;
use crate::ownship::{AircraftSlot, RESPAWN_TICKS};
use crate::services::Services;

/// The energy of a round's hit, J: `½ m |v_round − v_target|²`.
#[must_use]
pub fn round_energy_j(mass_kg: f64, v_round: DVec3, v_target: DVec3) -> f64 {
    0.5 * mass_kg * (v_round - v_target).length_squared()
}

/// Shows an explosion, over the oldest if all are showing.
pub(crate) fn explode(c: &mut Combat, pos_bf: DVec3, kind: usize) {
    let at = c
        .explosions
        .iter()
        .position(|e| e.kind == 0)
        .unwrap_or_else(|| {
            let mut oldest = 0;
            for (i, e) in c.explosions.iter().enumerate().take(EXPLOSION_CAPACITY) {
                if e.age_s > c.explosions[oldest].age_s {
                    oldest = i;
                }
            }
            oldest
        });
    c.explosions[at] = Explosion {
        pos_bf,
        age_s: 0.0,
        kind: kind as u8,
    };
    c.stats.explosions = c.stats.explosions.wrapping_add(1);
}

/// The bandit takes `joules` from the player's `weapon` (`kill_weapon`) at `range_m`; a kill if it
/// was alive and has nothing left.
#[allow(clippy::too_many_arguments)] // ALGO: who, what, where and when
pub(crate) fn damage_bandit(
    c: &mut Combat,
    models: &CombatModels,
    joules: f64,
    weapon: usize,
    range_m: f64,
    pos_bf: DVec3,
    tick: u64,
) {
    if c.bandit.life != BanditLife::Alive {
        return;
    }
    c.bandit.health_hp -= joules / models.joules_per_hp;
    c.bandit.damaged_by_player_tick = Some(tick);
    if c.bandit.health_hp <= 0.0 {
        c.bandit.health_hp = 0.0;
        kill_bandit(c, weapon, range_m, pos_bf, tick);
    }
}

/// The bandit is destroyed in the air and credited to the player's `weapon`.
fn kill_bandit(c: &mut Combat, weapon: usize, range_m: f64, pos_bf: DVec3, tick: u64) {
    wreck_bandit(c, pos_bf, tick);
    credit(c, weapon, range_m, tick);
}

/// The bandit becomes a wreck.
pub(crate) fn wreck_bandit(c: &mut Combat, pos_bf: DVec3, tick: u64) {
    c.bandit.life = BanditLife::Wreck;
    c.bandit.life_ticks = 0;
    c.bandit.fire_gun = false;
    c.bandit.brain.set_wreck(crate::time::sim_time(tick + 1));
    explode(c, pos_bf, explosion_kind::AIRCRAFT);
}

/// A KILL for the player.
fn credit(c: &mut Combat, weapon: usize, range_m: f64, tick: u64) {
    let s = &mut c.stats;
    s.kills = s.kills.wrapping_add(1);
    match weapon {
        kill_weapon::GUN => s.gun_kills = s.gun_kills.wrapping_add(1),
        kill_weapon::MISSILE => s.missile_kills = s.missile_kills.wrapping_add(1),
        _ => {}
    }
    c.stamps.kill_tick = Some(tick);
    c.events.push(
        combat_event::KILL as u8,
        combat_actor::PLAYER as u8,
        combat_actor::BANDIT as u8,
        weapon as u8,
        range_m,
        tick,
    );
}

/// The player takes `joules` from the bandit's gun; shot down if nothing is left.
pub(crate) fn damage_player(
    c: &mut Combat,
    models: &CombatModels,
    slot: &mut AircraftSlot,
    joules: f64,
    pos_bf: DVec3,
    tick: u64,
) {
    if slot.crashed {
        return;
    }
    c.player.health_hp -= joules / models.joules_per_hp;
    c.stats.hits_taken = c.stats.hits_taken.wrapping_add(1);
    c.stamps.player_hit_tick = Some(tick);
    if c.player.health_hp <= 0.0 {
        c.player.health_hp = 0.0;
        player_destroyed(
            c,
            slot,
            crash_cause::SHOT_DOWN,
            combat_actor::BANDIT,
            pos_bf,
            tick,
        );
    }
}

/// The player's aircraft is destroyed by combat: frozen, and it respawns like a crash.
fn player_destroyed(
    c: &mut Combat,
    slot: &mut AircraftSlot,
    cause: usize,
    by: usize,
    pos_bf: DVec3,
    tick: u64,
) {
    slot.crashed = true;
    slot.crash_cause = cause as u8;
    slot.respawn_ticks = RESPAWN_TICKS;
    c.stats.deaths = c.stats.deaths.wrapping_add(1);
    c.events.push(
        combat_event::DEATH as u8,
        by as u8,
        combat_actor::PLAYER as u8,
        cause as u8,
        0.0,
        tick,
    );
    explode(c, pos_bf, explosion_kind::AIRCRAFT);
}

/// The collision stage's combat work. `player` is the player's slot (with its motion now) while
/// it is alive.
pub(crate) fn collision_tick(
    c: &mut Combat,
    models: &CombatModels,
    model: &AircraftModel,
    svc: &Services,
    player: Option<&mut AircraftSlot>,
    tick: u64,
) {
    let mut player = player.filter(|s| !(s.crashed || s.state.numerical_fault));
    let pk = player.as_deref().map(|s| Kin::of(model, s));
    let bandit_alive = c.bandit.life == BanditLife::Alive;
    let bk = bandit_alive.then(|| Kin::of(model, &c.bandit.slot));
    // A spawn is a jump, not a path: its tick tests from where it is now.
    let p_prev = match (&player, pk) {
        (Some(s), Some(_)) if s.spawn_seq == c.prev.player_spawn_seq => c.prev.player_ref_bf,
        (_, Some(k)) => k.ref_bf,
        _ => DVec3::ZERO,
    };
    let b_prev = match bk {
        Some(_) if c.bandit.spawn_seq == c.prev.bandit_spawn_seq => c.prev.bandit_ref_bf,
        Some(k) => k.ref_bf,
        None => DVec3::ZERO,
    };
    let caps = models.capsules();

    // Rounds.
    for i in 0..c.rounds.slots.len() {
        let r = c.rounds.slots[i];
        if !r.live() {
            continue;
        }
        if r.owner == combat_actor::PLAYER as u8
            && let Some(k) = bk
        {
            let (s0, s1) = relative_segment(r.prev_bf, r.round.pos, b_prev, k.ref_bf, k.att);
            if segment_hits_capsules(s0, s1, caps).is_some() {
                c.rounds.slots[i] = RoundSlot::FREE;
                c.stats.round_hits = c.stats.round_hits.wrapping_add(1);
                c.stamps.hit_tick = Some(tick);
                let e = round_energy_j(models.gun.mass_kg, r.round.vel, k.vel_bf);
                let range = pk.map_or(0.0, |p| (k.ref_bf - p.ref_bf).length());
                damage_bandit(c, models, e, kill_weapon::GUN, range, k.ref_bf, tick);
            }
        } else if r.owner == combat_actor::BANDIT as u8
            && let (Some(slot), Some(k)) = (player.as_deref_mut(), pk)
        {
            let (s0, s1) = relative_segment(r.prev_bf, r.round.pos, p_prev, k.ref_bf, k.att);
            if segment_hits_capsules(s0, s1, caps).is_some() {
                c.rounds.slots[i] = RoundSlot::FREE;
                let e = round_energy_j(models.gun.mass_kg, r.round.vel, k.vel_bf);
                damage_player(c, models, slot, e, k.ref_bf, tick);
            }
        }
    }

    // Missile fuzes against the bandit.
    if let Some(k) = bk {
        for i in 0..c.missiles.len() {
            let s = c.missiles[i];
            if !(s.flying() && fuze_armed(&s.m, &models.missile)) {
                continue;
            }
            let Some(d) = fuze_check(
                s.prev_bf,
                s.m.pos,
                b_prev,
                k.ref_bf,
                k.att,
                caps,
                &models.missile,
            ) else {
                continue;
            };
            c.missiles[i].end();
            c.missiles[i].m.pos = d.point_bf;
            explode(c, d.point_bf, explosion_kind::MISSILE);
            c.stats.missile_hits = c.stats.missile_hits.wrapping_add(1);
            c.events.push(
                combat_event::MISSILE_HIT as u8,
                s.owner,
                combat_actor::BANDIT as u8,
                0,
                d.miss_m,
                tick,
            );
            let e = damage_energy(&models.missile, d.miss_m);
            let range = (s.m.launch_pos - k.ref_bf).length();
            damage_bandit(c, models, e, kill_weapon::MISSILE, range, k.ref_bf, tick);
        }
    }

    // The two aircraft.
    if let (Some(slot), Some(p), Some(b)) = (player, pk, bk)
        && c.bandit.life == BanditLife::Alive
        && capsules_distance(caps, p.att, p.ref_bf, caps, b.att, b.ref_bf) <= 0.0
    {
        c.events.push(
            combat_event::COLLISION as u8,
            combat_actor::PLAYER as u8,
            combat_actor::BANDIT as u8,
            0,
            0.0,
            tick,
        );
        wreck_bandit(c, b.ref_bf, tick);
        player_destroyed(
            c,
            slot,
            crash_cause::COLLISION,
            combat_actor::BANDIT,
            p.ref_bf,
            tick,
        );
    }

    bandit_ground(c, model, svc, tick);
}

/// What the ground did to the bandit this tick.
fn bandit_ground(c: &mut Combat, model: &AircraftModel, svc: &Services, tick: u64) {
    let b = &mut c.bandit;
    if !b.present() || b.slot.crashed {
        return;
    }
    let ev = core::mem::replace(&mut b.slot.events, GroundEvents::NONE);
    let impact = ev.destroyed.is_some() || b.slot.state.numerical_fault || {
        let r = model.ref_point_bf(&b.slot.state);
        r.z < svc.geo.ground_z_bf(r.x, r.y)
    };
    if !impact {
        return;
    }
    let pos = model.ref_point_bf(&b.slot.state);
    let was_alive = b.life == BanditLife::Alive;
    let credited = b
        .damaged_by_player_tick
        .is_some_and(|d| tick.saturating_sub(d) <= u64::from(super::ticks(KILL_CREDIT_WINDOW_S)));
    b.slot.crashed = true;
    b.life = BanditLife::Absent;
    b.life_ticks = 0;
    b.fire_gun = false;
    explode(c, pos, explosion_kind::GROUND);
    if was_alive {
        c.events.push(
            combat_event::BANDIT_CRASH as u8,
            combat_actor::BANDIT as u8,
            combat_actor::NONE as u8,
            0,
            0.0,
            tick,
        );
        if credited {
            credit(c, kill_weapon::CRASH, 0.0, tick);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_round_delivers_its_kinetic_energy_relative_to_the_target() {
        // ½ · 0.26 · 700² = 63 700 J: 10 hit points of a Kestrel's 100.
        let e = round_energy_j(
            0.26,
            DVec3::new(0.0, 900.0, 0.0),
            DVec3::new(0.0, 200.0, 0.0),
        );
        assert!((e - 63_700.0).abs() < 1e-6);
        assert!((e / 6_370.0 - 10.0).abs() < 1e-9);
    }

    #[test]
    fn hits_to_destroy_at_three_speeds() {
        // GN6: 20 hits at 500 m/s, 14 at 600, 7 at 900 (½ · 0.26 · v² / 6 370 hp each).
        for (v, want) in [(500.0, 20), (600.0, 14), (900.0, 7)] {
            let mut hp = 100.0;
            let mut hits = 0;
            while hp > 0.0 {
                hp -= round_energy_j(f64::from(0.26f32), DVec3::new(v, 0.0, 0.0), DVec3::ZERO)
                    / 6_370.0;
                hits += 1;
            }
            assert_eq!(hits, want, "{v} m/s");
        }
    }
}
