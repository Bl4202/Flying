//! The duel in the core: the configuration rules, the gun's rate and arming (GN5), damage (GN6),
//! kill credit (ST1's four sim cases), the reset (DT3), the hash's coverage and the snapshot.

use ai_pilot::BrainKind;

use super::hits::{damage_bandit, damage_player, round_energy_j};
use super::{AirStart, BanditLife, Combat, CombatSetup};
use crate::input::{FRAME_BYTES, InputFrame};
use crate::layout::{
    axis, button, combat_actor, combat_event, crash_cause, input_script, kill_weapon,
    snapshot as snap,
};
use crate::math::DVec3;
use crate::session::{Arena, Assist, ConfigError, GEffects, Scenario, SessionConfig, Spawn};
use crate::sim::SimCore;
use crate::test_support::{pack, slot};

fn duel(spawn: Spawn, skill: u8) -> SessionConfig {
    SessionConfig {
        spawn,
        assist: Assist::Standard,
        g_effects: GEffects::Full,
        arena: Arena::Rugged,
        scenario: Scenario::Duel,
        skill,
    }
}

/// A started duel core, set up as `setup` says.
fn duel_core(seed: u64, session: SessionConfig, setup: Option<CombatSetup>) -> SimCore {
    let mut c = SimCore::new(seed);
    c.load_content(&pack(8400)).expect("the pack loads");
    c.configure(session).expect("a duel");
    if let Some(s) = setup {
        assert!(c.combat_setup(s));
    }
    assert!(c.start());
    c
}

/// Both aircraft in the air far apart: the player at 4 000 m heading east, a non-manoeuvring
/// bandit 30 km north.
fn apart() -> CombatSetup {
    CombatSetup {
        player: Some(AirStart {
            east_m: 0.0,
            north_m: 0.0,
            alt_msl_m: 4_000.0,
            heading_rad: core::f64::consts::FRAC_PI_2,
            tas_mps: 200.0,
        }),
        bandit: Some(AirStart {
            east_m: 0.0,
            north_m: 30_000.0,
            alt_msl_m: 4_000.0,
            heading_rad: 0.0,
            tas_mps: 200.0,
        }),
        brain: BrainKind::HoldCourse,
        bandit_active: false,
        gci: false,
        unarmed: false,
    }
}

/// Neutral host frames (the throttle held, the spawn acknowledged) with `buttons` held.
fn fly(c: &mut SimCore, ticks: u32, buttons: u64) {
    for _ in 0..ticks {
        let seq = c.input().last_seq() + 1;
        let mut fr = InputFrame::neutral(seq);
        fr.axes[axis::THROTTLE] = 0.8;
        fr.spawn_ack = slot(c).spawn_seq;
        fr.buttons = buttons;
        let mut b = [0_u8; FRAME_BYTES];
        fr.encode(&mut b);
        assert_eq!(c.push_inputs(&b), 1);
        c.step(1);
    }
}

/// A change to a combat state, for the hash's coverage.
type Mutate = fn(&mut Combat);

fn combat(c: &SimCore) -> &Combat {
    c.combat().expect("a duel")
}

#[test]
fn a_duel_needs_combat_content_and_a_skill_and_its_scripts_need_a_duel() {
    // No content: no duel.
    let mut bare = SimCore::new(1);
    assert_eq!(
        bare.configure(duel(Spawn::Air, 2)),
        Err(ConfigError::Invalid)
    );
    assert!(!bare.combat_setup(CombatSetup::default()));
    // With the pack: every rank; free flight refuses the duel's knobs and scripts.
    let mut c = SimCore::new(1);
    c.load_content(&pack(8400)).unwrap();
    for skill in 1..=3 {
        assert_eq!(c.configure(duel(Spawn::Runway, skill)), Ok(()));
    }
    assert_eq!(
        c.configure(SessionConfig {
            skill: 4,
            ..duel(Spawn::Runway, 1)
        }),
        Err(ConfigError::Invalid)
    );
    c.configure(SessionConfig::default()).unwrap();
    assert!(!c.combat_setup(CombatSetup::default()));
    for kind in [
        input_script::SORTIE,
        input_script::HOLD,
        input_script::HOLD_FIRE,
    ] {
        assert!(!c.set_input_script(kind as u32), "{kind} outside a duel");
    }
    c.configure(duel(Spawn::Runway, 2)).unwrap();
    assert!(c.combat_setup(CombatSetup::default()));
    for kind in [
        input_script::SORTIE,
        input_script::HOLD,
        input_script::HOLD_FIRE,
    ] {
        assert!(c.set_input_script(kind as u32), "{kind} in a duel");
    }
    assert!(c.start());
    assert!(!c.combat_setup(CombatSetup::default()), "only while idle");
    let cb = combat(&c);
    assert_eq!((cb.rank, cb.bandit.life), (2, BanditLife::Alive));
    assert_eq!(cb.events.entries[0].kind, combat_event::BANDIT_SPAWN as u8);
    // The first bandit is on the CAP orbit at its height, at the spawn speed.
    let model = c.services().aircraft.as_ref().unwrap();
    let p = model.ref_point_bf(&cb.bandit.slot.state);
    let d = (p - cb.cap_centre_bf).length();
    assert!((d - super::CAP_RADIUS_M).abs() < 50.0, "{d}");
    let h = c.services().geo.height_msl_m(p);
    assert!((h - super::CAP_ALT_MSL_M).abs() < 1.0, "{h}");
}

#[test]
fn the_gun_is_safe_on_the_wheels_and_for_a_second_after_and_empties_in_317_ticks() {
    // GN5. On the runway: the trigger held for two seconds fires nothing.
    let mut c = duel_core(3, duel(Spawn::Runway, 1), None);
    fly(&mut c, 120, 1 << button::FIRE_GUN);
    assert_eq!(combat(&c).player.gun.rounds, 150);
    assert_eq!(combat(&c).stats.rounds_fired, 0);
    // In the air from the first tick: armed after 60 ticks; then 150 rounds in 317 ticks.
    let mut c = duel_core(3, duel(Spawn::Air, 1), Some(apart()));
    fly(&mut c, 59, 1 << button::FIRE_GUN);
    assert_eq!(
        combat(&c).player.gun.rounds,
        150,
        "not within 1 s of leaving the wheels"
    );
    assert!(!combat(&c).player.armed);
    let mut ticks = 0;
    while combat(&c).player.gun.rounds > 0 {
        fly(&mut c, 1, 1 << button::FIRE_GUN);
        ticks += 1;
        assert!(ticks < 400);
    }
    assert_eq!(ticks, 317);
    let rate = c.services().combat.as_ref().unwrap().gun.rate_rps;
    let t = f64::from(ticks) / 60.0;
    assert!((t - 150.0 / rate).abs() <= 1.0 / 60.0, "{t} s");
    assert_eq!(combat(&c).stats.rounds_fired, 150);
    fly(&mut c, 30, 1 << button::FIRE_GUN);
    assert_eq!(combat(&c).stats.rounds_fired, 150, "it stops at 0");
    // Every fourth round is a tracer; the rounds fly and expire within 3 s.
    let live = combat(&c).rounds.live();
    assert!(live > 0);
    fly(&mut c, 200, 0);
    assert_eq!(combat(&c).rounds.live(), 0);
}

#[test]
fn hits_to_destroy_the_bandit_at_three_relative_speeds() {
    // GN6, through the real models: ½ · 0.26 · v² / 6 370 hp a hit against 100 hp.
    for (v, want) in [(500.0, 20), (600.0, 14), (900.0, 7)] {
        let mut c = duel_core(4, duel(Spawn::Air, 1), Some(apart()));
        let models = c.services().combat.clone().unwrap();
        let tick = c.tick();
        let cb = c.combat_mut().unwrap();
        let mut hits = 0;
        while cb.bandit.life == BanditLife::Alive {
            let e = round_energy_j(models.gun.mass_kg, DVec3::new(v, 0.0, 0.0), DVec3::ZERO);
            damage_bandit(cb, &models, e, kill_weapon::GUN, 500.0, DVec3::ZERO, tick);
            hits += 1;
            assert!(hits < 100);
        }
        assert_eq!(hits, want, "{v} m/s");
        assert_eq!(cb.bandit.life, BanditLife::Wreck);
    }
}

/// The KILL events of a combat.
fn kills(cb: &Combat) -> Vec<u8> {
    cb.events
        .since(0)
        .filter(|e| e.kind == combat_event::KILL as u8)
        .map(|e| e.arg)
        .collect()
}

#[test]
fn kill_credit_for_the_gun_a_missile_and_a_crash_after_damage() {
    // ST1's four sim cases.
    // A gun kill.
    let mut c = duel_core(5, duel(Spawn::Air, 1), Some(apart()));
    let models = c.services().combat.clone().unwrap();
    let tick = c.tick();
    let cb = c.combat_mut().unwrap();
    damage_bandit(cb, &models, 1e9, kill_weapon::GUN, 412.0, DVec3::ZERO, tick);
    assert_eq!(kills(cb), [kill_weapon::GUN as u8]);
    assert_eq!((cb.stats.kills, cb.stats.gun_kills), (1, 1));
    let k = cb.events.since(0).last().copied().unwrap();
    assert_eq!((k.actor, k.target, k.value), (0, 1, 412.0));
    // A missile kill: a burst 2 m away is 2 548 000 · (6/8)² J = 225 hp.
    let mut c = duel_core(5, duel(Spawn::Air, 1), Some(apart()));
    let e = weapons::fuze::damage_energy(&models.missile, 2.0);
    assert!((e / models.joules_per_hp - 225.0).abs() < 1e-6);
    let cb = c.combat_mut().unwrap();
    damage_bandit(
        cb,
        &models,
        e,
        kill_weapon::MISSILE,
        2_000.0,
        DVec3::ZERO,
        tick,
    );
    assert_eq!(kills(cb), [kill_weapon::MISSILE as u8]);
    assert_eq!(cb.stats.missile_kills, 1);
    // A crash 10 s after the player damaged it is the player's; one with no damage, or 31 s
    // after, is not.
    for (damaged_s_ago, credited) in [(Some(10.0), true), (None, false), (Some(31.0), false)] {
        let mut c = duel_core(5, duel(Spawn::Air, 1), Some(apart()));
        fly(&mut c, 2_000, 0);
        let now = c.tick();
        let ground = {
            let cb = combat(&c);
            let model = c.services().aircraft.as_ref().unwrap();
            let p = model.ref_point_bf(&cb.bandit.slot.state);
            c.services().geo.ground_z_bf(p.x, p.y)
        };
        let cb = c.combat_mut().unwrap();
        if let Some(s) = damaged_s_ago {
            cb.bandit.health_hp = 60.0;
            cb.bandit.damaged_by_player_tick = Some(now - (s * 60.0) as u64);
        }
        cb.bandit.slot.state.body.pos_bf.z = ground - 20.0;
        fly(&mut c, 1, 0);
        let cb = combat(&c);
        assert_eq!(cb.bandit.life, BanditLife::Absent);
        assert!(
            cb.events
                .since(0)
                .any(|e| e.kind == combat_event::BANDIT_CRASH as u8)
        );
        let want: &[u8] = if credited {
            &[kill_weapon::CRASH as u8]
        } else {
            &[]
        };
        assert_eq!(kills(cb), want, "{damaged_s_ago:?}");
    }
}

#[test]
fn the_player_shot_down_dies_and_respawns_rearmed() {
    let mut c = duel_core(6, duel(Spawn::Air, 2), Some(apart()));
    fly(&mut c, 600, 1 << button::FIRE_GUN);
    assert!(combat(&c).player.gun.rounds < 150);
    let models = c.services().combat.clone().unwrap();
    let tick = c.tick();
    let seq = slot(&c).spawn_seq;
    {
        let cb = c.world_combat_and_slot();
        let (cb, s) = cb;
        damage_player(cb, &models, s, 1e9, DVec3::ZERO, tick);
    }
    assert!(slot(&c).crashed);
    assert_eq!(slot(&c).crash_cause, crash_cause::SHOT_DOWN as u8);
    let cb = combat(&c);
    let death = cb.events.since(0).last().copied().unwrap();
    assert_eq!(
        (death.kind, death.actor, death.arg),
        (
            combat_event::DEATH as u8,
            combat_actor::BANDIT as u8,
            crash_cause::SHOT_DOWN as u8
        )
    );
    assert_eq!(cb.stats.deaths, 1);
    fly(&mut c, 125, 0);
    assert_ne!(slot(&c).spawn_seq, seq, "it respawned");
    let p = &combat(&c).player;
    assert_eq!((p.gun.rounds, p.missiles_left), (150, 2));
    assert!((p.health_hp - models.hp).abs() < 1e-12);
}

#[test]
fn a_reset_after_a_duel_flies_as_a_fresh_core() {
    // DT3: a duel, a reset, the same duel again equals a fresh core in every column, and a free
    // session after a reset equals a fresh free core.
    let session = duel(Spawn::Runway, 2);
    let run = |c: &mut SimCore| {
        c.configure(session).unwrap();
        assert!(c.set_input_script(input_script::SORTIE as u32));
        assert!(c.start());
        c.step(3_000);
    };
    let mut a = SimCore::new(9);
    a.load_content(&pack(8400)).unwrap();
    run(&mut a);
    assert!(a.combat().is_some());
    a.reset();
    assert!(a.combat().is_none());
    run(&mut a);
    let mut b = SimCore::new(9);
    b.load_content(&pack(8400)).unwrap();
    run(&mut b);
    assert_eq!(a.subsystem_hashes(), b.subsystem_hashes());
    assert_ne!(a.subsystem_hashes().combat, crate::hash::no_combat_hash());
    assert_eq!(a.snapshot_bytes(), b.snapshot_bytes());
    a.reset();
    a.configure(SessionConfig::default()).unwrap();
    a.start();
    a.step(600);
    let mut f = SimCore::new(9);
    f.load_content(&pack(8400)).unwrap();
    f.start();
    f.step(600);
    assert_eq!(a.subsystem_hashes(), f.subsystem_hashes());
    assert_eq!(a.subsystem_hashes().combat, crate::hash::no_combat_hash());
}

#[test]
fn every_hashed_number_moves_the_combat_hash_and_no_observation_does() {
    let mut c = duel_core(7, duel(Spawn::Air, 2), Some(apart()));
    fly(&mut c, 400, 1 << button::FIRE_GUN);
    let base = combat(&c).clone();
    let h0 = base.hash();
    let changes: &[(&str, Mutate)] = &[
        ("rank", |c| c.rank = 3),
        ("gci", |c| c.gci = !c.gci),
        ("unarmed", |c| c.unarmed = !c.unarmed),
        ("brain kind", |c| c.brain_kind = BrainKind::DefendOnly),
        ("activated", |c| c.activated = !c.activated),
        ("airborne", |c| c.airborne_ticks += 1),
        ("cap", |c| c.cap_centre_bf.x += 1.0),
        ("bandit pos", |c| c.bandit.slot.state.body.pos_bf.x += 1e-6),
        ("bandit fcs", |c| c.bandit.slot.fcs.n_cmd += 1e-6),
        ("bandit input", |c| {
            c.bandit
                .input
                .push(InputFrame::neutral(c.bandit.frame_seq + 7));
        }),
        ("brain", |c| c.bandit.brain.speed_i += 1e-6),
        ("brain hold", |c| c.bandit.brain.hold.cas_mps += 1e-6),
        ("life", |c| c.bandit.life = BanditLife::Wreck),
        ("life ticks", |c| c.bandit.life_ticks += 1),
        ("bandit hp", |c| c.bandit.health_hp -= 1e-9),
        ("bandit spawn", |c| c.bandit.spawn_seq += 1),
        ("bandit rounds", |c| c.bandit.gun.rounds ^= 1),
        ("bandit acc", |c| c.bandit.gun.acc += 0.1),
        ("bandit trigger", |c| c.bandit.fire_gun = !c.bandit.fire_gun),
        ("damaged", |c| c.bandit.damaged_by_player_tick = Some(3)),
        ("frame seq", |c| c.bandit.frame_seq += 1),
        ("player hp", |c| c.player.health_hp -= 1e-9),
        ("player rounds", |c| c.player.gun.rounds ^= 1),
        ("player fired", |c| c.player.gun.fired += 1),
        ("missiles", |c| c.player.missiles_left ^= 1),
        ("rail", |c| c.player.next_rail += 1),
        ("seeker", |c| c.player.seeker.tone_ticks += 1),
        ("signal", |c| c.player.signal += 1e-6),
        ("armed", |c| c.player.armed = !c.player.armed),
        ("player airborne", |c| c.player.airborne_ticks += 1),
        ("player spawn", |c| c.player.spawn_seq += 1),
        ("cue target", |c| c.cues.target = !c.cues.target),
        ("cue range", |c| c.cues.range_m += 1e-6),
        ("cue pipper", |c| c.cues.pipper_bf = Some(DVec3::X)),
        ("cue in range", |c| c.cues.in_range = !c.cues.in_range),
        ("cue shoot", |c| {
            c.cues.shoot_missile = !c.cues.shoot_missile;
        }),
        ("round", |c| {
            let s = c.rounds.slots.iter_mut().find(|s| s.live()).unwrap();
            s.round.pos.y += 1e-6;
        }),
        ("round prev", |c| {
            let s = c.rounds.slots.iter_mut().find(|s| s.live()).unwrap();
            s.prev_bf.y += 1e-6;
        }),
        ("dropped", |c| c.rounds.dropped += 1),
        ("missile", |c| c.missiles[0].m.mass += 1.0),
        ("missile noticed", |c| c.missiles[1].noticed = true),
        ("explosion", |c| c.explosions[2].age_s += 0.1),
        ("ring", |c| c.perception.ring[0].pos_bf.x += 1e-6),
        ("ring head", |c| c.perception.head += 1),
        ("gci report", |c| {
            c.perception.gci = Some(super::perception::Gci::default());
        }),
        ("clearance", |c| c.perception.clearance_min_m += 1.0),
        ("los", |c| {
            c.perception.los_player = !c.perception.los_player;
        }),
        ("rng gun", |c| {
            c.rng.gun.next_u64();
        }),
        ("rng bandit gun", |c| {
            c.rng.bandit_gun.next_u64();
        }),
        ("rng bandit", |c| {
            c.rng.bandit.next_u64();
        }),
        ("rng missile", |c| {
            c.rng.missile.next_u64();
        }),
        ("rng spawn", |c| {
            c.rng.spawn.next_u64();
        }),
        ("prev", |c| c.prev.bandit_ref_bf.z += 1e-6),
    ];
    for (name, f) in changes {
        let mut m = base.clone();
        f(&mut m);
        assert_ne!(m.hash(), h0, "{name}");
    }
    // Observations do not move it.
    let mut m = base.clone();
    m.events.push(1, 0, 1, 0, 0.0, 5);
    m.stats.kills += 1;
    m.stamps.hit_tick = Some(9);
    assert_eq!(m.hash(), h0);
}

#[test]
fn the_combat_block_mirrors_the_state_and_is_zero_without_combat() {
    let f32_at = |b: &[u8], o: usize| f32::from_le_bytes(b[o..o + 4].try_into().unwrap());
    let f64_at = |b: &[u8], o: usize| f64::from_le_bytes(b[o..o + 8].try_into().unwrap());
    let u32_at = |b: &[u8], o: usize| u32::from_le_bytes(b[o..o + 4].try_into().unwrap());
    let mut free = SimCore::new(1);
    free.load_content(&pack(8400)).unwrap();
    free.start();
    free.step(10);
    assert!(
        free.snapshot_bytes()[snap::COMBAT..]
            .iter()
            .all(|&x| x == 0)
    );

    let mut c = duel_core(8, duel(Spawn::Air, 2), Some(apart()));
    fly(&mut c, 300, 1 << button::FIRE_GUN);
    let b = c.snapshot_bytes();
    let cb = combat(&c);
    let model = c.services().aircraft.as_ref().unwrap();
    let flags = u16::from_le_bytes([b[snap::C_FLAGS], b[snap::C_FLAGS + 1]]);
    assert_ne!(flags & crate::layout::combat_flag::ACTIVE as u16, 0);
    assert_ne!(flags & crate::layout::combat_flag::ARMED as u16, 0);
    assert_ne!(flags & crate::layout::combat_flag::GUN_FIRING as u16, 0);
    assert_eq!(b[snap::C_SEEKER], cb.player.seeker.mode.code());
    assert_eq!(
        u16::from_le_bytes([b[snap::C_ROUNDS], b[snap::C_ROUNDS + 1]]),
        cb.player.gun.rounds
    );
    assert_eq!((b[snap::C_MISSILES], b[snap::C_BANDITS]), (2, 1));
    assert_eq!(u32_at(b, snap::C_EVENT_SEQ), cb.events.seq);
    assert_eq!(u32_at(b, snap::C_ROUNDS_FIRED), cb.stats.rounds_fired);
    assert_eq!(f32_at(b, snap::C_HEALTH), 1.0);
    let p = model.ref_point_bf(&cb.bandit.slot.state);
    assert_eq!(f64_at(b, snap::BD_POS_BF), p.x);
    assert_eq!(f64_at(b, snap::BD_POS_BF + 16), p.z);
    assert_eq!(
        f32_at(b, snap::BD_VEL_BF + 4),
        cb.bandit.slot.state.body.vel_bf.y as f32
    );
    assert_eq!(b[snap::BD_LIFE], BanditLife::Alive.code());
    assert_eq!(
        (b[snap::BD_AI], b[snap::BD_SKILL]),
        (cb.bandit.brain.state.code(), 2)
    );
    // The tracers are every fourth live round, in pool order, with their pool index.
    let n = u32_at(b, snap::TR_COUNT) as usize;
    let tracers: Vec<usize> = cb
        .rounds
        .slots
        .iter()
        .enumerate()
        .filter(|(_, s)| s.live() && s.tracer)
        .map(|(i, _)| i)
        .take(super::TRACER_CAPACITY)
        .collect();
    assert_eq!(n, tracers.len());
    assert!(n > 5, "{n}");
    for (k, &i) in tracers.iter().enumerate() {
        let at = snap::TR_FIRST + k * snap::TRACER_STRIDE;
        assert_eq!(usize::from(b[at + snap::TR_ID]), i);
        assert_eq!(
            f64_at(b, at + snap::TR_POS_BF),
            cb.rounds.slots[i].round.pos.x
        );
        assert_eq!(b[at + snap::TR_OWNER], combat_actor::PLAYER as u8);
    }
    // The spawn event is the first entry.
    assert_eq!(u32_at(b, snap::EVENTS + snap::EV_SEQ), 1);
    assert_eq!(
        b[snap::EVENTS + snap::EV_KIND],
        combat_event::BANDIT_SPAWN as u8
    );
}
