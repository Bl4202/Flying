//! The player's weapons (ADR-48): arming, the gun, the next missile's seeker on the rail, launches
//! and their refusals, and the cues the HUD shows and the bot reads (the target, the gunsight's
//! pipper, `IN RNG` from the fly-out predictor, `SHOOT`).

use flight::aircraft::AircraftModel;
use weapons::missile::{LaunchRefusal, launch};
use weapons::predict::fly_out;
use weapons::seeker::{SeekerMode, SeekerState, TargetView, seeker_step};
use weapons::sight::{Mover, Shooter, gun_solution};

use super::models::CombatModels;
use super::{
    Combat, GUN_RANGING_CONE_RAD, GUN_RANGING_M, MISSILE_CAPACITY, MissileSlot,
    PREDICTOR_EVERY_TICKS, PREDICTOR_MAX_RANGE_M, SEEKER_LOS_EVERY_TICKS, TARGET_DESIGNATE_M,
};
use crate::layout::{combat_actor, combat_event, launch_inhibit};
use crate::math::DVec3;
use crate::ownship::AircraftSlot;
use crate::services::Services;

/// `SHOOT` for the gun: the bandit within this range ... // DATA: ADR-48
pub const SHOOT_GUN_RANGE_M: f64 = 1_000.0;
/// ... and within this angle of the pipper, rad. // DATA: ADR-48 (the bandit's firing window)
pub const SHOOT_GUN_AIM_RAD: f64 = ai_pilot::bandit::FIRE_AIM_RAD;

/// A gun's ammunition and rate-of-fire memory.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GunState {
    /// Rounds left.
    pub rounds: u16,
    /// Rounds owed to the rate of fire, carried between ticks.
    pub acc: f64,
    /// The trigger was held at the last tick (with rounds left).
    pub firing: bool,
    /// Rounds fired since it was loaded (every `tracer_every`-th is a tracer).
    pub fired: u32,
}

impl GunState {
    /// Loaded with `rounds`.
    #[must_use]
    pub const fn full(rounds: u16) -> Self {
        Self {
            rounds,
            acc: 0.0,
            firing: false,
            fired: 0,
        }
    }
}

/// The player's weapons.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayerWeapons {
    /// Hit points left.
    pub health_hp: f64,
    /// The gun.
    pub gun: GunState,
    /// Missiles on the rails.
    pub missiles_left: u8,
    /// The rail the next missile leaves from (index into the models' rails).
    pub next_rail: u8,
    /// The next missile's seeker (OFF with no missile left).
    pub seeker: SeekerState,
    /// Its signal ratio at the last tick.
    pub signal: f64,
    /// Ticks continuously off the wheels.
    pub airborne_ticks: u32,
    /// Weapons are armed.
    pub armed: bool,
    /// The aircraft's spawn counter the weapons were loaded for.
    pub spawn_seq: u8,
}

impl PlayerWeapons {
    /// Loaded, for the life `spawn_seq`.
    #[must_use]
    pub fn new(models: &CombatModels, spawn_seq: u8) -> Self {
        Self {
            health_hp: models.hp,
            gun: GunState::full(models.gun_rounds),
            missiles_left: models.rail_count,
            next_rail: 0,
            seeker: if models.rail_count > 0 {
                SeekerState::searching()
            } else {
                SeekerState::default()
            },
            signal: 0.0,
            airborne_ticks: 0,
            armed: false,
            spawn_seq,
        }
    }
}

/// What the player's instruments show at the end of the tick (and the bot reads at the next).
#[allow(clippy::struct_excessive_bools, reason = "one flag per cue")]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Cues {
    /// The bandit is designated (alive within 20 km).
    pub target: bool,
    /// Its range, m.
    pub range_m: f64,
    /// The closure, m/s (positive closing).
    pub closure_mps: f64,
    /// The player's aspect seen from the bandit's tail, rad.
    pub aspect_rad: f64,
    /// The gunsight's aim point, BF.
    pub pipper_bf: Option<DVec3>,
    /// The bandit is on the pipper in gun range.
    pub shoot_gun: bool,
    /// The fly-out predictor says a missile launched now would hit.
    pub in_range: bool,
    /// Locked, in range, and a launch now would go.
    pub shoot_missile: bool,
}

/// An aircraft's motion as the weapons need it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Kin {
    /// Reference point, BF.
    pub ref_bf: DVec3,
    /// Velocity, BF.
    pub vel_bf: DVec3,
    /// Attitude, body to BF.
    pub att: crate::math::DQuat,
    /// Body rates.
    pub omega_body: DVec3,
}

impl Kin {
    /// Of an aircraft slot.
    #[must_use]
    pub fn of(model: &AircraftModel, slot: &AircraftSlot) -> Self {
        let b = &slot.state.body;
        Self {
            ref_bf: model.ref_point_bf(&slot.state),
            vel_bf: b.vel_bf,
            att: b.att,
            omega_body: b.omega_body,
        }
    }

    /// As a gun's shooter.
    #[must_use]
    pub const fn shooter(&self) -> Shooter {
        Shooter {
            ref_bf: self.ref_bf,
            vel_bf: self.vel_bf,
            att: self.att,
            omega_body: self.omega_body,
        }
    }

    /// The nose, BF.
    #[must_use]
    pub fn nose_bf(&self) -> DVec3 {
        self.att.rotate(DVec3::X)
    }
}

/// The angle between two directions, rad (0 when either is zero).
#[must_use]
pub fn angle_between(a: DVec3, b: DVec3) -> f64 {
    ai_pilot::geometry::angle_between(a, b)
}

/// What a seeker needs of the bandit, taken once a tick: its motion, its engine and its Mach.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TargetSource {
    /// Its motion.
    pub kin: Kin,
    /// Its engine's core thrust as a fraction of military.
    pub core: f64,
    /// Its afterburner level.
    pub ab: f64,
    /// Its Mach number.
    pub mach: f64,
}

impl TargetSource {
    /// Of the bandit's slot at time `t_s`.
    #[must_use]
    pub fn of(model: &AircraftModel, svc: &Services, slot: &AircraftSlot, t_s: f64) -> Self {
        let kin = Kin::of(model, slot);
        let engine = &slot.state.engines[0];
        let h = svc.geo.height_msl_m(kin.ref_bf);
        let a = model.atmosphere().sample(h, svc.weather.day()).a_mps;
        Self {
            kin,
            core: engine.core,
            ab: engine.ab_level,
            mach: (kin.vel_bf - svc.weather.wind_bf(kin.ref_bf, t_s)).length() / a,
        }
    }

    /// The bandit as a seeker at `viewer_bf` sees it: its infrared intensity in that direction.
    #[must_use]
    pub fn view(&self, models: &CombatModels, viewer_bf: DVec3) -> TargetView {
        let k = &self.kin;
        let aa = weapons::ir::aspect_angle_rad(k.nose_bf(), k.ref_bf, viewer_bf);
        TargetView {
            pos_bf: k.ref_bf,
            vel_bf: k.vel_bf,
            att: k.att,
            intensity: weapons::ir::intensity(&models.ir, self.core, self.ab, self.mach, aa),
        }
    }
}

/// Why a launch is refused (`launch_inhibit`), in the order SAFE, EMPTY, `NO_LOCK`, then the
/// missile's own limits; `None` if it may go.
#[must_use]
pub fn launch_inhibit(
    w: &PlayerWeapons,
    models: &CombatModels,
    range_m: f64,
    nz_g: f64,
    alpha_rad: f64,
    tas_mps: f64,
) -> Option<u8> {
    let code = if !w.armed {
        launch_inhibit::SAFE
    } else if w.missiles_left == 0 {
        launch_inhibit::EMPTY
    } else if w.seeker.mode != SeekerMode::Locked {
        launch_inhibit::NO_LOCK
    } else {
        match models
            .missile
            .launch
            .check(range_m, nz_g, alpha_rad, tas_mps)?
        {
            LaunchRefusal::TooClose => launch_inhibit::TOO_CLOSE,
            LaunchRefusal::GLimit => launch_inhibit::G_LIMIT,
            LaunchRefusal::AlphaLimit => launch_inhibit::ALPHA_LIMIT,
            LaunchRefusal::Slow => launch_inhibit::SLOW,
        }
    };
    Some(code as u8)
}

/// The next missile as it would leave its rail now.
#[must_use]
pub fn missile_on_rail(c: &Combat, models: &CombatModels, kin: &Kin) -> weapons::missile::Missile {
    let rail = models.rails[usize::from(c.player.next_rail).min(models.rails.len() - 1)];
    let pos = kin.ref_bf + kin.att.rotate(rail);
    let vel = kin.vel_bf + kin.att.rotate(kin.omega_body.cross(rail));
    launch(&models.missile, pos, vel, c.player.seeker)
}

/// The player's side of the Weapons stage after the rounds have moved: a new life's reload, the
/// arming timer, the gun, the seeker, a launch press, then the cues. `bandit` is the bandit while
/// it is alive.
#[allow(clippy::too_many_arguments)] // ALGO: the combat, both aircraft and the world
pub(crate) fn weapons_tick(
    c: &mut Combat,
    models: &CombatModels,
    model: &AircraftModel,
    svc: &Services,
    slot: &AircraftSlot,
    input: &crate::input::InputState,
    bandit: Option<&TargetSource>,
    tick: u64,
) {
    let dt = model.tick_dt_s();
    let t = crate::time::sim_time(tick + 1);
    if slot.spawn_seq != c.player.spawn_seq {
        c.player = PlayerWeapons::new(models, slot.spawn_seq);
    }
    let alive = !(slot.crashed || slot.state.numerical_fault);
    let p = &mut c.player;
    if alive && !slot.state.ground.wow {
        p.airborne_ticks = p.airborne_ticks.saturating_add(1);
    } else {
        p.airborne_ticks = 0;
    }
    p.armed = alive && p.airborne_ticks >= super::ticks(super::ARM_DELAY_S);
    let kin = Kin::of(model, slot);

    // The gun.
    let trigger = alive && p.armed && input.held(crate::layout::button::FIRE_GUN);
    let n = super::rounds::rounds_this_tick(&mut p.gun, trigger, models.gun.rate_rps, dt);
    if n > 0 {
        let added = super::rounds::fire(
            &mut c.rounds,
            &models.gun,
            &models.mount,
            &kin.shooter(),
            &mut c.player.gun,
            n,
            combat_actor::PLAYER as u8,
            &mut c.rng.gun,
        );
        c.stats.rounds_fired = c.stats.rounds_fired.wrapping_add(added);
    }

    // The seeker on the rail.
    let view = bandit.map(|b| (b.view(models, kin.ref_bf), b.kin));
    let p = &mut c.player;
    if !alive || p.missiles_left == 0 {
        p.seeker = SeekerState::default();
        p.signal = 0.0;
    } else if let Some((tv, _)) = &view {
        if p.seeker.mode == SeekerMode::Off {
            p.seeker = SeekerState::searching();
        }
        let los = if tick.is_multiple_of(SEEKER_LOS_EVERY_TICKS) {
            svc.geo.los(kin.ref_bf, tv.pos_bf, 1.0).visible
        } else {
            p.seeker.los_clear
        };
        p.signal = seeker_step(
            &mut p.seeker,
            &models.missile.seeker,
            kin.nose_bf(),
            kin.ref_bf,
            tv,
            los,
        );
    } else {
        p.seeker = SeekerState::searching();
        p.signal = 0.0;
    }

    let range = view
        .as_ref()
        .map_or(f64::INFINITY, |(tv, _)| (tv.pos_bf - kin.ref_bf).length());
    let flight_data = || {
        let tel = model.telemetry_light(&slot.state, &svc.flight_env(), t);
        (tel.nz_g, tel.air.alpha_rad, tel.air.tas_mps)
    };

    // A launch press.
    if alive && input.pressed(crate::layout::button::FIRE_MISSILE) {
        let (nz, alpha, tas) = flight_data();
        match launch_inhibit(&c.player, models, range, nz, alpha, tas) {
            Some(code) => {
                c.stamps.inhibit = Some((tick, code));
                c.events.push(
                    combat_event::LAUNCH_REFUSED as u8,
                    combat_actor::PLAYER as u8,
                    combat_actor::BANDIT as u8,
                    code,
                    range.min(f64::from(f32::MAX)),
                    tick,
                );
            }
            None => launch_now(c, models, &kin, range, tick),
        }
    }

    if alive {
        cues_tick(
            c,
            models,
            model,
            svc,
            &kin,
            view.as_ref(),
            flight_data,
            tick,
            t,
            dt,
        );
    } else {
        c.cues = Cues::default();
    }
}

/// Puts the next missile in the air.
fn launch_now(c: &mut Combat, models: &CombatModels, kin: &Kin, range: f64, tick: u64) {
    let Some(at) = c.missiles.iter().position(|m| !m.in_use()) else {
        return;
    };
    let m = missile_on_rail(c, models, kin);
    c.missiles[at] = MissileSlot::launched(m, combat_actor::PLAYER as u8);
    let p = &mut c.player;
    p.missiles_left -= 1;
    p.next_rail = p.next_rail.saturating_add(1);
    p.seeker = if p.missiles_left > 0 {
        SeekerState::searching()
    } else {
        SeekerState::default()
    };
    p.signal = 0.0;
    c.stats.missiles_fired = c.stats.missiles_fired.wrapping_add(1);
    c.events.push(
        combat_event::MISSILE_LAUNCH as u8,
        combat_actor::PLAYER as u8,
        combat_actor::BANDIT as u8,
        at.min(MISSILE_CAPACITY) as u8,
        range.min(f64::from(f32::MAX)),
        tick,
    );
}

/// The cues at the end of the tick.
#[allow(clippy::too_many_arguments)] // ALGO: the combat, the player, the bandit and the world
fn cues_tick(
    c: &mut Combat,
    models: &CombatModels,
    model: &AircraftModel,
    svc: &Services,
    kin: &Kin,
    view: Option<&(TargetView, Kin)>,
    flight_data: impl Fn() -> (f64, f64, f64),
    tick: u64,
    t: f64,
    dt: f64,
) {
    let target = view.filter(|(tv, _)| (tv.pos_bf - kin.ref_bf).length() <= TARGET_DESIGNATE_M);
    let Some((tv, bk)) = target else {
        c.cues = Cues::default();
        return;
    };
    let env = svc.flight_env();
    let rel = tv.pos_bf - kin.ref_bf;
    let range = rel.length();
    let cues = &mut c.cues;
    cues.target = true;
    cues.range_m = range;
    cues.closure_mps =
        ai_pilot::geometry::closure_mps(kin.ref_bf, kin.vel_bf, tv.pos_bf, tv.vel_bf);
    cues.aspect_rad = weapons::ir::aspect_angle_rad(bk.nose_bf(), bk.ref_bf, kin.ref_bf);
    cues.pipper_bf = (range <= GUN_RANGING_M
        && angle_between(kin.nose_bf(), rel) <= GUN_RANGING_CONE_RAD)
        .then(|| {
            gun_solution(
                &models.gun,
                &models.mount,
                &kin.shooter(),
                &Mover {
                    pos_bf: tv.pos_bf,
                    vel_bf: tv.vel_bf,
                },
                &env,
                model.atmosphere(),
                t,
            )
        })
        .flatten()
        .map(|s| s.aim_point_bf);
    cues.shoot_gun = cues.pipper_bf.is_some_and(|a| {
        range <= SHOOT_GUN_RANGE_M && angle_between(a - kin.ref_bf, rel) <= SHOOT_GUN_AIM_RAD
    });
    if tick.is_multiple_of(PREDICTOR_EVERY_TICKS) {
        let fly = c.player.missiles_left > 0 && range <= PREDICTOR_MAX_RANGE_M;
        let m = missile_on_rail(c, models, kin);
        c.cues.in_range = fly
            && fly_out(
                &models.missile,
                m,
                tv,
                models.capsules(),
                &env,
                model.atmosphere(),
                t,
                dt,
            );
    }
    let p = &c.player;
    let ready =
        p.seeker.mode == SeekerMode::Locked && c.cues.in_range && p.armed && p.missiles_left > 0;
    c.cues.shoot_missile = ready && {
        let (nz, alpha, tas) = flight_data();
        launch_inhibit(p, models, range, nz, alpha, tas).is_none()
    };
}
