use super::*;
use crate::test_support::{Cockpit, DT};
use fcs::Assist;
use fcs::tester::Presses;

const NORTH: DVec3 = DVec3::new(0.0, 200.0, 0.0);
const UP: DVec3 = DVec3::Z;
const GRAVITY: DVec3 = DVec3::new(0.0, 0.0, -G0);

#[test]
fn a_goal_to_the_right_banks_right_and_pulls() {
    // Northbound at 200 m/s, the goal 30° to the right, cap 5 g. The turn rate wanted, 1.5 · 0.52,
    // is beyond the cap's 5 g0 / 200 = 0.245 rad/s, so the turn is at the cap: 49 m/s² to the right
    // and g up gives n = |(49, 0, 9.8)| / g0 = 5.1, held to 5, and a bank of atan(49 / 9.8) = 78.7°.
    let goal = Goal {
        dir_bf: DVec3::new(0.5, 0.866_025_403_784_438_6, 0.0),
        n_cap: 5.0,
        throttle: 0.8,
    };
    let c = goal_command(&goal, NORTH, UP, GRAVITY, UP).expect("a command");
    assert_eq!(c.n, 5.0);
    assert!(
        (c.bank - dmath::atan2(5.0 * G0, G0)).abs() < 1e-12,
        "{}",
        c.bank
    );
    // A gentle offset: 2° right with a 9 g cap. ω = 1.5 · 0.0349 = 0.0524 rad/s, a = 10.5 m/s².
    let small = Goal {
        dir_bf: DVec3::new(dmath::sin(0.034_906_585), dmath::cos(0.034_906_585), 0.0),
        n_cap: 9.0,
        throttle: 0.8,
    };
    let c = goal_command(&small, NORTH, UP, GRAVITY, UP).expect("a command");
    let a = 200.0 * K_TURN * 0.034_906_585;
    assert!((c.n - dmath::hypot(a, G0) / G0).abs() < 1e-6, "{}", c.n);
    assert!((c.bank - dmath::atan2(a, G0)).abs() < 1e-6 && c.bank > 0.0);
    // To the left: a negative bank.
    let left = Goal {
        dir_bf: DVec3::new(-0.5, 0.866, 0.0),
        ..goal
    };
    assert!(
        goal_command(&left, NORTH, UP, GRAVITY, UP)
            .expect("a command")
            .bank
            < 0.0
    );
}

#[test]
fn straight_ahead_holds_one_g_and_a_dive_rolls_over() {
    let ahead = Goal {
        dir_bf: NORTH,
        n_cap: 7.0,
        throttle: 0.8,
    };
    let c = goal_command(&ahead, NORTH, UP, GRAVITY, UP).expect("a command");
    assert!((c.n - 1.0).abs() < 1e-12 && c.bank.abs() < 1e-12, "{c:?}");
    // 30° down at 200 m/s with 5 g: 49 m/s² down beats gravity, so the lift points down (inverted).
    let dive = Goal {
        dir_bf: DVec3::new(0.0, 0.866, -0.5),
        n_cap: 5.0,
        throttle: 0.8,
    };
    let c = goal_command(&dive, NORTH, UP, GRAVITY, UP).expect("a command");
    assert!(
        (c.n - 4.0).abs() < 1e-9 && (c.bank.abs() - core::f64::consts::PI).abs() < 1e-9,
        "{c:?}"
    );
    // Straight behind: the lift direction picks the way (here, rolled right 90°: a right turn).
    let behind = Goal {
        dir_bf: -NORTH,
        n_cap: 5.0,
        throttle: 0.8,
    };
    let c = goal_command(&behind, NORTH, UP, GRAVITY, DVec3::X).expect("a command");
    assert!(c.bank > 1.0 && (c.n - 5.0).abs() < 1e-12, "{c:?}");
}

#[test]
fn unusable_goals_give_neutral_sticks() {
    for dir in [
        DVec3::ZERO,
        DVec3::new(f64::NAN, 0.0, 0.0),
        DVec3::new(f64::INFINITY, 1.0, 0.0),
    ] {
        let goal = Goal {
            dir_bf: dir,
            n_cap: 5.0,
            throttle: 0.8,
        };
        assert_eq!(goal_command(&goal, NORTH, UP, GRAVITY, UP), None);
        let cockpit = Cockpit::air(
            Assist::Standard,
            DVec3::new(0.0, 0.0, 4_000.0),
            0.0,
            200.0,
            10_260.0,
        );
        let c = fly_goal(&goal, &cockpit.airframe());
        assert_eq!((c.pitch, c.roll, c.yaw, c.throttle), (0.0, 0.0, 0.0, 0.8));
    }
    let nan_cap = Goal {
        dir_bf: NORTH,
        n_cap: f64::NAN,
        throttle: f64::NAN,
    };
    assert_eq!(goal_command(&nan_cap, NORTH, UP, GRAVITY, UP), None);
    let cockpit = Cockpit::air(
        Assist::Manual,
        DVec3::new(0.0, 0.0, 4_000.0),
        0.0,
        200.0,
        10_260.0,
    );
    let c = fly_goal(&nan_cap, &cockpit.airframe());
    assert!(c.pitch == 0.0 && c.throttle == MILITARY && c.is_finite_controls());
}

trait Finite {
    fn is_finite_controls(&self) -> bool;
}

impl Finite for PilotControls {
    fn is_finite_controls(&self) -> bool {
        [self.pitch, self.roll, self.yaw, self.throttle]
            .iter()
            .all(|x| x.is_finite())
    }
}

#[test]
fn the_controller_turns_the_kestrel_onto_a_goal_under_every_assist() {
    // Northbound at 4 000 m and 220 m/s; the goal is due east at the same height. After 30 s the
    // track is within 5° of it and the height within 300 m.
    for assist in [
        Assist::Full,
        Assist::Standard,
        Assist::Reduced,
        Assist::Manual,
    ] {
        let mut ck = Cockpit::air(assist, DVec3::new(0.0, 0.0, 4_000.0), 0.0, 220.0, 10_260.0);
        let mut speed_i = 0.0;
        for _ in 0..(30.0 / DT) as u32 {
            let af = ck.airframe();
            let s = af.sensed();
            let up = af.up();
            let gamma = height_gamma(4_000.0, ck.height(), 2_000.0, 0.15);
            let dir = climb_dir(heading_dir(core::f64::consts::FRAC_PI_2, up), up, gamma);
            let throttle = hold_speed(220.0, s.cas, &mut speed_i, DT, 1.0);
            let c = fly_goal(
                &Goal {
                    dir_bf: dir,
                    n_cap: 5.0,
                    throttle,
                },
                &af,
            );
            assert!(c.is_finite_controls());
            ck.step(&c, Presses::default());
        }
        let track = ck.track();
        assert!(!ck.destroyed, "{assist:?}");
        assert!(
            (track - core::f64::consts::FRAC_PI_2).abs() < 5f64.to_radians(),
            "{assist:?}: track {}",
            track.to_degrees()
        );
        assert!(
            (ck.height() - 4_000.0).abs() < 300.0,
            "{assist:?}: {} m",
            ck.height()
        );
    }
}

#[test]
fn the_speed_hold_worked_example() {
    // 10 m/s slow: 0.5 + 0.03 · 10 = 0.8, inside the stops, so the integral grows by 10 · dt.
    let mut i = 0.0;
    assert!((hold_speed(200.0, 190.0, &mut i, DT, 1.0) - (0.8 + 0.004 * 10.0 * DT)).abs() < 1e-12);
    assert!((i - 10.0 * DT).abs() < 1e-15);
    // 50 m/s slow against a 0.8 stop: held at 0.8, and the integral does not wind up.
    let mut i = 0.0;
    assert_eq!(hold_speed(250.0, 200.0, &mut i, DT, 0.8), 0.8);
    assert_eq!(i, 0.0);
    // NaN: the feed-forward, the integral untouched.
    let mut i = 3.0;
    assert_eq!(hold_speed(200.0, f64::NAN, &mut i, DT, 1.0), SPEED_FF);
    assert_eq!(i, 3.0);
    // Height and direction helpers.
    assert_eq!(height_gamma(4_500.0, 4_000.0, 2_000.0, 0.15), 0.15);
    assert!((height_gamma(4_100.0, 4_000.0, 2_000.0, 0.15) - 0.05).abs() < 1e-15);
    assert_eq!(height_gamma(f64::NAN, 4_000.0, 2_000.0, 0.15), 0.0);
    assert!((heading_dir(core::f64::consts::FRAC_PI_2, UP) - DVec3::X).length() < 1e-15);
    let d = climb_dir(DVec3::Y, UP, 0.1);
    assert!((d - DVec3::new(0.0, dmath::cos(0.1), dmath::sin(0.1))).length() < 1e-15);
}
