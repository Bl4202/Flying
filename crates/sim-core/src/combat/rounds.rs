//! Rounds in flight: a fixed pool shared by both guns, and the gun's rate of fire (ADR-46, GDD
//! 15.4).
//!
//! A tick first advances the rounds already flying (two RK2 sub-steps, from where they were at
//! the start of the tick), then the guns fire new rounds from the muzzles where the aircraft now
//! are. A new round's path this tick is its muzzle point, so the swept hit test of the collision
//! stage covers every round over exactly the interval the aircraft moved.

use flight::atmosphere::Atmosphere;
use flight::env::FlightEnv;
use weapons::ballistics::{
    GunBallistics, Round, dispersed_dir_body, gun_axis_body, round_expired, spawn_round, step_round,
};
use weapons::sight::{GunMount, Shooter};

use super::ROUND_CAPACITY;
use super::player::GunState;
use crate::layout::combat_actor;
use crate::math::DVec3;
use crate::rng::{Xoshiro256StarStar, gaussian_pair};

/// One entry of the pool.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RoundSlot {
    /// The round.
    pub round: Round,
    /// Where it was at the start of the tick, BF.
    pub prev_bf: DVec3,
    /// Whose it is (`combat_actor`; NONE: the entry is free).
    pub owner: u8,
    /// It is a tracer.
    pub tracer: bool,
}

impl RoundSlot {
    /// A free entry.
    pub const FREE: Self = Self {
        round: Round {
            pos: DVec3::ZERO,
            vel: DVec3::ZERO,
            age_s: 0.0,
        },
        prev_bf: DVec3::ZERO,
        owner: combat_actor::NONE as u8,
        tracer: false,
    };

    /// The entry holds a round.
    #[must_use]
    pub const fn live(&self) -> bool {
        self.owner != combat_actor::NONE as u8
    }
}

/// The pool.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RoundPool {
    /// The entries, in index order.
    pub slots: [RoundSlot; ROUND_CAPACITY],
    /// Rounds that found the pool full (and were not fired). Never expected at the guns' rates.
    pub dropped: u32,
}

impl Default for RoundPool {
    fn default() -> Self {
        Self::new()
    }
}

impl RoundPool {
    /// An empty pool.
    #[must_use]
    #[allow(
        clippy::large_stack_arrays,
        reason = "built once, into the boxed combat state"
    )]
    pub const fn new() -> Self {
        Self {
            slots: [RoundSlot::FREE; ROUND_CAPACITY],
            dropped: 0,
        }
    }

    /// Rounds in flight.
    #[must_use]
    pub fn live(&self) -> usize {
        self.slots.iter().filter(|s| s.live()).count()
    }

    /// Advances every round one tick from `t_s` and frees those that expire.
    pub fn advance<E: FlightEnv>(
        &mut self,
        g: &GunBallistics,
        env: &E,
        atmo: &Atmosphere,
        t_s: f64,
        dt: f64,
    ) {
        for s in self.slots.iter_mut().filter(|s| s.live()) {
            s.prev_bf = s.round.pos;
            step_round(&mut s.round, g, env, atmo, t_s, dt);
            if round_expired(&s.round, g, env) || !s.round.pos.is_finite() {
                *s = RoundSlot::FREE;
            }
        }
    }

    /// Puts a round in the first free entry; false if the pool is full.
    pub fn add(&mut self, round: Round, owner: u8, tracer: bool) -> bool {
        if let Some(s) = self.slots.iter_mut().find(|s| !s.live()) {
            *s = RoundSlot {
                round,
                prev_bf: round.pos,
                owner,
                tracer,
            };
            true
        } else {
            self.dropped = self.dropped.wrapping_add(1);
            false
        }
    }
}

/// How many rounds a gun fires this tick (GDD 15.4, ADR-48): on the trigger's first tick one at
/// once; then `rate · dt` a tick, carried over; none once the trigger is released or the rounds
/// run out. 150 rounds take 317 ticks (5.28 s; `150 / 28.33` is 5.29 s, GN5).
pub fn rounds_this_tick(gun: &mut GunState, trigger: bool, rate_rps: f64, dt: f64) -> u32 {
    if !trigger || gun.rounds == 0 {
        gun.acc = 0.0;
        gun.firing = false;
        return 0;
    }
    if gun.firing {
        gun.acc += rate_rps * dt;
    } else {
        gun.acc = 1.0;
        gun.firing = true;
    }
    let mut n = 0;
    while gun.acc >= 1.0 && gun.rounds > 0 {
        gun.acc -= 1.0;
        gun.rounds -= 1;
        n += 1;
    }
    n
}

/// Fires `n` rounds from a shooter into the pool, each turned by the gun's dispersion (two normals
/// from `rng`); every `tracer_every`-th round of the gun's count is a tracer. Returns how many
/// went into the pool.
#[allow(clippy::too_many_arguments)] // ALGO: the gun, the shooter and the pool
pub fn fire(
    pool: &mut RoundPool,
    g: &GunBallistics,
    mount: &GunMount,
    shooter: &Shooter,
    gun: &mut GunState,
    n: u32,
    owner: u8,
    rng: &mut Xoshiro256StarStar,
) -> u32 {
    let axis = gun_axis_body(mount.harmonisation_rad);
    let mut added = 0;
    for _ in 0..n {
        let (n1, n2) = gaussian_pair(rng);
        let dir = dispersed_dir_body(axis, g.dispersion_rad, [n1, n2]);
        let round = spawn_round(
            g,
            shooter.ref_bf,
            shooter.vel_bf,
            shooter.att,
            shooter.omega_body,
            mount.muzzle_m,
            dir,
        );
        gun.fired = gun.fired.wrapping_add(1);
        let tracer = g.tracer_every > 0 && gun.fired.is_multiple_of(u32::from(g.tracer_every));
        if pool.add(round, owner, tracer) {
            added += 1;
        }
    }
    added
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::player::GunState;

    #[test]
    fn a_full_gun_empties_in_317_ticks_and_stops_at_zero() {
        // GN5: 1 700 rounds a minute, 150 rounds.
        let rate = 1_700.0 / 60.0;
        let dt = 1.0 / 60.0;
        let mut g = GunState::full(150);
        let mut ticks = 0;
        let mut fired = 0;
        while g.rounds > 0 {
            fired += rounds_this_tick(&mut g, true, rate, dt);
            ticks += 1;
            assert!(ticks < 1_000);
        }
        assert_eq!((fired, ticks), (150, 317));
        let t = f64::from(ticks) * dt;
        assert!((t - 150.0 / rate).abs() <= dt, "{t}");
        // Empty: nothing more, however long the trigger is held.
        assert_eq!(rounds_this_tick(&mut g, true, rate, dt), 0);
        // A tap fires one round at once; a release resets the carry.
        let mut g = GunState::full(150);
        assert_eq!(rounds_this_tick(&mut g, true, rate, dt), 1);
        assert_eq!(rounds_this_tick(&mut g, false, rate, dt), 0);
        assert_eq!((g.acc, g.firing, g.rounds), (0.0, false, 149));
        assert_eq!(rounds_this_tick(&mut g, true, rate, dt), 1);
    }

    #[test]
    fn the_pool_fills_in_index_order_and_frees_on_expiry() {
        let mut p = RoundPool::new();
        let r = Round {
            pos: DVec3::new(0.0, 0.0, 1_000.0),
            vel: DVec3::new(0.0, 900.0, 0.0),
            age_s: 0.0,
        };
        for _ in 0..ROUND_CAPACITY {
            assert!(p.add(r, combat_actor::PLAYER as u8, false));
        }
        assert!(!p.add(r, combat_actor::BANDIT as u8, false));
        assert_eq!((p.live(), p.dropped), (ROUND_CAPACITY, 1));
        p.slots[3] = RoundSlot::FREE;
        assert!(p.add(r, combat_actor::BANDIT as u8, true));
        assert_eq!(p.slots[3].owner, combat_actor::BANDIT as u8);
        assert_eq!(p.slots[3].prev_bf, r.pos);
    }
}
