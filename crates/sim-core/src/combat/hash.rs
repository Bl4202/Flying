//! The combat column of the state hash (ADR-23 amendment 1, ADR-48): every number of the combat
//! state in a fixed order, and nothing observational (the event ring, the counters, the stamps).

use weapons::ballistics::Round;
use weapons::missile::Missile;
use weapons::seeker::SeekerState;

use super::Combat;
use super::player::GunState;
use crate::hash::StateHasher;
use crate::math::DVec3;
use crate::ownship::hash_aircraft;

/// A tag so a combat state never hashes like the no-combat constant ("COMBAT01" in ASCII).
const COMBAT_TAG: u64 = 0x3130_5441_424D_4F43;

fn v3(h: &mut StateHasher, v: DVec3) {
    h.write_f64s(&v.to_array());
}

fn opt_v3(h: &mut StateHasher, v: Option<DVec3>) {
    h.write_u64(u64::from(v.is_some()));
    v3(h, v.unwrap_or(DVec3::ZERO));
}

fn opt_tick(h: &mut StateHasher, t: Option<u64>) {
    h.write_u64(t.unwrap_or(u64::MAX));
}

fn gun(h: &mut StateHasher, g: &GunState) {
    h.write_u64(u64::from(g.rounds) | u64::from(g.firing) << 16 | u64::from(g.fired) << 32);
    h.write_f64(g.acc);
}

fn seeker(h: &mut StateHasher, s: SeekerState) {
    h.write_u64(
        u64::from(s.mode.code()) | u64::from(s.los_clear) << 8 | u64::from(s.tone_ticks) << 32,
    );
}

fn round(h: &mut StateHasher, r: &Round) {
    v3(h, r.pos);
    v3(h, r.vel);
    h.write_f64(r.age_s);
}

fn missile(h: &mut StateHasher, m: &Missile) {
    h.write_u64(u64::from(m.phase.code()));
    v3(h, m.pos);
    v3(h, m.vel);
    h.write_f64s(&[m.mass, m.t_s, m.flown_m, m.target_lost_s]);
    v3(h, m.a_lat);
    seeker(h, m.seeker);
    v3(h, m.launch_pos);
}

impl Combat {
    /// The combat column.
    #[must_use]
    pub fn hash(&self) -> u64 {
        let mut h = StateHasher::new();
        h.write_u64(COMBAT_TAG);
        h.write_u64(
            u64::from(self.rank)
                | u64::from(self.gci) << 8
                | u64::from(self.unarmed) << 12
                | u64::from(self.brain_kind as u8) << 16
                | u64::from(self.activated) << 24
                | u64::from(self.airborne_ticks) << 32,
        );
        v3(&mut h, self.cap_centre_bf);
        self.hash_bandit(&mut h);
        self.hash_player(&mut h);
        self.hash_flying(&mut h);
        self.hash_perception(&mut h);
        for r in [
            &self.rng.gun,
            &self.rng.bandit_gun,
            &self.rng.bandit,
            &self.rng.missile,
            &self.rng.spawn,
        ] {
            for w in r.state() {
                h.write_u64(w);
            }
        }
        let p = &self.prev;
        v3(&mut h, p.player_ref_bf);
        v3(&mut h, p.bandit_ref_bf);
        h.write_u64(u64::from(p.player_spawn_seq) | u64::from(p.bandit_spawn_seq) << 8);
        h.finish()
    }

    fn hash_bandit(&self, h: &mut StateHasher) {
        let b = &self.bandit;
        hash_aircraft(&b.slot, h);
        b.input.hash_into(h);
        for w in b.brain.words() {
            h.write_u64(w);
        }
        h.write_u64(
            u64::from(b.life.code())
                | u64::from(b.spawn_seq) << 8
                | u64::from(b.fire_gun) << 24
                | u64::from(b.life_ticks) << 32,
        );
        h.write_f64(b.health_hp);
        gun(h, &b.gun);
        opt_tick(h, b.damaged_by_player_tick);
        h.write_u64(u64::from(b.frame_seq));
    }

    fn hash_player(&self, h: &mut StateHasher) {
        let p = &self.player;
        h.write_f64s(&[p.health_hp, p.signal]);
        gun(h, &p.gun);
        h.write_u64(
            u64::from(p.missiles_left)
                | u64::from(p.next_rail) << 8
                | u64::from(p.armed) << 16
                | u64::from(p.spawn_seq) << 24
                | u64::from(p.airborne_ticks) << 32,
        );
        seeker(h, p.seeker);
        let c = &self.cues;
        h.write_u64(
            u64::from(c.target)
                | u64::from(c.shoot_gun) << 1
                | u64::from(c.in_range) << 2
                | u64::from(c.shoot_missile) << 3,
        );
        h.write_f64s(&[c.range_m, c.closure_mps, c.aspect_rad]);
        opt_v3(h, c.pipper_bf);
    }

    fn hash_flying(&self, h: &mut StateHasher) {
        for s in &self.rounds.slots {
            h.write_u64(u64::from(s.owner) | u64::from(s.tracer) << 8);
            if s.live() {
                round(h, &s.round);
                v3(h, s.prev_bf);
            }
        }
        h.write_u64(u64::from(self.rounds.dropped));
        for s in &self.missiles {
            missile(h, &s.m);
            v3(h, s.prev_bf);
            h.write_u64(
                u64::from(s.owner) | u64::from(s.noticed) << 8 | u64::from(s.burst_ticks) << 32,
            );
        }
        for e in &self.explosions {
            v3(h, e.pos_bf);
            h.write_f64(e.age_s);
            h.write_u64(u64::from(e.kind));
        }
    }

    fn hash_perception(&self, h: &mut StateHasher) {
        let p = &self.perception;
        for s in &p.ring {
            h.write_u64(s.tick);
            v3(h, s.pos_bf);
            v3(h, s.vel_bf);
            v3(h, s.nose_bf);
        }
        h.write_u64(
            u64::from(p.head)
                | u64::from(p.len) << 8
                | u64::from(p.los_player) << 16
                | u64::from(p.sees) << 17,
        );
        h.write_f64(p.clearance_min_m);
        h.write_u64(u64::from(p.gci.is_some()));
        if let Some(g) = p.gci {
            h.write_f64(g.t_s);
            v3(h, g.pos_bf);
            v3(h, g.vel_bf);
        }
    }
}
