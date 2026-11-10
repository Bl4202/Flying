use super::*;
use crate::test_content;
use flight::env::FlatSea;
use flight::physical::EARTH_RADIUS_M;

const DT: f64 = 1.0 / 60.0;

fn dart1() -> MissileParams {
    MissileParams::from_def(test_content::missile(), DT).expect("a missile")
}

fn locked() -> SeekerState {
    SeekerState {
        mode: SeekerMode::Locked,
        tone_ticks: 30,
        los_clear: true,
    }
}

/// Eastbound at `alt` m.
fn eastbound_at(speed: f64, alt: f64) -> Missile {
    launch(
        &dart1(),
        DVec3::new(0.0, 0.0, alt),
        DVec3::new(speed, 0.0, 0.0),
        locked(),
    )
}

/// Eastbound at 4 000 m.
fn eastbound(speed: f64) -> Missile {
    launch(
        &dart1(),
        DVec3::new(0.0, 0.0, 4_000.0),
        DVec3::new(speed, 0.0, 0.0),
        locked(),
    )
}

#[test]
fn the_dart1_from_its_entry() {
    let p = dart1();
    assert_eq!(
        (p.mass_kg, p.thrust_n, p.burn_s, p.propellant_kg),
        (85.0, 11_000.0, 2.5, 11.5)
    );
    assert_eq!(
        (p.cn_max, p.g_max, p.g_min_speed_mps, p.n_prime, p.tau_s),
        (8.0, 30.0, 150.0, 4.0, f64::from(0.4_f32))
    );
    assert_eq!(
        (
            p.arm_dist_m,
            p.trigger_radius_m,
            p.lethal_radius_m,
            p.damage_j
        ),
        (300.0, 8.0, 8.0, 2_548_000.0)
    );
    assert_eq!(
        (p.max_time_s, p.min_speed_mps, p.target_lost_s),
        (25.0, 150.0, 2.0)
    );
    assert_eq!(
        (p.launch.min_range_m, p.launch.safe_sep_s, p.launch.max_g),
        (450.0, 0.5, 7.5)
    );
    assert_eq!((p.l_eff_burning_m, p.l_eff_m), (3.0, 0.5));
    assert!(MissileParams::from_def(test_content::gun(), DT).is_none());
    assert_eq!(
        [
            MissilePhase::Free,
            MissilePhase::Boost,
            MissilePhase::Coast,
            MissilePhase::Done
        ]
        .map(MissilePhase::code),
        [0, 1, 2, 3]
    );
}

#[test]
fn launch_limits_in_order() {
    let l = dart1().launch;
    let ok = (2_000.0, 3.0, 0.1, 230.0);
    assert_eq!(l.check(ok.0, ok.1, ok.2, ok.3), None);
    assert_eq!(
        l.check(449.0, 9.0, 1.0, 10.0),
        Some(LaunchRefusal::TooClose)
    );
    assert_eq!(l.check(ok.0, -7.6, 1.0, 10.0), Some(LaunchRefusal::GLimit));
    assert_eq!(
        l.check(ok.0, ok.1, 0.44, 10.0),
        Some(LaunchRefusal::AlphaLimit)
    );
    assert_eq!(l.check(ok.0, ok.1, ok.2, 79.0), Some(LaunchRefusal::Slow));
    assert_eq!(
        l.check(f64::NAN, ok.1, ok.2, ok.3),
        Some(LaunchRefusal::TooClose)
    );
}

#[test]
fn the_boost_worked_example() {
    // One tick from launch in still air at 4 000 m, 250 m/s east, no command: thrust 11 kN on
    // 85 kg less drag, gravity down; mass falls by 11.5 / 2.5 / 60 kg.
    let p = dart1();
    let atmo = Atmosphere::new();
    let mut m = eastbound(250.0);
    step_missile(&mut m, &p, DVec3::ZERO, &FlatSea, &atmo, 0.0, DT);
    let air = atmo.sample(4_000.0, FlatSea.day());
    let qbar = 0.5 * air.rho_kgpm3 * 250.0 * 250.0;
    let drag = qbar * p.s_ref_m2 * p.cd0_boost.eval(250.0 / air.a_mps);
    let ax = (p.thrust_n - drag) / p.mass_kg;
    assert!(
        (m.vel.x - 250.0 - ax * DT).abs() < 0.01,
        "{:?} vs {ax}",
        m.vel
    );
    assert!((m.vel.z + G0 * DT).abs() < 1e-3, "{:?}", m.vel);
    assert!(
        (m.mass - (85.0 - 11.5 / 2.5 * DT)).abs() < 1e-12,
        "{}",
        m.mass
    );
    assert!((m.t_s - DT).abs() < 1e-15 && m.phase == MissilePhase::Boost);
    assert!((m.flown_m - 250.0 * DT).abs() < 0.1, "{}", m.flown_m);
    // Burnout after 150 ticks exactly, with the propellant gone.
    for k in 1..149 {
        step_missile(
            &mut m,
            &p,
            DVec3::ZERO,
            &FlatSea,
            &atmo,
            f64::from(k) * DT,
            DT,
        );
        assert_eq!(m.phase, MissilePhase::Boost, "tick {k}");
    }
    step_missile(&mut m, &p, DVec3::ZERO, &FlatSea, &atmo, 149.0 * DT, DT);
    assert_eq!(m.phase, MissilePhase::Coast);
    assert!((m.mass - 73.5).abs() < 1e-9, "{}", m.mass);
    assert!(
        m.vel.length() > 500.0,
        "about Mach 1.7 at burnout: {:?}",
        m.vel
    );
    // A free slot does not move.
    let mut free = Missile::default();
    step_missile(&mut free, &p, DVec3::ZERO, &FlatSea, &atmo, 0.0, DT);
    assert_eq!(free, Missile::default());
}

/// The specific energy `½ V² + Φ(h)`, with `Φ = g0 R h / (R + h)` the potential of the flat sea's
/// gravity `g0 (R / (R + h))²`.
fn specific_energy(m: &Missile) -> f64 {
    let h = m.pos.z;
    0.5 * m.vel.length_squared() + G0 * EARTH_RADIUS_M * h / (EARTH_RADIUS_M + h)
}

/// The command of tick `k` in the MS1 schedule: a weave across the flight path (BF y) and gravity
/// compensation with a slow pitch weave (BF z), at most about 3 g, inside the airframe's limits.
fn ms1_command(k: u32) -> DVec3 {
    let t = f64::from(k) * DT;
    DVec3::new(
        0.0,
        20.0 * dmath::sin(0.7 * t),
        G0 + 15.0 * dmath::sin(0.45 * t) * dmath::cos(0.2 * t),
    )
}

#[test]
fn ms1_ac_15_3_sixty_seconds_against_a_480_hz_reference() {
    let p = dart1();
    let atmo = Atmosphere::new();
    // The game's integrator (`step_missile`, whatever rate it uses) against RK4 at 480 Hz.
    let fly = |substeps: Option<u32>| {
        let mut m = eastbound_at(300.0, 6_000.0);
        for k in 0..3_600 {
            let (a, t) = (ms1_command(k), f64::from(k) * DT);
            match substeps {
                None => step_missile(&mut m, &p, a, &FlatSea, &atmo, t, DT),
                Some(n) => step_missile_n(&mut m, &p, a, &FlatSea, &atmo, t, DT, n),
            }
        }
        m
    };
    let (game, reference) = (fly(None), fly(Some(8)));
    let err = (game.pos - reference.pos).length();
    assert!(err < 0.2, "{err} m after 60 s");
    println!("MS1: {err:.4} m after 60 s");
    // A real flight: tens of kilometres, weaving, and above the 150 m/s below which it could not
    // manoeuvre all the way.
    assert!(
        reference.flown_m > 20_000.0 && reference.pos.y.abs() > 100.0,
        "{reference:?}"
    );
    assert!(reference.vel.length() > 150.0, "{reference:?}");
    assert!((reference.t_s - 60.0).abs() < 1e-9);
}

#[test]
fn ms1_an_unguided_coast_never_gains_energy() {
    let p = dart1();
    let atmo = Atmosphere::new();
    // Burnt out, climbing at 45° at 600 m/s, no command: drag only takes energy away.
    let mut m = launch(
        &p,
        DVec3::new(0.0, 0.0, 4_000.0),
        DVec3::new(424.264, 0.0, 424.264),
        locked(),
    );
    m.t_s = p.burn_s;
    m.mass = p.mass_kg - p.propellant_kg;
    m.phase = MissilePhase::Coast;
    let mut e = specific_energy(&m);
    for k in 0..3_600 {
        step_missile(
            &mut m,
            &p,
            DVec3::ZERO,
            &FlatSea,
            &atmo,
            f64::from(k) * DT,
            DT,
        );
        let e_next = specific_energy(&m);
        assert!(e_next <= e, "tick {k}: {e} -> {e_next}");
        e = e_next;
    }
}

#[test]
fn ms3_pn_is_perpendicular_to_the_line_of_sight_and_zero_on_a_collision_course() {
    // Worked example: target 1 000 m north, crossing east at 200 m/s; missile still. Ω = R × Vr /
    // R² = (0, 1000, 0) × (200, 0, 0) / 10⁶ = (0, 0, −0.2); Vc = 0; so a = 0 here.
    let r = DVec3::new(0.0, 1_000.0, 0.0);
    assert_eq!(pn_accel(r, DVec3::new(200.0, 0.0, 0.0), 4.0), DVec3::ZERO);
    // Closing at 500 m/s while the target crosses east at 200 m/s: Vc = 500,
    // Ω × R̂ = (0, 0, −0.2) × (0, 1, 0) = (0.2, 0, 0), a = 4 · 500 · 0.2 = 400 m/s² east.
    let a = pn_accel(r, DVec3::new(200.0, -500.0, 0.0), 4.0);
    assert!((a - DVec3::new(400.0, 0.0, 0.0)).length() < 1e-9, "{a:?}");
    let mut state = 5u64;
    let mut next = || {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((state >> 11) as f64 / (1u64 << 53) as f64) * 2.0 - 1.0
    };
    for _ in 0..500 {
        let r = DVec3::new(next(), next(), next()) * 5_000.0;
        let vr = DVec3::new(next(), next(), next()) * 800.0;
        let a = pn_accel(r, vr, 4.0);
        assert!(
            a.dot(r).abs() <= 1e-9 * a.length() * r.length(),
            "{a:?} {r:?}"
        );
        // A collision course: the relative velocity along the line of sight.
        let c = pn_accel(r, r * (-0.1 * next().abs() - 0.01), 4.0);
        assert!(c.length() < 1e-9, "{c:?}");
    }
    assert_eq!(pn_accel(DVec3::ZERO, DVec3::X, 4.0), DVec3::ZERO);
}

#[test]
fn ms3_the_lateral_limit() {
    let p = dart1();
    let atmo = Atmosphere::new();
    let limit = |speed: f64, h: f64, mass: f64| {
        let mut m = eastbound(speed);
        m.pos.z = h;
        m.mass = mass;
        lateral_limit(&m, &p, &FlatSea, &atmo, 0.0)
    };
    // Slow and high: the aerodynamic limit q̄ S C_N,max / m (C_N,max 8, the WP4 tuning).
    let rho = atmo.sample(8_000.0, FlatSea.day()).rho_kgpm3;
    let aero = 0.5 * rho * 300.0 * 300.0 * p.s_ref_m2 * 8.0 / 73.5;
    assert!((limit(300.0, 8_000.0, 73.5) - aero).abs() < 1e-9);
    assert!(aero < 30.0 * G0);
    // Fast and low: the structural 30 g.
    assert_eq!(limit(800.0, 500.0, 85.0), 30.0 * G0);
    // Below 150 m/s: nothing.
    assert_eq!(limit(149.0, 500.0, 85.0), 0.0);
    // The command is held to it (closing at 247 m/s, the line of sight turning at 0.11 rad/s: PN
    // asks for 112 m/s²).
    let target = TargetView {
        pos_bf: DVec3::new(3_000.0, 500.0, 4_000.0),
        vel_bf: DVec3::new(0.0, 300.0, 0.0),
        att: simmath::math::DQuat::IDENTITY,
        intensity: 1.0,
    };
    let m = eastbound(300.0);
    let a = guide(&m, &p, &target, [0.0, 0.0], &FlatSea, &atmo, 0.0);
    let lim = lateral_limit(&m, &p, &FlatSea, &atmo, 0.0);
    assert!((a.length() - lim).abs() < 1e-9, "{} vs {lim}", a.length());
    assert!(a.dot(m.vel).abs() < 1e-9);
    // The airframe is held to it as well: a command beyond the limit is pulled only to the limit.
    let mut pulled = m;
    step_missile(
        &mut pulled,
        &p,
        DVec3::new(0.0, 0.0, 400.0),
        &FlatSea,
        &atmo,
        0.0,
        DT,
    );
    assert!(
        pulled.a_lat.length() <= lim * 1.01,
        "{:?} vs {lim}",
        pulled.a_lat
    );
    // Not locked: no command.
    let mut searching = m;
    searching.seeker = SeekerState::searching();
    assert_eq!(
        guide(&searching, &p, &target, [0.0, 0.0], &FlatSea, &atmo, 0.0),
        DVec3::ZERO
    );
}

#[test]
fn ms3_gravity_is_cancelled_on_a_collision_course() {
    // Level, eastbound, target dead ahead and still: PN is zero, so the command is the gravity
    // compensation, G0 up (slightly less with height).
    let p = dart1();
    let atmo = Atmosphere::new();
    let m = eastbound(500.0);
    let target = TargetView {
        pos_bf: DVec3::new(3_000.0, 0.0, 4_000.0),
        vel_bf: DVec3::ZERO,
        att: simmath::math::DQuat::IDENTITY,
        intensity: 1.0,
    };
    let a = guide(&m, &p, &target, [0.0, 0.0], &FlatSea, &atmo, 0.0);
    let g = FlatSea.gravity_bf(m.pos).length();
    assert!((a - DVec3::new(0.0, 0.0, g)).length() < 1e-6, "{a:?}");
    // Seeker noise moves the line of sight by about σ |R| (0.1° at 3 km: 5.2 m).
    let noisy = guide(&m, &p, &target, [1.0, 0.0], &FlatSea, &atmo, 0.0);
    assert_ne!(noisy, a);
}

#[test]
fn ms3_the_autopilot_reaches_63_percent_after_tau() {
    let p = dart1();
    let atmo = Atmosphere::new();
    let mut m = eastbound(600.0);
    m.t_s = p.burn_s;
    m.phase = MissilePhase::Coast;
    m.mass = 73.5;
    let step = 10.0 * G0;
    let mut reached = None;
    for k in 0..60 {
        let v_hat = m.vel.normalized_or(DVec3::X);
        let up = (DVec3::Z - v_hat * DVec3::Z.dot(v_hat)).normalized_or(DVec3::Z);
        step_missile(
            &mut m,
            &p,
            up * step,
            &FlatSea,
            &atmo,
            f64::from(k) * DT,
            DT,
        );
        if reached.is_none() && m.a_lat.length() >= (1.0 - dmath::exp(-1.0)) * step {
            reached = Some(k + 1);
        }
    }
    let t = f64::from(reached.expect("reaches it")) * DT;
    assert!((t - p.tau_s).abs() <= DT + 1e-12, "{t}");
}

#[test]
fn expiry_rules() {
    let p = dart1();
    let mut m = eastbound(600.0);
    assert_eq!(missile_expiry(&m, &p, &FlatSea, true), None);
    m.t_s = 25.0;
    assert_eq!(
        missile_expiry(&m, &p, &FlatSea, true),
        Some(Expiry::Timeout)
    );
    m.t_s = 10.0;
    m.vel = DVec3::new(149.0, 0.0, 0.0);
    assert_eq!(missile_expiry(&m, &p, &FlatSea, true), Some(Expiry::Slow));
    m.t_s = 2.0;
    assert_eq!(missile_expiry(&m, &p, &FlatSea, true), None);
    m.vel = DVec3::new(600.0, 0.0, 0.0);
    m.target_lost_s = 1.9;
    assert_eq!(missile_expiry(&m, &p, &FlatSea, false), None);
    m.target_lost_s = 2.0;
    assert_eq!(
        missile_expiry(&m, &p, &FlatSea, false),
        Some(Expiry::TargetLost)
    );
    assert_eq!(missile_expiry(&m, &p, &FlatSea, true), None);
    m.pos.z = -0.5;
    assert_eq!(missile_expiry(&m, &p, &FlatSea, true), Some(Expiry::Ground));
    // The eye sees the motor's plume while it burns.
    let mut v = eastbound(300.0);
    assert_eq!(visual_l_eff_m(&v, &p), 3.0);
    v.phase = MissilePhase::Coast;
    assert_eq!(visual_l_eff_m(&v, &p), 0.5);
}
