//! Missiles in flight (ADR-46, ADR-48): each tick its seeker looks (the line of sight checked
//! every six ticks), the guidance commands with fresh seeker noise, the airframe flies two RK4
//! sub-steps, and it may expire. The fuze is the collision stage's ([`super::hits`]). A missile
//! that detonates or expires shows as BURST for a second, then its slot is free.

use flight::aircraft::AircraftModel;
use weapons::missile::{Expiry, Missile, MissilePhase, guide, missile_expiry, step_missile};
use weapons::seeker::{SeekerMode, SeekerState, TargetView, seeker_step};

use super::models::CombatModels;
use super::player::TargetSource;
use super::{Combat, EXPLOSION_LIFE_S, SEEKER_LOS_EVERY_TICKS};
use crate::layout::{combat_actor, combat_event, miss_reason};
use crate::math::DVec3;
use crate::rng::gaussian_pair;
use crate::services::Services;

/// One missile slot.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MissileSlot {
    /// The missile (FREE: the slot is empty).
    pub m: Missile,
    /// Where it was at the start of the tick, BF.
    pub prev_bf: DVec3,
    /// Whose it is (`combat_actor`).
    pub owner: u8,
    /// The bandit has noticed it.
    pub noticed: bool,
    /// Ticks left showing the burst (while DONE).
    pub burst_ticks: u32,
}

impl MissileSlot {
    /// A missile just off the rail.
    #[must_use]
    pub fn launched(m: Missile, owner: u8) -> Self {
        Self {
            prev_bf: m.pos,
            m,
            owner,
            noticed: false,
            burst_ticks: 0,
        }
    }

    /// The slot holds a missile (flying or bursting).
    #[must_use]
    pub fn in_use(&self) -> bool {
        self.m.phase != MissilePhase::Free
    }

    /// The missile is flying.
    #[must_use]
    pub fn flying(&self) -> bool {
        self.m.phase.flying()
    }

    /// It ends now: shown as a burst for a second.
    pub fn end(&mut self) {
        self.m.phase = MissilePhase::Done;
        self.burst_ticks = super::ticks(EXPLOSION_LIFE_S);
    }
}

/// The code of a `MISSILE_MISS` event's reason.
#[must_use]
pub const fn miss_code(e: Expiry) -> u8 {
    (match e {
        Expiry::Ground => miss_reason::GROUND,
        Expiry::Timeout => miss_reason::TIME,
        Expiry::Slow => miss_reason::SLOW,
        Expiry::TargetLost => miss_reason::TARGET_GONE,
    }) as u8
}

/// Advances every flying missile one tick from the start of the tick, against the bandit while it
/// is alive.
pub(crate) fn advance(
    c: &mut Combat,
    models: &CombatModels,
    model: &AircraftModel,
    svc: &Services,
    target: Option<&TargetSource>,
    tick: u64,
) {
    let dt = model.tick_dt_s();
    let t = crate::time::sim_time(tick);
    let env = svc.flight_env();
    let params = &models.missile;
    for s in c.missiles.iter_mut().filter(|s| s.flying()) {
        s.prev_bf = s.m.pos;
        let m = &mut s.m;
        let view: Option<TargetView> = target.map(|b| b.view(models, m.pos));
        if let Some(tv) = &view {
            m.target_lost_s = 0.0;
            if m.seeker.mode == SeekerMode::Off {
                m.seeker = SeekerState::searching();
            }
            let los = if tick.is_multiple_of(SEEKER_LOS_EVERY_TICKS) {
                svc.geo.los(m.pos, tv.pos_bf, 1.0).visible
            } else {
                m.seeker.los_clear
            };
            let axis = m.vel;
            seeker_step(&mut m.seeker, &params.seeker, axis, m.pos, tv, los);
        } else {
            m.target_lost_s += dt;
            m.seeker = SeekerState {
                los_clear: m.seeker.los_clear,
                ..SeekerState::searching()
            };
        }
        let (n1, n2) = gaussian_pair(&mut c.rng.missile);
        let a_cmd = view.as_ref().map_or(DVec3::ZERO, |tv| {
            guide(m, params, tv, [n1, n2], &env, model.atmosphere(), t)
        });
        step_missile(m, params, a_cmd, &env, model.atmosphere(), t, dt);
        if let Some(e) = missile_expiry(m, params, &env, view.is_some()) {
            s.end();
            c.events.push(
                combat_event::MISSILE_MISS as u8,
                s.owner,
                combat_actor::BANDIT as u8,
                miss_code(e),
                0.0,
                tick,
            );
        }
    }
}

/// Counts the bursts down and frees their slots.
pub(crate) fn age_bursts(c: &mut Combat) {
    for s in c
        .missiles
        .iter_mut()
        .filter(|s| s.m.phase == MissilePhase::Done)
    {
        s.burst_ticks = s.burst_ticks.saturating_sub(1);
        if s.burst_ticks == 0 {
            *s = MissileSlot::default();
        }
    }
}
