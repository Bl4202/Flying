use super::*;
use crate::percept::{Dice, OwnView};
use crate::test_support::{Cockpit, DT, skill_def};
use fcs::Assist;
use fcs::tester::Presses;
use simmath::math::DQuat;

const N_MAX: f64 = 9.0;
/// The Kestrel's `V_mo`, m/s.
const VMO_MPS: f64 = 360.0;

fn skill(rank: u8) -> SkillParams {
    SkillParams::from_def(skill_def(rank), N_MAX)
}

/// Level, nose north.
fn north() -> DQuat {
    DQuat::from_axis_angle(DVec3::Z, core::f64::consts::FRAC_PI_2)
        * DQuat::from_axis_angle(DVec3::X, core::f64::consts::PI)
}

/// The bandit on its patrol, 15 km north of home at 4 500 m, northbound at 200 m/s; nothing seen.
fn base(t: f64) -> Percept {
    Percept {
        t_s: t,
        own: OwnView {
            pos_bf: DVec3::new(5_000.0, 15_000.0, 4_500.0),
            vel_bf: DVec3::new(0.0, 200.0, 0.0),
            att: north(),
            cas_mps: 170.0,
            tas_mps: 200.0,
            agl_m: 4_400.0,
            alt_msl_m: 4_500.0,
            up_bf: DVec3::Z,
            n_max: N_MAX,
            vmo_mps: VMO_MPS,
        },
        contact: None,
        threat_ata_rad: None,
        missile: None,
        gun_aim_point_bf: None,
        clearance_min_m: 4_400.0,
        home_bf: DVec3::ZERO,
        arena_radius_m: 40_000.0,
        from_home_m: 15_800.0,
        cap_centre_bf: DVec3::new(0.0, 15_000.0, 0.0),
        cap_radius_m: 5_000.0,
        cap_alt_msl_m: 4_500.0,
        dice: Dice::default(),
    }
}

/// A contact `dy` m north (negative: behind) of the bandit, `dx` east, flying north at 200 m/s.
fn contact(dx: f64, dy: f64, visual: bool) -> Contact {
    Contact {
        pos_bf: DVec3::new(5_000.0 + dx, 15_000.0 + dy, 4_500.0),
        vel_bf: DVec3::new(0.0, 200.0, 0.0),
        visual,
        age_s: 0.0,
    }
}

/// Decides at `t` with `p` and returns the state.
fn decided(b: &mut BanditBrain, p: &Percept, s: &SkillParams) -> BanditState {
    b.decide(p, s);
    b.state
}

#[test]
fn rules_10_9_8_patrol_intercept_pursue() {
    let s = skill(2);
    let mut b = BanditBrain::new(BrainKind::Normal, 1.0);
    assert_eq!(decided(&mut b, &base(0.0), &s), BanditState::Patrol);
    // Told about a contact 20 km away: intercept.
    let mut p = base(10.0);
    p.contact = Some(contact(0.0, 20_000.0, false));
    assert_eq!(decided(&mut b, &p, &s), BanditState::Intercept);
    // Told it is 7 km away: pursue, once the commitment (2.5 s) has run out.
    p.contact = Some(contact(0.0, 7_000.0, false));
    p.t_s = 11.0;
    assert_eq!(decided(&mut b, &p, &s), BanditState::Intercept, "committed");
    p.t_s = 12.6;
    assert_eq!(decided(&mut b, &p, &s), BanditState::Pursue);
    // Seen at 9 km: still pursue (seen beats the 8 km GCI limit).
    let mut q = base(20.0);
    q.contact = Some(contact(0.0, 9_000.0, true));
    assert_eq!(decided(&mut b, &q, &s), BanditState::Pursue);
    // Gone: back to the patrol.
    assert_eq!(decided(&mut b, &base(30.0), &s), BanditState::Patrol);
}

#[test]
fn rule_7_gun_when_close_and_the_aim_point_is_near_the_nose() {
    let s = skill(2);
    let mut b = BanditBrain::new(BrainKind::Normal, 1.0);
    let mut p = base(0.0);
    p.contact = Some(contact(0.0, 800.0, true));
    // The aim point 10 mrad off the nose at 800 m (the window there is 30 mrad).
    p.gun_aim_point_bf = Some(p.own.pos_bf + DVec3::new(8.0, 800.0, 0.0));
    assert_eq!(decided(&mut b, &p, &s), BanditState::Gun);
    // 40 mrad off: pursue.
    let mut b = BanditBrain::new(BrainKind::Normal, 1.0);
    p.gun_aim_point_bf = Some(p.own.pos_bf + DVec3::new(32.0, 800.0, 0.0));
    assert_eq!(decided(&mut b, &p, &s), BanditState::Pursue);
    // Not seen (GCI): no gun.
    let mut b = BanditBrain::new(BrainKind::Normal, 1.0);
    p.gun_aim_point_bf = Some(p.own.pos_bf + DVec3::new(0.0, 800.0, 0.0));
    p.contact = Some(contact(0.0, 800.0, false));
    assert_eq!(decided(&mut b, &p, &s), BanditState::Pursue);
}

#[test]
fn rule_6_only_the_veteran_extends() {
    let mut p = base(0.0);
    p.own.cas_mps = 130.0;
    p.contact = Some(contact(0.0, 5_000.0, true));
    let mut vet = BanditBrain::new(BrainKind::Normal, 1.0);
    assert_eq!(decided(&mut vet, &p, &skill(3)), BanditState::Extend);
    // It keeps extending until 220 m/s, not just past 140.
    p.own.cas_mps = 180.0;
    p.t_s = 5.0;
    assert_eq!(decided(&mut vet, &p, &skill(3)), BanditState::Extend);
    p.own.cas_mps = 221.0;
    assert_eq!(decided(&mut vet, &p, &skill(3)), BanditState::Pursue);
    let mut rec = BanditBrain::new(BrainKind::Normal, 1.0);
    p.own.cas_mps = 130.0;
    assert_eq!(decided(&mut rec, &p, &skill(1)), BanditState::Pursue);
}

#[test]
fn rule_5_defend_the_gun_after_the_reaction_delay() {
    let s = skill(2); // 2.0 s
    let mut b = BanditBrain::new(BrainKind::Normal, 1.0);
    let mut p = base(0.0);
    p.contact = Some(contact(0.0, -1_000.0, true));
    p.threat_ata_rad = Some(0.05);
    assert_eq!(decided(&mut b, &p, &s), BanditState::Pursue, "not yet");
    p.t_s = 1.9;
    assert_eq!(decided(&mut b, &p, &s), BanditState::Pursue);
    p.t_s = 2.0;
    assert_eq!(
        decided(&mut b, &p, &s),
        BanditState::DefendGun,
        "urgent: no commitment wait"
    );
    // A player whose nose is elsewhere, or who is ahead, or far, is no gun threat.
    for (dy, ata, range_ok) in [
        (-1_000.0, 0.7, true),
        (1_000.0, 0.05, true),
        (-3_000.0, 0.05, false),
    ] {
        let mut b = BanditBrain::new(BrainKind::Normal, 1.0);
        let mut q = base(0.0);
        q.contact = Some(contact(0.0, dy, true));
        q.threat_ata_rad = Some(ata);
        b.decide(&q, &s);
        q.t_s = 5.0;
        assert_ne!(
            decided(&mut b, &q, &s),
            BanditState::DefendGun,
            "{dy} {ata} {range_ok}"
        );
    }
}

#[test]
fn rule_4_defend_the_missile_and_time_the_break() {
    let s = skill(3); // 1.2 s, sigma 0.6 s
    let mut b = BanditBrain::new(BrainKind::Normal, 1.0);
    let mut p = base(0.0);
    p.missile = Some(Contact {
        pos_bf: p.own.pos_bf - DVec3::new(0.0, 3_000.0, 0.0),
        vel_bf: DVec3::new(0.0, 700.0, 0.0),
        visual: true,
        age_s: 0.0,
    });
    p.dice.g[0] = 1.0;
    b.decide(&p, &s);
    assert_ne!(b.state, BanditState::DefendMissile);
    assert_eq!(b.defend_at_s, Some(f64::from(1.2f32)));
    assert!((b.break_tgo_s.expect("drawn") - (3.0 + f64::from(0.6f32))).abs() < 1e-12);
    p.t_s = 1.21;
    p.dice.g[0] = -2.0; // Drawn once only.
    assert_eq!(decided(&mut b, &p, &s), BanditState::DefendMissile);
    assert!((b.break_tgo_s.expect("drawn") - 3.6).abs() < 1e-6);
    // The missile gone: the bookkeeping clears.
    p.missile = None;
    p.t_s = 10.0;
    b.decide(&p, &s);
    assert_eq!(
        (b.defend_at_s, b.break_tgo_s, b.break_at_s),
        (None, None, None)
    );
}

#[test]
fn rule_3_return_and_its_hysteresis() {
    let s = skill(1);
    let mut b = BanditBrain::new(BrainKind::Normal, 1.0);
    let mut p = base(0.0);
    p.from_home_m = 41_000.0;
    assert_eq!(decided(&mut b, &p, &s), BanditState::Return);
    p.from_home_m = 35_000.0;
    p.t_s = 10.0;
    assert_eq!(
        decided(&mut b, &p, &s),
        BanditState::Return,
        "until inside 32 km"
    );
    p.from_home_m = 31_000.0;
    assert_eq!(decided(&mut b, &p, &s), BanditState::Patrol);
}

#[test]
fn rule_2_recover_now_and_until_clear_and_climbing() {
    let s = skill(1); // 300 m
    let mut b = BanditBrain::new(BrainKind::Normal, 1.0);
    let mut p = base(0.0);
    p.contact = Some(contact(0.0, 5_000.0, true));
    assert_eq!(decided(&mut b, &p, &s), BanditState::Pursue);
    p.t_s = 0.2;
    p.clearance_min_m = 290.0;
    assert_eq!(decided(&mut b, &p, &s), BanditState::Recover, "urgent");
    // Clear by 700 m but level: stays; climbing: leaves (after its commitment).
    p.clearance_min_m = 700.0;
    p.t_s = 5.0;
    assert_eq!(decided(&mut b, &p, &s), BanditState::Recover);
    p.own.vel_bf.z = 10.0;
    assert_eq!(decided(&mut b, &p, &s), BanditState::Pursue);
    // Under 150 m above the ground whatever the clearance ahead.
    let mut b = BanditBrain::new(BrainKind::Normal, 1.0);
    let mut q = base(0.0);
    q.own.agl_m = 140.0;
    assert_eq!(decided(&mut b, &q, &s), BanditState::Recover);
}

#[test]
fn rule_1_the_wreck_is_final() {
    let s = skill(2);
    let mut b = BanditBrain::new(BrainKind::Normal, 1.0);
    b.set_wreck(3.0);
    let mut p = base(4.0);
    p.clearance_min_m = 10.0;
    assert_eq!(decided(&mut b, &p, &s), BanditState::Wreck);
    let ck = Cockpit::air(Assist::Standard, p.own.pos_bf, 0.0, 200.0, 10_260.0);
    let out = b.act(&p, &s, &ck.airframe());
    assert!(out.manual && !out.fire_gun);
    assert_eq!(
        (out.controls.roll, out.controls.pitch, out.controls.throttle),
        (WRECK_ROLL, 0.0, 0.0)
    );
}

#[test]
fn the_test_brains() {
    let s = skill(3);
    let mut p = base(0.0);
    p.contact = Some(contact(0.0, 800.0, true));
    p.gun_aim_point_bf = Some(p.own.pos_bf + DVec3::new(0.0, 800.0, 0.0));
    // HoldCourse never leaves the patrol state (and flies straight); it never fires.
    let mut hold = BanditBrain::new(BrainKind::HoldCourse, 1.0);
    assert_eq!(decided(&mut hold, &p, &s), BanditState::Patrol);
    let ck = Cockpit::air(Assist::Standard, p.own.pos_bf, 0.0, 200.0, 10_260.0);
    assert!(!hold.act(&p, &s, &ck.airframe()).fire_gun);
    // DefendOnly ignores the gun and the player, and defends a missile.
    let mut d = BanditBrain::new(BrainKind::DefendOnly, 1.0);
    p.threat_ata_rad = Some(0.0);
    assert_eq!(decided(&mut d, &p, &s), BanditState::Patrol);
    p.missile = Some(Contact {
        pos_bf: p.own.pos_bf - DVec3::new(0.0, 2_000.0, 0.0),
        vel_bf: DVec3::new(0.0, 700.0, 0.0),
        visual: true,
        age_s: 0.0,
    });
    p.t_s = 0.2;
    d.decide(&p, &s);
    p.t_s = 1.5;
    assert_eq!(decided(&mut d, &p, &s), BanditState::DefendMissile);
}

#[test]
fn the_recruit_holds_its_fire_and_the_others_fire_in_bursts() {
    let mut p = base(0.0);
    p.contact = Some(contact(0.0, 600.0, true));
    p.gun_aim_point_bf = Some(p.own.pos_bf + DVec3::new(0.0, 600.0, 0.0));
    let ck = Cockpit::air(Assist::Standard, p.own.pos_bf, 0.0, 200.0, 10_260.0);
    let af = ck.airframe();
    // Its nose exactly on the aim point (the cockpit's nose is a few degrees up: aim there).
    let nose = ck.ac.body.att.rotate(DVec3::X);
    p.own.att = ck.ac.body.att;
    p.own.vel_bf = ck.ac.body.vel_bf;
    p.gun_aim_point_bf = Some(p.own.pos_bf + nose * 600.0);
    for (rank, fires) in [(1, false), (2, true), (3, true)] {
        let s = skill(rank);
        let mut b = BanditBrain::new(BrainKind::Normal, 1.0);
        b.decide(&p, &s);
        assert_eq!(b.state, BanditState::Gun, "rank {rank}");
        let mut pattern = Vec::new();
        for k in 0..60 {
            let mut q = p;
            q.t_s = f64::from(k) * DT;
            pattern.push(b.act(&q, &s, &af).fire_gun);
        }
        assert_eq!(pattern.iter().any(|f| *f), fires, "rank {rank}");
        if fires {
            // 0.6 s on, 0.4 s off: about 36 ticks of the first second.
            let on = pattern.iter().filter(|f| **f).count();
            assert!((35..=37).contains(&on), "rank {rank}: {on}");
            assert!(pattern[0] && pattern[30] && !pattern[45] && !pattern[59]);
        }
    }
}

/// Every field of the percept made NaN in turn.
fn nan_percepts() -> Vec<Percept> {
    let mut p = base(1.0);
    p.contact = Some(contact(0.0, 900.0, true));
    p.missile = Some(contact(0.0, -2_000.0, true));
    p.threat_ata_rad = Some(0.1);
    p.gun_aim_point_bf = Some(DVec3::new(5_000.0, 15_900.0, 4_500.0));
    let n = f64::NAN;
    let edits: Vec<fn(&mut Percept, f64)> = vec![
        |p, n| p.t_s = n,
        |p, n| p.own.pos_bf.x = n,
        |p, n| p.own.vel_bf.y = n,
        |p, n| p.own.att.w = n,
        |p, n| p.own.cas_mps = n,
        |p, n| p.own.tas_mps = n,
        |p, n| p.own.agl_m = n,
        |p, n| p.own.alt_msl_m = n,
        |p, n| p.own.up_bf.z = n,
        |p, n| p.own.n_max = n,
        |p, n| p.contact.as_mut().expect("set").pos_bf.z = n,
        |p, n| p.contact.as_mut().expect("set").vel_bf.x = n,
        |p, n| p.contact.as_mut().expect("set").age_s = n,
        |p, n| p.threat_ata_rad = Some(n),
        |p, n| p.missile.as_mut().expect("set").pos_bf.y = n,
        |p, n| p.gun_aim_point_bf = Some(DVec3::new(n, 0.0, 0.0)),
        |p, n| p.clearance_min_m = n,
        |p, n| p.home_bf.x = n,
        |p, n| p.arena_radius_m = n,
        |p, n| p.from_home_m = n,
        |p, n| p.cap_centre_bf.y = n,
        |p, n| p.cap_radius_m = n,
        |p, n| p.cap_alt_msl_m = n,
        |p, n| p.dice.u[2] = n,
        |p, n| p.dice.g[3] = n,
    ];
    edits
        .into_iter()
        .map(|f| {
            let mut q = p;
            f(&mut q, n);
            q
        })
        .collect()
}

#[test]
fn ai6_a_nan_anywhere_recovers_with_finite_controls_in_one_tick() {
    let ck = Cockpit::air(
        Assist::Standard,
        DVec3::new(5_000.0, 15_000.0, 4_500.0),
        0.0,
        200.0,
        10_260.0,
    );
    let af = ck.airframe();
    let percepts = nan_percepts();
    assert_eq!(percepts.len(), 25);
    for (i, p) in percepts.iter().enumerate() {
        assert!(!p.is_finite(), "edit {i}");
        for rank in [1, 2, 3] {
            let s = skill(rank);
            for start in [
                BanditState::Patrol,
                BanditState::Gun,
                BanditState::DefendGun,
            ] {
                // Through act alone (a tick between decisions) and through decide.
                let mut b = BanditBrain::new(BrainKind::Normal, 1.0);
                b.state = start;
                let out = b.act(p, &s, &af);
                assert_eq!(b.state, BanditState::Recover, "edit {i}");
                let c = out.controls;
                for x in [c.pitch, c.roll, c.yaw] {
                    assert!(x.is_finite() && (-1.0..=1.0).contains(&x), "edit {i}");
                }
                assert!((0.0..=1.0).contains(&c.throttle) && !out.fire_gun);
                let mut b = BanditBrain::new(BrainKind::Normal, 1.0);
                b.state = start;
                b.decide(p, &s);
                assert_eq!(b.state, BanditState::Recover, "edit {i}");
            }
        }
    }
    // The base percept is finite.
    assert!(base(0.0).is_finite());
}

#[test]
fn every_field_reaches_the_hash() {
    let b = BanditBrain::new(BrainKind::Normal, 1.0);
    let edits: Vec<fn(&mut BanditBrain)> = vec![
        |b| b.state = BanditState::Gun,
        |b| b.kind = BrainKind::DefendOnly,
        |b| b.since_s = 1.0,
        |b| b.commit_until_s = 1.0,
        |b| b.speed_i = 1.0,
        |b| b.aim_offset[0] = 1e-3,
        |b| b.aim_offset[1] = 1e-3,
        |b| b.aim_until_s = 1.0,
        |b| b.burst_until_s = 1.0,
        |b| b.pause_until_s = 1.0,
        |b| b.jink_sign = -1.0,
        |b| b.jink_until_s = 1.0,
        |b| b.defend_at_s = Some(0.0),
        |b| b.break_tgo_s = Some(3.0),
        |b| b.break_at_s = Some(0.0),
        |b| b.gun_threat_s = Some(0.0),
        |b| b.orbit_sign = -1.0,
        |b| b.hold.alt_msl_m = 1.0,
        |b| b.hold.heading_rad = 1.0,
        |b| b.hold.cas_mps = 1.0,
        |b| b.hold.speed_i = 1.0,
        |b| b.hold.set = true,
        |b| b.hold.fired = true,
    ];
    for (i, f) in edits.into_iter().enumerate() {
        let mut c = b;
        f(&mut c);
        assert_ne!(b.words(), c.words(), "edit {i}");
    }
    assert_eq!(
        [BanditState::Patrol, BanditState::Wreck].map(BanditState::code),
        [0, 9]
    );
}

#[test]
fn the_patrol_orbits_at_its_height() {
    // Two minutes of PATROL from the orbit's east side: the bandit stays within 300 m of 4 500 m
    // and within 2 km of the 5 km circle, and goes round clockwise (the bearing grows).
    let s = skill(1);
    let mut p = base(0.0);
    let mut ck = Cockpit::air(
        Assist::Standard,
        p.own.pos_bf,
        core::f64::consts::PI,
        200.0,
        10_260.0,
    );
    let mut b = BanditBrain::new(BrainKind::Normal, 1.0);
    let bearing = |pos: DVec3| dmath::atan2(pos.x, pos.y - 15_000.0);
    let b0 = bearing(ck.ac.body.pos_bf);
    let mut turned = 0.0;
    let mut last = b0;
    for k in 0..(120.0 / DT) as u32 {
        let af = ck.airframe();
        let sensed = af.sensed();
        p.t_s = ck.t();
        p.own.pos_bf = ck.ac.body.pos_bf;
        p.own.vel_bf = ck.ac.body.vel_bf;
        p.own.att = ck.ac.body.att;
        p.own.cas_mps = sensed.cas;
        p.own.tas_mps = sensed.v;
        p.own.alt_msl_m = ck.height();
        if k % 12 == 6 {
            b.decide(&p, &s);
        }
        let out = b.act(&p, &s, &af);
        ck.step(&out.controls, Presses::default());
        let now = bearing(ck.ac.body.pos_bf);
        turned += fcs::tester::geom::wrap(now - last);
        last = now;
        if k > 600 {
            assert!(
                (ck.height() - 4_500.0).abs() < 300.0,
                "t {} h {}",
                ck.t(),
                ck.height()
            );
            let r = DVec3::new(ck.ac.body.pos_bf.x, ck.ac.body.pos_bf.y - 15_000.0, 0.0).length();
            assert!((r - 5_000.0).abs() < 2_000.0, "t {} r {r}", ck.t());
        }
    }
    assert_eq!(b.state, BanditState::Patrol);
    assert!(turned > 0.5, "clockwise: {turned} rad");
}

#[test]
fn the_speed_guard_closes_the_throttle_and_limits_the_dive_near_vmo() {
    let dive = DVec3::new(0.0, 1.0, -1.0).normalized_or(DVec3::Y); // 45° down
    let goal = || Goal {
        dir_bf: dive,
        n_cap: 4.0,
        throttle: MILITARY,
    };
    let mut o = base(0.0).own;
    o.vel_bf = dive * 300.0;
    // Below the share: untouched.
    o.cas_mps = 0.9 * VMO_MPS;
    let mut g = goal();
    assert!(!speed_guard(&mut g, &o));
    assert_eq!(g, goal());
    // Half way from the share to V_mo: idle, and no steeper than half of 15° down.
    o.cas_mps = f64::midpoint(GUARD_VMO_SHARE, 1.0) * VMO_MPS;
    let mut g = goal();
    assert!(speed_guard(&mut g, &o));
    assert_eq!(g.throttle, 0.0);
    let gamma = dmath::asin(g.dir_bf.normalized_or(DVec3::Y).dot(o.up_bf));
    assert!((gamma - 0.5 * GUARD_GAMMA_RAD).abs() < 1e-9, "{gamma}");
    assert!(
        g.dir_bf.x.abs() < 1e-9 && g.dir_bf.y > 0.0,
        "still northbound"
    );
    // At V_mo and above: level at the least. A climb is left alone.
    o.cas_mps = 1.05 * VMO_MPS;
    let mut g = goal();
    assert!(speed_guard(&mut g, &o));
    assert!(g.dir_bf.dot(o.up_bf).abs() < 1e-9);
    let climb = DVec3::new(0.0, 1.0, 0.5);
    let mut g = Goal {
        dir_bf: climb,
        ..goal()
    };
    assert!(speed_guard(&mut g, &o));
    assert_eq!(g.dir_bf, climb);
}
