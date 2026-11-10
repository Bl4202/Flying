//! The frames of the duel scripts (ADR-47, ADR-48): SORTIE (the bot playtester) and HOLD /
//! `HOLD_FIRE` (straight and level; the missile button pressed once at the first lock). The core
//! makes them in Ingest from the aircraft and the combat state, like the circuit tester's.

use ai_pilot::{Airframe, BanditView, HoldCourse, OwnInputs, SortieOut, SortieTester};
use fcs::FcsModel;
use fcs::tester::{Levers, Presses};
use flight::aircraft::AircraftModel;
use flight::controls::PilotControls;
use weapons::seeker::SeekerMode;

use super::{BanditLife, Combat};
use crate::input::InputFrame;
use crate::layout::{axis, button};
use crate::ownship::AircraftSlot;
use crate::services::Services;
use crate::time::sim_time;

/// The levers of a slot as the testers see them.
#[must_use]
pub fn levers(slot: &AircraftSlot) -> Levers {
    Levers {
        gear_down: slot.gear_lever_down,
        flap_detent: slot.flap_detent,
        parking_brake: slot.parking_brake,
    }
}

/// An input frame from a tester's sticks and presses, plus the two trigger buttons.
#[must_use]
pub fn encode(
    tick: u64,
    c: &PilotControls,
    p: &Presses,
    fire_gun: bool,
    fire_missile: bool,
) -> InputFrame {
    let mut f = InputFrame::neutral(u32::try_from((tick + 1) & 0xFFFF_FFFF).unwrap_or(0));
    f.t_ms = tick as f64 * (1_000.0 / 60.0);
    f.axes[axis::PITCH] = c.pitch as f32;
    f.axes[axis::ROLL] = c.roll as f32;
    f.axes[axis::YAW] = c.yaw as f32;
    f.axes[axis::THROTTLE] = c.throttle as f32;
    f.axes[axis::BRAKE_L] = c.brake_l as f32;
    f.axes[axis::BRAKE_R] = c.brake_r as f32;
    let mut buttons = 0_u64;
    for (on, bit) in [
        (p.gear, button::GEAR),
        (p.flaps_down, button::FLAPS_DOWN),
        (p.flaps_up, button::FLAPS_UP),
        (p.parking_brake, button::PARKING_BRAKE),
        (c.airbrake > 0.5, button::AIRBRAKE),
        (fire_gun, button::FIRE_GUN),
        (fire_missile, button::FIRE_MISSILE),
    ] {
        if on {
            buttons |= 1 << bit;
        }
    }
    f.buttons = buttons;
    f
}

/// What the bot reads of its own aircraft and weapons.
#[must_use]
pub fn own_inputs(slot: &AircraftSlot, c: &Combat) -> OwnInputs {
    OwnInputs {
        levers: levers(slot),
        spawn_seq: slot.spawn_seq,
        locked: c.player.seeker.mode == SeekerMode::Locked,
        in_range: c.cues.in_range,
        shoot_missile: c.cues.shoot_missile,
        rounds: c.player.gun.rounds,
        missiles: c.player.missiles_left,
        pipper_bf: c.cues.pipper_bf,
    }
}

/// The bandit as the bot sees it (exactly), while there is one.
#[must_use]
pub fn bandit_view(c: &Combat, model: &AircraftModel) -> Option<BanditView> {
    let b = &c.bandit;
    b.present().then(|| BanditView {
        pos_bf: model.ref_point_bf(&b.slot.state),
        vel_bf: b.slot.state.body.vel_bf,
        att: b.slot.state.body.att,
        alive: b.life == BanditLife::Alive,
    })
}

/// The bot's frame for `tick`, from the aircraft and the combat as the last tick left them.
#[must_use]
pub fn frame(
    bot: &mut SortieTester,
    slot: &AircraftSlot,
    c: &Combat,
    model: &AircraftModel,
    fcs_model: &FcsModel,
    svc: &Services,
    tick: u64,
) -> InputFrame {
    let env = svc.flight_env();
    let af = Airframe {
        model,
        fcs: fcs_model,
        ac: &slot.state,
        fs: &slot.fcs,
        env: &env,
        t_s: sim_time(tick),
    };
    let out: SortieOut = bot.step(&own_inputs(slot, c), bandit_view(c, model).as_ref(), &af);
    encode(
        tick,
        &out.controls,
        &out.presses,
        out.fire_gun,
        out.fire_missile,
    )
}

/// The HOLD (or `HOLD_FIRE`) pilot's frame for `tick`.
#[must_use]
pub fn hold_frame(
    hold: &mut HoldCourse,
    slot: &AircraftSlot,
    c: &Combat,
    model: &AircraftModel,
    fcs_model: &FcsModel,
    svc: &Services,
    tick: u64,
) -> InputFrame {
    let env = svc.flight_env();
    let af = Airframe {
        model,
        fcs: fcs_model,
        ac: &slot.state,
        fs: &slot.fcs,
        env: &env,
        t_s: sim_time(tick),
    };
    let controls = hold.step(&af);
    let press = hold.fire(c.player.seeker.mode == SeekerMode::Locked);
    encode(tick, &controls, &Presses::default(), false, press)
}
