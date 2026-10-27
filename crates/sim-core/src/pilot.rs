//! The pilot's side of the aircraft's tick (ADR-34, ADR-36): the levers, the controls the input
//! frame gives, what the pilot's body does to them, the control system, and the flight model's
//! tick.
//!
//! ```text
//! buttons ─▶ levers (gear, flaps, parking brake)         assist request ─▶ bumpless switch
//! axes ─▶ controls ─▶ blackout delay / G-LOC ─▶ FCS (once per tick) ─▶ flight model (2 RK4 steps)
//!                                                                           │
//!                                  physiology and structure ◀── load factor ─┘
//! ```
//!
//! Nothing here allocates: the pilot's ring of recent stick positions is a fixed array in the slot.

use fcs::{Aim, FcsModel, PilotInput};
use flight::aircraft::AircraftModel;
use flight::controls::PilotControls;
use flight::physio::{MAX_DELAY_TICKS, PhysioParams};

use crate::input::InputState;
use crate::layout::{axis, button, input_flag};
use crate::ownship::AircraftSlot;
use crate::services::Services;
use crate::spawn::FLAP_DETENT_FRACTION;
use crate::world::Counters;

/// The highest flap detent: 0 up, 1 take-off, 2 landing.
const FLAP_DETENT_MAX: u8 = 2;

/// Works the levers the buttons press: the gear lever (up is refused on the wheels), the flap
/// lever, the parking brake and the trim reset.
pub fn work_levers(slot: &mut AircraftSlot, input: &InputState) {
    if input.pressed(button::GEAR) {
        if !slot.gear_lever_down {
            slot.gear_lever_down = true;
        } else if !(slot.state.ground.wow || slot.fcs.on_ground) {
            slot.gear_lever_down = false;
        }
    }
    let mut detent = slot.flap_detent;
    if input.pressed(button::FLAPS_DOWN) {
        detent = (detent + 1).min(FLAP_DETENT_MAX);
    }
    if input.pressed(button::FLAPS_UP) {
        detent = detent.saturating_sub(1);
    }
    if input.pressed(button::FLAPS_CYCLE) {
        detent = (detent + 1) % (FLAP_DETENT_MAX + 1);
    }
    slot.flap_detent = detent;
    if input.pressed(button::PARKING_BRAKE) {
        slot.parking_brake = !slot.parking_brake;
    }
    if input.pressed(button::TRIM_RESET) {
        slot.state.trim_pitch_rad = 0.0;
    }
}

/// The controls the input frame and the levers give. The throttle is the held spawn setting until
/// the host acknowledges the spawn; the parking brake sets both wheel brakes.
#[must_use]
pub fn controls(slot: &mut AircraftSlot, input: &InputState) -> PilotControls {
    if slot.throttle_hold.is_some() && input.spawn_ack() == slot.spawn_seq {
        slot.throttle_hold = None;
    }
    let throttle = slot.throttle_hold.unwrap_or_else(|| input.throttle_cmd());
    let park = if slot.parking_brake { 1.0 } else { 0.0 };
    PilotControls {
        pitch: input.axis(axis::PITCH),
        roll: input.axis(axis::ROLL),
        yaw: input.axis(axis::YAW),
        throttle,
        trim_nose_up: input.held(button::TRIM_NOSE_UP),
        trim_nose_down: input.held(button::TRIM_NOSE_DOWN),
        flaps: FLAP_DETENT_FRACTION[usize::from(slot.flap_detent.min(FLAP_DETENT_MAX))],
        airbrake: if input.held(button::AIRBRAKE) {
            1.0
        } else {
            0.0
        },
        gear: if slot.gear_lever_down { 1.0 } else { 0.0 },
        brake_l: input.axis(axis::BRAKE_L).max(park),
        brake_r: input.axis(axis::BRAKE_R).max(park),
    }
}

/// What the pilot's body does to the stick and pedals (GDD 11.14): in a blackout they arrive late,
/// in G-LOC not at all, and for the ramp after it only in part (and the trim switches are not
/// worked). The ring remembers what the pilot did, whatever reached the aircraft.
pub fn filter(slot: &mut AircraftSlot, c: &mut PilotControls) {
    let n = MAX_DELAY_TICKS;
    let head = usize::from(slot.ring_head) % n;
    let delay = slot.effects.delay_ticks.min(n);
    let sticks = if delay > 0 {
        slot.stick_ring[(head + n - delay) % n]
    } else {
        [c.pitch, c.roll, c.yaw]
    };
    slot.stick_ring[head] = [c.pitch, c.roll, c.yaw];
    slot.ring_head = ((head + 1) % n) as u8;
    let k = slot.effects.control_scale.clamp(0.0, 1.0);
    c.pitch = sticks[0] * k;
    c.roll = sticks[1] * k;
    c.yaw = sticks[2] * k;
    if slot.effects.gloc {
        c.trim_nose_up = false;
        c.trim_nose_down = false;
    }
}

/// The mouse-aim direction the frame carries (`CURSOR_X = azimuth / pi`, `CURSOR_Y = elevation /
/// (pi / 2)`), unless the pilot is out.
#[must_use]
pub fn aim_from_frame(input: &InputState, gloc: bool) -> Option<Aim> {
    if gloc || !input.has_flag(input_flag::MOUSE_AIM) {
        return None;
    }
    Some(Aim {
        azimuth_rad: input.axis(axis::CURSOR_X) * core::f64::consts::PI,
        elevation_rad: input.axis(axis::CURSOR_Y) * core::f64::consts::FRAC_PI_2,
    })
}

/// One tick of the aircraft: levers, assist request, controls, the control system, the flight
/// model, then the pilot's physiology and the structure with the load factor the tick gave. The
/// ground events wait in the slot for the contact stage.
pub fn fly(
    slot: &mut AircraftSlot,
    model: &AircraftModel,
    fcs_model: &FcsModel,
    svc: &Services,
    input: &InputState,
    counters: &mut Counters,
    t_s: f64,
) {
    // A pilot who is out reaches for nothing: the levers and the trim switches do not move. The
    // assist level is chosen from the menu, so that still works.
    let pilot_out = slot.effects.gloc;
    if !pilot_out {
        work_levers(slot, input);
    }
    if let Some(level) = fcs::Assist::from_code(input.assist_req())
        && level != slot.fcs.assist
    {
        fcs_model.switch_assist(&mut slot.fcs, level);
        counters.assist_changes = counters.assist_changes.wrapping_add(1);
    }

    let mut c = controls(slot, input);
    slot.throttle = c.throttle;
    slot.brake_cmd = [c.brake_l, c.brake_r];
    filter(slot, &mut c);
    let aim = aim_from_frame(input, pilot_out);
    slot.aim = aim.map(|a| [a.azimuth_rad, a.elevation_rad]);
    let pilot = PilotInput {
        controls: c,
        aim,
        recover: input.held(button::RECOVER) && !pilot_out,
        gloc: pilot_out,
        auto_gcas: input.has_flag(input_flag::AUTO_GCAS),
    };

    let env = svc.flight_env();
    let cmd = fcs_model.step(model, &mut slot.state, &mut slot.fcs, &pilot, &env, t_s);
    let out = model.step_tick(&mut slot.state, &cmd, &env, t_s);
    counters.fdm_faults = counters.fdm_faults.wrapping_add(out.faults);
    slot.events = out.events;

    let dt = model.tick_dt_s();
    let effects = slot.physio.step(
        out.nz_g,
        dt,
        &PhysioParams::GDD,
        slot.session.g_effects.to_flight(),
    );
    if effects.gloc_started {
        counters.gloc_events = counters.gloc_events.wrapping_add(1);
    }
    slot.effects = effects;
    let tel = model.telemetry_light(&slot.state, &env, t_s + dt);
    let load = model.structure_input(&slot.state, &tel);
    slot.warnings = slot.structure.step(&model.limits, &load, dt);
}

#[cfg(test)]
mod tests {
    use super::*;
    use flight::physio::PhysioEffects;

    use crate::input::InputFrame;
    use crate::session::{SessionConfig, Spawn};
    use crate::test_support::flying_under;

    /// A slot to work the levers of; only the pilot's side is under test, so any state will do.
    fn slot() -> Box<AircraftSlot> {
        let c = flying_under(
            1,
            SessionConfig {
                spawn: Spawn::Air,
                ..SessionConfig::default()
            },
        );
        Box::new(*crate::test_support::slot(&c))
    }

    fn press(bits: &[usize]) -> InputState {
        let mut input = InputState::new();
        let mut f = InputFrame::neutral(1);
        for &b in bits {
            f.buttons |= 1 << b;
        }
        input.push(f);
        input
    }

    #[test]
    fn the_gear_lever_toggles_and_stays_down_on_the_wheels() {
        let mut s = slot();
        work_levers(&mut s, &press(&[button::GEAR]));
        assert!(s.gear_lever_down, "down from up");
        s.state.ground.wow = true;
        work_levers(&mut s, &press(&[button::GEAR]));
        assert!(s.gear_lever_down, "up is refused on the wheels");
        s.state.ground.wow = false;
        work_levers(&mut s, &press(&[button::GEAR]));
        assert!(!s.gear_lever_down, "up in the air");
    }

    #[test]
    fn the_gear_lever_is_also_held_by_the_control_systems_weight_on_wheels() {
        let mut s = slot();
        s.gear_lever_down = true;
        s.fcs.on_ground = true;
        work_levers(&mut s, &press(&[button::GEAR]));
        assert!(s.gear_lever_down);
    }

    #[test]
    fn the_flap_lever_steps_cycles_and_stops_at_its_ends() {
        let mut s = slot();
        let detent_after = |s: &mut AircraftSlot, b: usize| {
            work_levers(s, &press(&[b]));
            s.flap_detent
        };
        assert_eq!(detent_after(&mut s, button::FLAPS_DOWN), 1);
        assert_eq!(detent_after(&mut s, button::FLAPS_DOWN), 2);
        assert_eq!(detent_after(&mut s, button::FLAPS_DOWN), 2, "no further");
        assert_eq!(detent_after(&mut s, button::FLAPS_UP), 1);
        assert_eq!(detent_after(&mut s, button::FLAPS_UP), 0);
        assert_eq!(detent_after(&mut s, button::FLAPS_UP), 0, "no further");
        for want in [1, 2, 0, 1] {
            assert_eq!(detent_after(&mut s, button::FLAPS_CYCLE), want);
        }
    }

    #[test]
    fn the_parking_brake_toggles_and_sets_both_wheel_brakes() {
        let mut s = slot();
        work_levers(&mut s, &press(&[button::PARKING_BRAKE]));
        assert!(s.parking_brake);
        let c = controls(&mut s, &InputState::new());
        assert_eq!((c.brake_l, c.brake_r), (1.0, 1.0));
        work_levers(&mut s, &press(&[button::PARKING_BRAKE]));
        assert!(!s.parking_brake);
        let c = controls(&mut s, &InputState::new());
        assert_eq!((c.brake_l, c.brake_r), (0.0, 0.0));
    }

    #[test]
    fn the_trim_reset_zeroes_the_trim() {
        let mut s = slot();
        s.state.trim_pitch_rad = 0.1;
        work_levers(&mut s, &press(&[button::TRIM_RESET]));
        assert_eq!(s.state.trim_pitch_rad, 0.0);
    }

    #[test]
    fn the_throttle_is_held_until_the_host_acknowledges_the_spawn() {
        let mut s = slot();
        s.spawn_seq = 3;
        s.throttle_hold = Some(0.25);
        let mut input = InputState::new();
        let mut f = InputFrame::neutral(1);
        f.axes[axis::THROTTLE] = 1.0;
        f.spawn_ack = 2;
        input.push(f);
        assert_eq!(controls(&mut s, &input).throttle, 0.25, "the old spawn");
        f.seq = 2;
        f.spawn_ack = 3;
        input.push(f);
        assert_eq!(controls(&mut s, &input).throttle, 1.0, "acknowledged");
        assert_eq!(s.throttle_hold, None);
        f.seq = 3;
        f.axes[axis::THROTTLE] = 0.4;
        f.spawn_ack = 0;
        input.push(f);
        assert!(
            (controls(&mut s, &input).throttle - 0.4).abs() < 1e-6,
            "free"
        );
    }

    fn with_effects(delay: usize, scale: f64) -> Box<AircraftSlot> {
        let mut s = slot();
        s.effects = PhysioEffects {
            delay_ticks: delay,
            control_scale: scale,
            ..PhysioEffects::NONE
        };
        s
    }

    fn stick(pitch: f64) -> PilotControls {
        PilotControls {
            pitch,
            roll: -pitch,
            yaw: 0.5 * pitch,
            ..PilotControls::default()
        }
    }

    #[test]
    fn a_pilot_who_is_fine_reaches_the_aircraft_unchanged() {
        let mut s = with_effects(0, 1.0);
        let mut c = stick(0.7);
        filter(&mut s, &mut c);
        assert_eq!((c.pitch, c.roll, c.yaw), (0.7, -0.7, 0.35));
    }

    #[test]
    fn a_blackout_delays_the_stick_by_the_ticks_it_asks_for() {
        let mut s = with_effects(0, 1.0);
        // Twenty ticks of a stick that rises by 0.05 a tick, flying fine.
        for i in 0..20 {
            let mut c = stick(0.05 * f64::from(i));
            filter(&mut s, &mut c);
        }
        s.effects.delay_ticks = 6;
        let mut c = stick(1.0);
        filter(&mut s, &mut c);
        // Six ticks ago the stick was at 0.05 * 14 = 0.7.
        assert!((c.pitch - 0.7).abs() < 1e-12, "{}", c.pitch);
        // The ring holds what the pilot did, so the delay can end without a jump in what it holds.
        s.effects.delay_ticks = 0;
        let mut c = stick(1.0);
        filter(&mut s, &mut c);
        assert_eq!(c.pitch, 1.0);
    }

    #[test]
    fn the_longest_delay_reads_the_oldest_entry() {
        let mut s = with_effects(0, 1.0);
        for i in 0..MAX_DELAY_TICKS {
            let mut c = stick((i as f64 + 1.0) / 100.0);
            filter(&mut s, &mut c);
        }
        s.effects.delay_ticks = MAX_DELAY_TICKS;
        let mut c = stick(0.9);
        filter(&mut s, &mut c);
        assert!((c.pitch - 0.01).abs() < 1e-12, "{}", c.pitch);
        s.effects.delay_ticks = MAX_DELAY_TICKS + 5;
        let mut c = stick(0.9);
        filter(&mut s, &mut c);
        assert!(
            (c.pitch - 0.02).abs() < 1e-12,
            "a longer request is clamped: {}",
            c.pitch
        );
    }

    #[test]
    fn g_loc_zeroes_the_stick_and_the_ramp_gives_it_back_in_part() {
        let mut s = with_effects(0, 0.0);
        let mut c = stick(1.0);
        filter(&mut s, &mut c);
        assert_eq!((c.pitch, c.roll, c.yaw), (0.0, 0.0, 0.0));
        s.effects.control_scale = 0.25;
        let mut c = stick(0.8);
        filter(&mut s, &mut c);
        assert!((c.pitch - 0.2).abs() < 1e-12 && (c.roll + 0.2).abs() < 1e-12);
    }

    #[test]
    fn a_pilot_who_is_out_does_not_work_the_trim_switches() {
        let mut s = with_effects(0, 0.0);
        s.effects.gloc = true;
        let mut c = PilotControls {
            trim_nose_up: true,
            trim_nose_down: true,
            ..stick(1.0)
        };
        filter(&mut s, &mut c);
        assert!(!c.trim_nose_up && !c.trim_nose_down);
        s.effects.gloc = false;
        s.effects.control_scale = 1.0;
        let mut c = PilotControls {
            trim_nose_up: true,
            ..stick(1.0)
        };
        filter(&mut s, &mut c);
        assert!(c.trim_nose_up);
    }

    #[test]
    fn the_aim_comes_from_the_cursor_axes_when_the_flag_is_set() {
        let mut input = InputState::new();
        let mut f = InputFrame::neutral(1);
        f.axes[axis::CURSOR_X] = 0.5;
        f.axes[axis::CURSOR_Y] = -0.5;
        input.push(f);
        assert!(aim_from_frame(&input, false).is_none(), "no flag, no aim");
        f.seq = 2;
        f.flags |= input_flag::MOUSE_AIM as u32;
        input.push(f);
        let a = aim_from_frame(&input, false).expect("aim");
        assert!((a.azimuth_rad - core::f64::consts::FRAC_PI_2).abs() < 1e-6);
        assert!((a.elevation_rad + core::f64::consts::FRAC_PI_4).abs() < 1e-6);
        assert!(aim_from_frame(&input, true).is_none(), "the pilot is out");
    }
}
