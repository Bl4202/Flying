//! The player's aircraft through the whole core: the runway spawn, the ground roll, the levers,
//! the assist request and the spawn hand-over, flown with input frames only (ADR-34, ADR-36).

use std::path::Path;
use std::sync::OnceLock;

use sim_core::input::{FRAME_BYTES, InputFrame};
use sim_core::layout::{axis, button, ownship_flag2, snapshot as snap, spawn_request};
use sim_core::{Assist, GEffects, SessionConfig, SimCore, Spawn};

/// The real core pack, baked once.
fn pack() -> &'static [u8] {
    static PACK: OnceLock<Vec<u8>> = OnceLock::new();
    PACK.get_or_init(|| {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content/core");
        content_bake::bake::bake_dir(&dir)
            .expect("the core pack bakes")
            .cpack
    })
}

const AIR_STANDARD: SessionConfig = SessionConfig {
    spawn: Spawn::Air,
    assist: Assist::Standard,
    g_effects: GEffects::Full,
};

/// A core and the host's side of the conversation: sequence numbers, the throttle lever and the
/// spawn acknowledgement the pump of the browser would make.
struct Flight {
    core: SimCore,
    seq: u32,
    lever: f32,
    /// Acknowledge each spawn the way the pump does (set the lever to the snapshot's, then ack).
    acknowledge: bool,
    ack: u8,
}

impl Flight {
    fn new(session: SessionConfig) -> Self {
        let mut core = SimCore::new(sim_core::DEFAULT_SEED);
        core.load_content(pack()).expect("the pack loads");
        core.configure(session).expect("idle");
        assert!(core.start());
        Self {
            core,
            seq: 0,
            lever: 0.0,
            acknowledge: true,
            ack: 0,
        }
    }

    /// Sends one frame (the host's lever and acknowledgement, then `f`) and runs one tick.
    fn tick(&mut self, f: impl FnOnce(&mut InputFrame)) {
        if self.acknowledge && self.spawn_seq() != self.ack {
            // The host sees a new spawn: it moves its lever to the snapshot's, then acknowledges.
            self.lever = self.f32_at(snap::O_THROTTLE);
            self.ack = self.spawn_seq();
        }
        self.seq += 1;
        let mut fr = InputFrame::neutral(self.seq);
        fr.axes[axis::THROTTLE] = self.lever;
        fr.spawn_ack = self.ack;
        f(&mut fr);
        let mut bytes = [0_u8; FRAME_BYTES];
        fr.encode(&mut bytes);
        assert_eq!(self.core.push_inputs(&bytes), 1);
        assert_eq!(self.core.step(1), 1);
    }

    fn run(&mut self, ticks: u32, mut f: impl FnMut(&mut InputFrame)) {
        for _ in 0..ticks {
            self.tick(&mut f);
        }
    }

    fn f32_at(&self, o: usize) -> f32 {
        f32::from_le_bytes(self.core.snapshot_bytes()[o..o + 4].try_into().unwrap())
    }

    fn f64_at(&self, o: usize) -> f64 {
        f64::from_le_bytes(self.core.snapshot_bytes()[o..o + 8].try_into().unwrap())
    }

    fn u8_at(&self, o: usize) -> u8 {
        self.core.snapshot_bytes()[o]
    }

    fn spawn_seq(&self) -> u8 {
        self.u8_at(snap::O_SPAWN_SEQ)
    }

    fn flags2(&self) -> u16 {
        let b = self.core.snapshot_bytes();
        u16::from_le_bytes([b[snap::O_FLAGS2], b[snap::O_FLAGS2 + 1]])
    }

    fn has(&self, flag: usize) -> bool {
        self.flags2() & flag as u16 != 0
    }

    fn pos(&self) -> [f64; 3] {
        [
            self.f64_at(snap::O_POS_BF),
            self.f64_at(snap::O_POS_BF + 8),
            self.f64_at(snap::O_POS_BF + 16),
        ]
    }

    fn ground_speed(&self) -> f64 {
        f64::from(self.f32_at(snap::O_GROUND_SPEED))
    }
}

/// Rotates at `rotate_mps` ground speed with `pull`, and lets go of the stick once climbing.
fn takeoff_stick(f: &Flight, rotate_mps: f64, pull: f32) -> f32 {
    if f.ground_speed() > rotate_mps && f.f32_at(snap::O_VS) < 3.0 {
        pull
    } else {
        0.0
    }
}

#[test]
fn the_runway_spawn_sits_still_on_its_parking_brake() {
    let mut f = Flight::new(SessionConfig::default());
    // Lined up 150 m past the threshold, on the centreline.
    let p0 = f.pos();
    assert!(
        p0[0].abs() < 1e-6 && (p0[1] + 6_000.0 - 150.0).abs() < 0.1,
        "{p0:?}"
    );
    assert_eq!(f.u8_at(snap::O_ASSIST), 2, "Standard");
    assert_eq!(f.u8_at(snap::O_FCS_MODE), 1, "the ground law");
    assert!(f.has(ownship_flag2::WOW) && f.has(ownship_flag2::PARKING_BRAKE));
    assert!(
        f.has(ownship_flag2::THROTTLE_HELD),
        "idle, until the host acknowledges"
    );
    assert_eq!(f.u8_at(snap::O_FLAP_DETENT), 1, "take-off flaps");
    assert_eq!(f.f32_at(snap::O_GEAR_POS), 1.0);
    // Lever down, all down and locked, and the three struts loaded.
    assert_eq!(f.u8_at(snap::O_GEAR_BITS), 0b1100_0000 | 0b01_01_01);

    let mut worst = 0.0_f64;
    for _ in 0..300 {
        f.tick(|_| {});
        worst = worst.max(f.ground_speed());
    }
    assert!(worst < 0.05, "moved at {worst} m/s");
    let p1 = f.pos();
    assert!(
        ((p1[0] - p0[0]) * (p1[0] - p0[0]) + (p1[1] - p0[1]) * (p1[1] - p0[1])).sqrt() < 0.1,
        "drifted from {p0:?} to {p1:?}"
    );
    assert!(f.has(ownship_flag2::WOW));
    assert!(!f.has(ownship_flag2::THROTTLE_HELD), "acknowledged");
    assert!(f.f32_at(snap::O_THRUST) < 8_000.0, "idle thrust");
    let c = f.core.counters();
    assert_eq!((c.touchdowns, c.crashes, c.fdm_faults), (0, 0, 0));
}

#[test]
fn full_power_and_released_brakes_roll_and_a_pull_lifts_off() {
    let mut f = Flight::new(SessionConfig::default());
    f.run(60, |_| {});
    // Parking brake off, full afterburner.
    f.lever = 1.0;
    f.tick(|fr| fr.buttons |= 1 << button::PARKING_BRAKE);
    assert!(!f.has(ownship_flag2::PARKING_BRAKE));
    let start_y = f.pos()[1];
    let mut roll_m = None;
    let mut worst_x = 0.0_f64;
    for _ in 0..(20 * 60) {
        let pull = takeoff_stick(&f, 68.0, 0.3);
        f.tick(|fr| fr.axes[axis::PITCH] = pull);
        worst_x = worst_x.max(f.pos()[0].abs());
        if roll_m.is_none() && !f.has(ownship_flag2::WOW) {
            roll_m = Some(f.pos()[1] - start_y);
        }
        if f.f32_at(snap::O_AGL) > 30.0 {
            break;
        }
    }
    let roll = roll_m.expect("it never left the ground");
    assert!(roll > 150.0 && roll < 900.0, "lifted off after {roll} m");
    assert!(worst_x < 5.0, "strayed {worst_x} m from the centreline");
    assert!(f.f32_at(snap::O_AGL) > 30.0, "climbing");
    assert!(f.f32_at(snap::O_VS) > 5.0);
    assert_eq!(
        f.f32_at(snap::O_GEAR_POS),
        1.0,
        "the gear stays down until the lever moves"
    );
    let c = f.core.counters();
    assert_eq!((c.touchdowns, c.crashes, c.fdm_faults), (0, 0, 0));
    // The ground law hands over to the flight law.
    f.run(240, |_| {});
    assert_eq!(f.u8_at(snap::O_FCS_MODE), 0, "the flight law");
}

#[test]
fn the_toe_brakes_stop_a_rolling_aircraft() {
    let mut f = Flight::new(SessionConfig::default());
    f.run(60, |_| {});
    f.lever = 1.0;
    f.tick(|fr| fr.buttons |= 1 << button::PARKING_BRAKE);
    while f.ground_speed() < 40.0 {
        f.tick(|_| {});
    }
    // Idle, both toe brakes.
    f.lever = 0.0;
    for _ in 0..(30 * 60) {
        f.tick(|fr| {
            fr.axes[axis::BRAKE_L] = 1.0;
            fr.axes[axis::BRAKE_R] = 1.0;
        });
        if f.ground_speed() < 0.2 {
            break;
        }
    }
    assert!(
        f.ground_speed() < 0.2,
        "still rolling at {} m/s",
        f.ground_speed()
    );
    assert!(f.has(ownship_flag2::WOW));
    assert_eq!(f.core.counters().crashes, 0);
    assert_eq!(
        (f.f32_at(snap::O_BRAKE_L), f.f32_at(snap::O_BRAKE_R)),
        (1.0, 1.0)
    );
}

#[test]
fn the_gear_lever_takes_six_seconds_and_is_refused_on_the_wheels() {
    // In the air: up, then the lever down.
    let mut f = Flight::new(AIR_STANDARD);
    assert_eq!(f.f32_at(snap::O_GEAR_POS), 0.0);
    assert_eq!(f.u8_at(snap::O_GEAR_BITS) & 0xC0, 0);
    f.run(10, |_| {});
    f.tick(|fr| fr.buttons |= 1 << button::GEAR);
    assert_ne!(
        f.u8_at(snap::O_GEAR_BITS) & 0x40,
        0,
        "the lever is down at once"
    );
    let mut last = 0.0_f32;
    let mut locked_at = None;
    for i in 1..=(8 * 60) {
        f.tick(|_| {});
        let g = f.f32_at(snap::O_GEAR_POS);
        assert!(g >= last, "monotone");
        last = g;
        if locked_at.is_none() && f.u8_at(snap::O_GEAR_BITS) & 0x80 != 0 {
            locked_at = Some(f64::from(i) / 60.0);
        }
    }
    let t = locked_at.expect("never locked");
    assert!((5.9..6.3).contains(&t), "down and locked after {t} s");
    assert_eq!(f.f32_at(snap::O_GEAR_POS), 1.0);
    // Back up in the air.
    f.tick(|fr| fr.buttons |= 1 << button::GEAR);
    assert_eq!(f.u8_at(snap::O_GEAR_BITS) & 0xC0, 0, "raised");
    f.run(7 * 60, |_| {});
    assert_eq!(f.f32_at(snap::O_GEAR_POS), 0.0);

    // On the wheels the lever cannot be raised.
    let mut g = Flight::new(SessionConfig::default());
    g.run(30, |_| {});
    g.tick(|fr| fr.buttons |= 1 << button::GEAR);
    g.run(60, |_| {});
    assert_eq!(g.f32_at(snap::O_GEAR_POS), 1.0);
    assert_ne!(
        g.u8_at(snap::O_GEAR_BITS) & 0xC0,
        0,
        "still down and locked"
    );
}

#[test]
fn the_flap_lever_moves_the_flaps_to_its_detents() {
    let mut f = Flight::new(SessionConfig::default());
    let tef = |f: &Flight| f.f32_at(snap::O_TEF);
    let to = tef(&f);
    assert!(to > 0.0, "take-off flaps at the spawn");
    f.tick(|fr| fr.buttons |= 1 << button::FLAPS_DOWN);
    assert_eq!(f.u8_at(snap::O_FLAP_DETENT), 2);
    f.run(10 * 60, |_| {});
    assert!(
        (tef(&f) - 2.0 * to).abs() < 0.01,
        "landing flaps are twice the take-off"
    );
    f.tick(|fr| fr.buttons |= 1 << button::FLAPS_CYCLE);
    assert_eq!(
        f.u8_at(snap::O_FLAP_DETENT),
        0,
        "the cycle goes round to up"
    );
    f.run(10 * 60, |_| {});
    assert!(tef(&f).abs() < 1e-3);
}

#[test]
fn the_spawn_throttle_is_held_until_the_host_acknowledges() {
    let mut f = Flight::new(SessionConfig::default());
    f.acknowledge = false;
    // The host's lever is at full, and it says nothing about the spawn.
    f.lever = 1.0;
    f.run(120, |_| {});
    assert!(f.has(ownship_flag2::THROTTLE_HELD));
    assert_eq!(f.f32_at(snap::O_THROTTLE), 0.0, "idle, not the lever");
    assert!(f.f32_at(snap::O_THRUST) < 8_000.0);
    assert!(f.ground_speed() < 0.05);
    // A wrong acknowledgement changes nothing.
    f.ack = f.spawn_seq().wrapping_add(1);
    f.run(10, |_| {});
    assert!(f.has(ownship_flag2::THROTTLE_HELD));
    // The right one hands the lever over.
    f.ack = f.spawn_seq();
    f.run(1, |_| {});
    assert!(!f.has(ownship_flag2::THROTTLE_HELD));
    assert_eq!(f.f32_at(snap::O_THROTTLE), 1.0);
    f.run(60, |_| {});
    assert!(f.f32_at(snap::O_THRUST) > 40_000.0, "afterburner");
}

#[test]
fn an_assist_request_switches_the_law_and_is_hashed() {
    let run = |request: Option<u8>| {
        let mut f = Flight::new(AIR_STANDARD);
        f.run(60, |_| {});
        assert_eq!(f.u8_at(snap::O_ASSIST), 2);
        if let Some(level) = request {
            f.run(60, |fr| fr.assist_req = level);
        } else {
            f.run(60, |_| {});
        }
        f.run(60, |_| {});
        f
    };
    let (a, b, c) = (run(None), run(Some(3)), run(Some(3)));
    assert_eq!(b.u8_at(snap::O_ASSIST), 3, "Reduced");
    assert_eq!(
        b.core.counters().assist_changes,
        1,
        "once, not once per frame"
    );
    assert_eq!(a.core.counters().assist_changes, 0);
    assert_ne!(
        a.core.subsystem_hashes().ownship,
        b.core.subsystem_hashes().ownship
    );
    assert_eq!(
        b.core.subsystem_hashes(),
        c.core.subsystem_hashes(),
        "and repeatable"
    );
    // Every level can be asked for; 0 and anything else keep the level.
    let mut f = Flight::new(AIR_STANDARD);
    for (req, want) in [(1, 1), (0, 1), (4, 4), (9, 4), (2, 2)] {
        f.run(30, |fr| fr.assist_req = req);
        assert_eq!(f.u8_at(snap::O_ASSIST), want, "request {req}");
    }
    assert_eq!(f.core.counters().assist_changes, 3);
}

#[test]
fn the_session_is_part_of_the_aircrafts_hash() {
    let hash = |session: SessionConfig| {
        let mut f = Flight::new(session);
        f.run(30, |_| {});
        f.core.subsystem_hashes().ownship
    };
    let base = hash(AIR_STANDARD);
    for other in [
        SessionConfig {
            g_effects: GEffects::Off,
            ..AIR_STANDARD
        },
        SessionConfig {
            g_effects: GEffects::Reduced,
            ..AIR_STANDARD
        },
        SessionConfig {
            assist: Assist::Full,
            ..AIR_STANDARD
        },
    ] {
        assert_ne!(hash(other), base, "{other:?}");
    }
    assert_eq!(hash(AIR_STANDARD), base);
}

#[test]
fn a_reset_with_a_spawn_request_puts_the_aircraft_where_it_asks() {
    let mut f = Flight::new(SessionConfig::default());
    f.run(60, |_| {});
    assert_eq!(f.spawn_seq(), 1);
    // To the air.
    f.tick(|fr| {
        fr.buttons |= 1 << button::RESET;
        fr.spawn_req = spawn_request::AIR as u8;
    });
    assert_eq!(f.spawn_seq(), 2, "a new spawn");
    assert!(
        f.f32_at(snap::O_AGL) > 1_900.0,
        "airborne: {}",
        f.f32_at(snap::O_AGL)
    );
    assert_eq!(f.f32_at(snap::O_GEAR_POS), 0.0, "gear up");
    assert_eq!(f.u8_at(snap::O_FLAP_DETENT), 0);
    assert!(!f.has(ownship_flag2::PARKING_BRAKE) && !f.has(ownship_flag2::WOW));
    assert!(
        f.has(ownship_flag2::THROTTLE_HELD),
        "held at the trim setting"
    );
    assert_eq!(
        f.u8_at(snap::O_ASSIST),
        2,
        "the assist level is the player's, kept"
    );
    assert!((f.f32_at(snap::O_TAS) - 200.0).abs() < 1.0);
    f.run(120, |_| {});
    assert!(
        !f.has(ownship_flag2::THROTTLE_HELD),
        "the host handed the lever over"
    );
    assert!(f.f32_at(snap::O_AGL) > 1_900.0, "and it flies level");
    // Back to the runway.
    f.tick(|fr| {
        fr.buttons |= 1 << button::RESET;
        fr.spawn_req = spawn_request::RUNWAY as u8;
    });
    assert_eq!(f.spawn_seq(), 3);
    assert!(f.has(ownship_flag2::WOW) && f.has(ownship_flag2::PARKING_BRAKE));
    // A reset without a request follows the session (the runway).
    f.run(5, |_| {});
    f.tick(|fr| fr.buttons |= 1 << button::RESET);
    assert_eq!(f.spawn_seq(), 4);
    assert!(f.has(ownship_flag2::WOW));
    assert_eq!(f.core.counters().respawns, 3);
}

#[test]
fn a_session_that_starts_in_the_air_respawns_in_the_air() {
    let mut f = Flight::new(AIR_STANDARD);
    f.run(30, |_| {});
    f.tick(|fr| fr.buttons |= 1 << button::RESET);
    assert!(f.f32_at(snap::O_AGL) > 1_900.0);
    f.tick(|_| {});
    f.tick(|fr| {
        fr.buttons |= 1 << button::RESET;
        fr.spawn_req = spawn_request::RUNWAY as u8;
    });
    assert!(
        f.has(ownship_flag2::WOW),
        "a request overrides the session once"
    );
}

#[test]
fn a_scripted_pilot_needs_no_hand_over() {
    use sim_core::layout::input_script;
    let mut core = SimCore::new(sim_core::DEFAULT_SEED);
    core.load_content(pack()).unwrap();
    core.configure(AIR_STANDARD).unwrap();
    assert!(core.set_input_script(input_script::FLIGHT60 as u32));
    assert!(core.start());
    core.step(10);
    let b = core.snapshot_bytes();
    let flags2 = u16::from_le_bytes([b[snap::O_FLAGS2], b[snap::O_FLAGS2 + 1]]);
    assert_eq!(
        flags2 & ownship_flag2::THROTTLE_HELD as u16,
        0,
        "the script acknowledged"
    );
}

#[test]
fn hands_off_the_aircraft_holds_its_trim_under_every_law() {
    for assist in [
        Assist::Full,
        Assist::Standard,
        Assist::Reduced,
        Assist::Manual,
    ] {
        let mut f = Flight::new(SessionConfig {
            assist,
            ..AIR_STANDARD
        });
        let h0 = f.f64_at(snap::O_POS_BF + 16);
        f.run(10 * 60, |_| {});
        let dh = f.f64_at(snap::O_POS_BF + 16) - h0;
        let vs = f.f32_at(snap::O_VS);
        assert!(dh.abs() < 15.0, "{assist:?}: height moved {dh} m");
        assert!(vs.abs() < 2.0, "{assist:?}: vertical speed {vs} m/s");
        assert_eq!(f.flags2() & 0x1C, 0, "{assist:?}: no limiter active");
        assert_eq!(f.core.counters().fdm_faults, 0);
        assert!((f.f32_at(snap::O_NZ) - 1.0).abs() < 0.1, "{assist:?}");
    }
}

#[test]
fn mouse_aim_turns_the_nose_towards_the_cursor() {
    let mut f = Flight::new(AIR_STANDARD);
    f.run(30, |_| {});
    // The cursor 60 degrees to the right of north, level.
    f.run(6 * 60, |fr| {
        fr.flags |= sim_core::layout::input_flag::MOUSE_AIM as u32;
        fr.axes[axis::CURSOR_X] = 1.0 / 3.0;
        fr.axes[axis::CURSOR_Y] = 0.0;
    });
    assert!(f.has(ownship_flag2::MOUSE_AIM));
    let az = f64::from(f.f32_at(snap::O_AIM_AZ));
    assert!(
        (az - core::f64::consts::FRAC_PI_3).abs() < 1e-4,
        "the echo: {az}"
    );
    // The velocity vector (ENU: x east, y north) is now close to the cursor's bearing.
    let b = f.core.snapshot_bytes();
    let vel = |o: usize| f64::from(f32::from_le_bytes(b[o..o + 4].try_into().unwrap()));
    let bearing = sim_core::dmath::atan2(vel(snap::O_VEL_BF), vel(snap::O_VEL_BF + 4)).to_degrees();
    assert!((bearing - 60.0).abs() < 8.0, "heading {bearing} degrees");
    // Let go of the mouse: the flag and the echo go.
    f.run(2, |_| {});
    assert!(!f.has(ownship_flag2::MOUSE_AIM));
    assert_eq!(f.f32_at(snap::O_AIM_AZ), 0.0);
}

#[test]
fn a_sharp_turn_on_the_wheels_is_not_a_departure() {
    let mut f = Flight::new(SessionConfig::default());
    f.run(60, |_| {});
    f.lever = 0.9;
    f.tick(|fr| fr.buttons |= 1 << button::PARKING_BRAKE);
    // Pedals over with the toe brakes on: the nose wheel steers hard and the aircraft yaws fast.
    let mut fastest = 0.0_f64;
    for _ in 0..(5 * 60) {
        f.tick(|fr| {
            fr.axes[axis::YAW] = 0.6;
            fr.axes[axis::BRAKE_L] = 0.4;
            fr.axes[axis::BRAKE_R] = 0.4;
        });
        let r = f64::from(f.f32_at(snap::O_OMEGA + 8)).abs();
        fastest = fastest.max(r);
        assert!(
            !f.has(ownship_flag2::DEPARTED),
            "flagged as departed at {r} rad/s"
        );
    }
    assert!(fastest > 0.5, "the yaw rate reached only {fastest} rad/s");
    assert!(f.f32_at(snap::O_STEER).abs() > 0.1, "steering");
}
