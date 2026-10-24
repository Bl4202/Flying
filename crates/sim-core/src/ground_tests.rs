//! What the ground and the pilot's body do to the aircraft, with the aircraft put in states no
//! input frame can reach (above the runway at a chosen sink rate, a pilot at the edge of G-LOC).
//! The flights from the input side are in `tests/aircraft.rs`.

use flight::gear::GroundState;

use crate::input::{FRAME_BYTES, InputFrame};
use crate::layout::{axis, crash_cause, fcs_mode, ownship_flag, ownship_flag2, snapshot as snap};
use crate::math::DVec3;
use crate::session::{Assist, GEffects, SessionConfig, Spawn};
use crate::sim::SimCore;
use crate::test_support::{flying_under, slot};

const RUNWAY: SessionConfig = SessionConfig {
    spawn: Spawn::Runway,
    assist: Assist::Standard,
    g_effects: GEffects::Full,
};

const AIR: SessionConfig = SessionConfig {
    spawn: Spawn::Air,
    assist: Assist::Standard,
    g_effects: GEffects::Full,
};

/// Sends neutral frames (with `f` applied) for `ticks` ticks, acknowledging the spawn.
fn fly(c: &mut SimCore, ticks: u32, mut f: impl FnMut(&mut InputFrame)) {
    for _ in 0..ticks {
        let seq = c.input().last_seq() + 1;
        let mut fr = InputFrame::neutral(seq);
        fr.axes[axis::THROTTLE] = c.input().throttle_cmd() as f32;
        fr.spawn_ack = slot(c).spawn_seq;
        f(&mut fr);
        let mut b = [0_u8; FRAME_BYTES];
        fr.encode(&mut b);
        assert_eq!(c.push_inputs(&b), 1);
        c.step(1);
    }
}

fn f32_at(c: &SimCore, o: usize) -> f32 {
    f32::from_le_bytes(c.snapshot_bytes()[o..o + 4].try_into().unwrap())
}

fn flags2(c: &SimCore) -> u16 {
    let b = c.snapshot_bytes();
    u16::from_le_bytes([b[snap::O_FLAGS2], b[snap::O_FLAGS2 + 1]])
}

fn has(c: &SimCore, flag: usize) -> bool {
    flags2(c) & flag as u16 != 0
}

/// A runway core with the aircraft lifted `height_m` above its resting place, falling at
/// `sink_mps` with the gear down, as if it had flown the approach.
fn about_to_land(sink_mps: f64, height_m: f64) -> SimCore {
    let mut c = flying_under(1, RUNWAY);
    fly(&mut c, 5, |_| {});
    let s = c.slot_mut();
    s.state.body.pos_bf += DVec3::new(0.0, 0.0, height_m);
    s.state.body.vel_bf = DVec3::new(0.0, 0.0, -sink_mps);
    s.state.ground = GroundState::AIRBORNE;
    s.parking_brake = false;
    s.fcs = fcs::FcsState::new(fcs::Assist::Standard, false);
    c
}

#[test]
fn a_gentle_touchdown_is_counted_and_is_not_hard() {
    let mut c = about_to_land(1.4, 0.17);
    fly(&mut c, 120, |_| {});
    let n = c.counters();
    assert_eq!((n.touchdowns, n.hard_landings, n.crashes), (1, 0, 0));
    assert!(!slot(&c).crashed);
    let vs = f32_at(&c, snap::O_TOUCHDOWN_VS);
    assert!((1.4..2.0).contains(&vs), "touched down at {vs} m/s");
    assert!(!has(&c, ownship_flag2::HARD_LANDING));
    assert!(has(&c, ownship_flag2::WOW));
}

#[test]
fn a_hard_landing_is_counted_and_flagged_for_three_seconds() {
    let mut c = about_to_land(3.6, 0.17);
    fly(&mut c, 30, |_| {});
    let n = c.counters();
    assert_eq!((n.touchdowns, n.hard_landings, n.crashes), (1, 1, 0));
    assert!(!slot(&c).crashed, "hard, not destroyed");
    assert!(has(&c, ownship_flag2::HARD_LANDING));
    let vs = f32_at(&c, snap::O_TOUCHDOWN_VS);
    assert!((3.4..4.1).contains(&vs), "touched down at {vs} m/s");
    fly(&mut c, 150, |_| {});
    assert!(
        has(&c, ownship_flag2::HARD_LANDING),
        "still latched at ~3 s"
    );
    fly(&mut c, 40, |_| {});
    assert!(!has(&c, ownship_flag2::HARD_LANDING), "released after 3 s");
    assert_eq!(c.counters().hard_landings, 1, "counted once");
}

#[test]
fn a_sink_rate_past_seven_destroys_the_aircraft_and_it_respawns_on_the_runway() {
    let mut c = about_to_land(8.5, 0.17);
    fly(&mut c, 20, |_| {});
    assert!(slot(&c).crashed);
    assert_eq!(c.counters().crashes, 1);
    assert_eq!(
        c.snapshot_bytes()[snap::O_CRASH_CAUSE],
        crash_cause::SINK_RATE as u8
    );
    assert_ne!(
        u32::from(c.snapshot_bytes()[snap::O_FLAGS]) & ownship_flag::CRASHED as u32,
        0
    );
    let wreck = slot(&c).state;
    let seq = slot(&c).spawn_seq;
    fly(&mut c, 90, |_| {});
    assert_eq!(slot(&c).state, wreck, "frozen where it hit");
    fly(&mut c, 20, |_| {});
    assert!(!slot(&c).crashed, "back after two seconds");
    assert_eq!(slot(&c).spawn_seq, seq.wrapping_add(1));
    assert_eq!(
        c.snapshot_bytes()[snap::O_CRASH_CAUSE],
        crash_cause::NONE as u8
    );
    assert!(
        has(&c, ownship_flag2::WOW) && has(&c, ownship_flag2::PARKING_BRAKE),
        "on the runway again"
    );
    assert_eq!(c.counters().respawns, 1);
}

/// A runway core with the gear up and the aircraft `height_m` above the runway going `speed_mps`
/// along it, sinking gently: it will land on its belly.
fn belly_landing(speed_mps: f64) -> SimCore {
    let mut c = flying_under(1, RUNWAY);
    fly(&mut c, 5, |_| {});
    let s = c.slot_mut();
    s.state.body.pos_bf += DVec3::new(0.0, 0.0, 1.2);
    s.state.body.vel_bf = DVec3::new(0.0, speed_mps, -0.5);
    s.state.ground = GroundState::AIRBORNE;
    s.state.surfaces.gear = 0.0;
    s.gear_lever_down = false;
    s.parking_brake = false;
    s.fcs = fcs::FcsState::new(fcs::Assist::Standard, false);
    c
}

#[test]
fn the_airframe_hitting_the_ground_at_speed_destroys_the_aircraft() {
    let mut c = belly_landing(70.0);
    fly(&mut c, 120, |_| {});
    assert!(slot(&c).crashed, "a belly landing at 70 m/s");
    assert_eq!(
        c.snapshot_bytes()[snap::O_CRASH_CAUSE],
        crash_cause::AIRFRAME_IMPACT as u8
    );
    assert_eq!(c.counters().crashes, 1);
}

#[test]
fn a_slow_belly_landing_is_a_skid_the_aircraft_survives() {
    let mut c = belly_landing(20.0);
    // Idle, so only the ground is slowing it.
    fly(&mut c, 600, |f| f.axes[axis::THROTTLE] = 0.0);
    assert!(!slot(&c).crashed, "skidded to a stop");
    assert_eq!(c.counters().crashes, 0);
    assert!(f32_at(&c, snap::O_GROUND_SPEED) < 1.0, "stopped");
}

#[test]
fn a_numerical_fault_says_so_and_the_aircraft_respawns() {
    let mut c = flying_under(2, AIR);
    fly(&mut c, 10, |_| {});
    c.slot_mut().state.numerical_fault = true;
    fly(&mut c, 2, |_| {});
    assert_eq!(
        c.snapshot_bytes()[snap::O_CRASH_CAUSE],
        crash_cause::NUMERICAL as u8
    );
    assert_ne!(
        u32::from(c.snapshot_bytes()[snap::O_FLAGS]) & ownship_flag::NUMERICAL_FAULT as u32,
        0
    );
    fly(&mut c, 130, |_| {});
    assert!(!slot(&c).state.numerical_fault);
    assert_eq!(
        c.snapshot_bytes()[snap::O_CRASH_CAUSE],
        crash_cause::NONE as u8
    );
    assert_eq!(c.counters().respawns, 1);
    assert_eq!(c.counters().crashes, 0, "a fault is not a crash");
}

/// An air core whose pilot is one tick from G-LOC.
fn about_to_black_out(g: GEffects) -> SimCore {
    let mut c = flying_under(
        3,
        SessionConfig {
            g_effects: g,
            ..AIR
        },
    );
    fly(&mut c, 30, |_| {});
    let p = &mut c.slot_mut().physio;
    p.stress = 1.2;
    p.g_f = 9.0;
    c
}

/// The roll rate the aircraft has, rad/s, from the snapshot's body rates.
fn roll_rate(c: &SimCore) -> f64 {
    f64::from(f32_at(c, snap::O_OMEGA)).abs()
}

#[test]
fn g_loc_zeroes_the_controls_for_six_seconds_and_gives_them_back_over_four() {
    let mut c = about_to_black_out(GEffects::Full);
    // Full roll stick all the time; the largest roll rate in each second.
    let mut out_cold = 0.0_f64;
    for second in 0..12 {
        fly(&mut c, 60, |f| f.axes[axis::ROLL] = 1.0);
        if second == 0 {
            assert!(has(&c, ownship_flag2::GLOC));
            assert_eq!(c.snapshot_bytes()[snap::O_FCS_MODE], fcs_mode::GLOC as u8);
        }
        if second < 5 {
            out_cold = out_cold.max(roll_rate(&c));
        }
    }
    assert_eq!(c.counters().gloc_events, 1, "one event, not one per tick");
    // The first five seconds are inside the six of nothing: the aircraft does not roll.
    assert!(
        out_cold < 0.1,
        "rolled at {out_cold} rad/s while the pilot was out"
    );
    // By the end the pilot is back, with the stick.
    assert!(
        roll_rate(&c) > 0.5,
        "roll rate {} after the ramp",
        roll_rate(&c)
    );
    assert!(!has(&c, ownship_flag2::GLOC));
}

#[test]
fn a_pilot_who_is_out_cannot_move_the_gear_lever_or_press_recover() {
    let mut c = about_to_black_out(GEffects::Full);
    fly(&mut c, 5, |_| {});
    assert!(has(&c, ownship_flag2::GLOC));
    let lever = slot(&c).gear_lever_down;
    fly(&mut c, 3, |f| f.buttons |= 1 << crate::layout::button::GEAR);
    fly(&mut c, 3, |_| {});
    fly(&mut c, 3, |f| f.buttons |= 1 << crate::layout::button::GEAR);
    assert_eq!(slot(&c).gear_lever_down, lever, "the lever did not move");
    // The assist level is a menu choice, so it still works.
    fly(&mut c, 3, |f| f.assist_req = 3);
    assert_eq!(c.snapshot_bytes()[snap::O_ASSIST], 3);
    // And the pilot is back after the ten seconds.
    fly(&mut c, 11 * 60, |_| {});
    fly(&mut c, 3, |f| f.buttons |= 1 << crate::layout::button::GEAR);
    assert_ne!(slot(&c).gear_lever_down, lever, "the lever moves again");
}

#[test]
fn with_the_dial_off_the_pilot_state_advances_and_nothing_shows() {
    let mut c = about_to_black_out(GEffects::Off);
    fly(&mut c, 60, |f| f.axes[axis::ROLL] = 1.0);
    assert_eq!(c.counters().gloc_events, 0);
    assert!(!has(&c, ownship_flag2::GLOC) && !has(&c, ownship_flag2::BLACKOUT));
    assert_eq!(
        f32_at(&c, snap::O_G_STRESS),
        0.0,
        "the picture shows nothing"
    );
    assert!(slot(&c).physio.stress > 0.0, "but the state is there");
    assert!(roll_rate(&c) > 0.5, "and the stick works");
}

#[test]
fn with_the_dial_reduced_the_picture_goes_but_the_controls_stay() {
    let mut c = about_to_black_out(GEffects::Reduced);
    fly(&mut c, 60, |f| f.axes[axis::ROLL] = 1.0);
    assert_eq!(c.counters().gloc_events, 0);
    assert!(has(&c, ownship_flag2::BLACKOUT));
    assert!(f32_at(&c, snap::O_G_STRESS) > 0.5);
    assert!(roll_rate(&c) > 0.5, "the stick works");
}

#[test]
fn the_gear_warning_shows_low_slow_and_gear_up_and_goes_with_the_gear_lever() {
    let mut c = flying_under(4, AIR);
    fly(&mut c, 5, |_| {});
    {
        let s = c.slot_mut();
        let up = DVec3::Z;
        s.state.body.pos_bf = DVec3::new(0.0, -3_000.0, 40.0) + up * 100.0;
        s.state.body.vel_bf = DVec3::new(0.0, 90.0, 0.0);
    }
    fly(&mut c, 2, |f| f.axes[axis::THROTTLE] = 0.2);
    // The throttle is held at the trim setting until acknowledged; acknowledged by `fly`.
    fly(&mut c, 2, |f| f.axes[axis::THROTTLE] = 0.2);
    assert!(has(&c, ownship_flag2::GEAR_WARNING));
    fly(&mut c, 2, |f| {
        f.axes[axis::THROTTLE] = 0.2;
        f.buttons |= 1 << crate::layout::button::GEAR;
    });
    assert!(!has(&c, ownship_flag2::GEAR_WARNING), "the lever is down");
    // Power up and the warning is off too.
    let mut d = flying_under(4, AIR);
    fly(&mut d, 5, |_| {});
    fly(&mut d, 5, |f| f.axes[axis::THROTTLE] = 0.9);
    assert!(!has(&d, ownship_flag2::GEAR_WARNING), "high");
}
