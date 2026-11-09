use super::*;
use crate::test_support::{Cockpit, DT, kestrel};
use fcs::Assist;

const MASS_KG: f64 = 10_260.0;

/// The test runway: threshold 50 m behind the cockpit's runway spot, heading north.
fn runway() -> Runway {
    Runway {
        threshold_bf: DVec3::new(0.0, -50.0, 0.0),
        heading_rad: 0.0,
        length_m: 3_000.0,
        width_m: 45.0,
    }
}

fn north() -> DQuat {
    DQuat::from_axis_angle(DVec3::Z, core::f64::consts::FRAC_PI_2)
        * DQuat::from_axis_angle(DVec3::X, core::f64::consts::PI)
}

fn bandit_at(pos: DVec3, alive: bool) -> BanditView {
    BanditView {
        pos_bf: pos,
        vel_bf: DVec3::new(0.0, -200.0, 0.0),
        att: DQuat::from_axis_angle(DVec3::Z, core::f64::consts::PI) * north(),
        alive,
    }
}

fn own(ck: &Cockpit) -> OwnInputs {
    OwnInputs {
        levers: ck.levers,
        rounds: 150,
        missiles: 2,
        ..OwnInputs::default()
    }
}

#[test]
fn the_take_off_hands_over_to_the_climb_at_the_crosswind_turn() {
    let mut ck = Cockpit::runway(Assist::Standard, MASS_KG);
    let mut bot = SortieTester::new(runway());
    // Far enough that the climb is still on when the run ends.
    let bandit = bandit_at(DVec3::new(0.0, 60_000.0, 4_500.0), true);
    let mut handed_over = None;
    for _ in 0..(150.0 / DT) as u32 {
        let before = bot.inner().phase();
        let out = bot.step(&own(&ck), Some(&bandit), &ck.airframe());
        if handed_over.is_none() && bot.phase() == SortiePhase::Climb {
            assert_eq!(before, Phase::Climb, "the tester flew the climb-out");
            assert_eq!(bot.inner().phase(), Phase::Crosswind);
            handed_over = Some((ck.t(), ck.height()));
        }
        ck.step(&out.controls, out.presses);
        assert!(!ck.destroyed);
    }
    let (t, h) = handed_over.expect("it reached the crosswind turn");
    assert!(t > 20.0 && h > 100.0, "{t} s, {h} m");
    assert_eq!(bot.phase(), SortiePhase::Climb);
    // Climbing towards the bandit (due north), the gear up.
    assert!(ck.height() > h + 500.0, "{} m", ck.height());
    assert!(
        ck.track().abs() < 20f64.to_radians(),
        "track {}",
        ck.track().to_degrees()
    );
    assert!(!ck.levers.gear_down);
}

#[test]
fn an_air_start_attacks_a_near_bandit_or_climbs() {
    let ck = Cockpit::air(
        Assist::Standard,
        DVec3::new(0.0, 0.0, 3_000.0),
        0.0,
        200.0,
        MASS_KG,
    );
    for (bandit, phase) in [
        (
            Some(bandit_at(DVec3::new(0.0, 8_000.0, 3_000.0), true)),
            SortiePhase::Attack,
        ),
        (
            Some(bandit_at(DVec3::new(0.0, 20_000.0, 3_000.0), true)),
            SortiePhase::Climb,
        ),
        (
            Some(bandit_at(DVec3::new(0.0, 8_000.0, 3_000.0), false)),
            SortiePhase::Climb,
        ),
        (None, SortiePhase::Climb),
    ] {
        let mut bot = SortieTester::new(runway());
        bot.step(&own(&ck), bandit.as_ref(), &ck.airframe());
        assert_eq!(bot.phase(), phase, "{bandit:?}");
    }
    // A respawn starts it again.
    let mut bot = SortieTester::new(runway());
    let near = bandit_at(DVec3::new(0.0, 8_000.0, 3_000.0), true);
    bot.step(&own(&ck), Some(&near), &ck.airframe());
    bot.step(&own(&ck), None, &ck.airframe());
    assert_eq!(bot.phase(), SortiePhase::Return);
    let respawned = OwnInputs {
        spawn_seq: 1,
        ..own(&ck)
    };
    bot.step(&respawned, Some(&near), &ck.airframe());
    assert_eq!(bot.phase(), SortiePhase::Attack);
}

#[test]
fn after_the_kill_it_flies_home_and_lands_under_every_assist() {
    let (model, _) = kestrel();
    let _ = model;
    for assist in [
        Assist::Full,
        Assist::Standard,
        Assist::Reduced,
        Assist::Manual,
    ] {
        // 8 km north of the lead-in point at 1 500 m, heading south-east, the bandit just killed.
        let mut ck = Cockpit::air(
            assist,
            DVec3::new(-2_000.0, 15_000.0, 1_500.0),
            2.5,
            200.0,
            MASS_KG,
        );
        let mut bot = SortieTester::new(runway());
        let mut phases = Vec::new();
        for k in 0..(500.0 / DT) as u32 {
            let bandit = bandit_at(ck.ac.body.pos_bf + DVec3::new(0.0, 3_000.0, 0.0), k == 0);
            let out = bot.step(&own(&ck), Some(&bandit), &ck.airframe());
            if phases.last() != Some(&bot.phase()) {
                phases.push(bot.phase());
            }
            ck.step(&out.controls, out.presses);
            if bot.phase() == SortiePhase::Done || ck.destroyed {
                break;
            }
        }
        let at = runway().locate(ck.ac.body.pos_bf);
        assert!(!ck.destroyed, "{assist:?}: {phases:?} at {at:?}");
        assert_eq!(
            phases,
            [
                SortiePhase::Attack,
                SortiePhase::Return,
                SortiePhase::Land,
                SortiePhase::Done
            ],
            "{assist:?} at {at:?} after {} s",
            ck.t()
        );
        assert!(
            at.across.abs() <= 22.5 && (0.0..=3_000.0).contains(&at.along),
            "{assist:?}: {at:?}"
        );
        assert!(ck.ac.body.vel_bf.length() < 2.0);
    }
}

#[test]
fn missiles_wait_for_the_cooldown_and_the_gun_fires_in_bursts_on_the_pipper() {
    let (model, fcs) = kestrel();
    let ck = Cockpit::air(
        Assist::Standard,
        DVec3::new(0.0, 0.0, 3_000.0),
        0.0,
        200.0,
        MASS_KG,
    );
    let pos = ck.ac.body.pos_bf;
    let af = |t: f64| Airframe {
        model,
        fcs,
        ac: &ck.ac,
        fs: &ck.fs,
        env: &ck.world,
        t_s: t,
    };
    // SHOOT held for 10 s: a press at once and one 8 s later.
    let mut bot = SortieTester::new(runway());
    let far = bandit_at(pos + DVec3::new(0.0, 3_000.0, 0.0), true);
    let shoot = OwnInputs {
        shoot_missile: true,
        ..own(&ck)
    };
    let presses: Vec<u32> = (0..600)
        .filter(|&k| {
            bot.step(&shoot, Some(&far), &af(f64::from(k) * DT))
                .fire_missile
        })
        .collect();
    assert_eq!(presses, [0, 480]);
    let empty = OwnInputs {
        missiles: 0,
        ..shoot
    };
    let mut bot = SortieTester::new(runway());
    assert!(!bot.step(&empty, Some(&far), &af(0.0)).fire_missile);
    // The bandit 800 m ahead with the pipper on it: bursts of 0.5 s with 0.25 s pauses.
    let close = bandit_at(pos + DVec3::new(0.0, 800.0, 0.0), true);
    let on = OwnInputs {
        pipper_bf: Some(close.pos_bf),
        ..own(&ck)
    };
    let mut bot = SortieTester::new(runway());
    let fired: Vec<bool> = (0..45)
        .map(|k| bot.step(&on, Some(&close), &af(f64::from(k) * DT)).fire_gun)
        .collect();
    assert!(
        fired[0] && fired[29] && !fired[31] && !fired[44],
        "{fired:?}"
    );
    // The pipper 10 m off at 800 m (over 5 m), or no rounds: no fire.
    for o in [
        OwnInputs {
            pipper_bf: Some(close.pos_bf + DVec3::new(10.0, 0.0, 0.0)),
            ..own(&ck)
        },
        OwnInputs { rounds: 0, ..on },
    ] {
        let mut bot = SortieTester::new(runway());
        assert!((0..30).all(|k| !bot.step(&o, Some(&close), &af(f64::from(k) * DT)).fire_gun));
    }
}

#[test]
fn every_field_reaches_the_hash() {
    let b = SortieTester::new(runway());
    let edits: Vec<fn(&mut SortieTester)> = vec![
        |b| b.phase = SortiePhase::Attack,
        |b| b.since_s = 1.0,
        |b| b.missile_cooldown_until_s = 1.0,
        |b| b.burst_until_s = 1.0,
        |b| b.pause_until_s = 1.0,
        |b| b.last_spawn_seq = 3,
        |b| b.started = true,
        |b| b.speed_i = 1.0,
        |b| b.return_leg = 1,
        |b| b.inner.join_downwind(5.0),
    ];
    for (i, f) in edits.into_iter().enumerate() {
        let mut c = b;
        f(&mut c);
        assert_ne!(b.words(), c.words(), "edit {i}");
    }
}
