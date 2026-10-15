use super::*;
use crate::actuators::Surfaces;
use crate::aircraft::AircraftModel;
use crate::energy::level_turn_load_factor;
use crate::env::SphereSea;
use crate::physical::G0;
use crate::test_content::registry;

fn kestrel() -> AircraftModel {
    let r = registry();
    AircraftModel::from_registry(r, r.aircraft_handles().next().unwrap(), 1.0 / 60.0).unwrap()
}

fn setup(m: &AircraftModel, alt_m: f64, mass_kg: f64) -> TrimSetup {
    TrimSetup {
        pos_bf: DVec3::new(0.0, 0.0, alt_m),
        fuel: m.mass.load_for(mass_kg - m.mass.empty_kg()),
        config: Surfaces::NEUTRAL,
    }
}

fn speed_of(m: &AircraftModel, alt_m: f64, mach: f64) -> f64 {
    mach * m
        .atmosphere()
        .sample(alt_m, crate::atmosphere::NonStandardDay::ISA)
        .a_mps
}

fn level(m: &AircraftModel, alt_m: f64, mass_kg: f64, mach: f64) -> TrimResult {
    let world = SphereSea::open();
    let s = setup(m, alt_m, mass_kg);
    let guess = guess_level(m, &s, speed_of(m, alt_m, mach), 1.0, &world);
    solve(m, &TrimProblem::level(), &s, guess, &world)
}

// The tests fly over the open sphere: at sea level over the sea the wing would be in ground
// effect, with no induced drag at all (GDD 11.6.5).

#[test]
fn a_level_trim_converges_and_holds() {
    let m = kestrel();
    let world = SphereSea::open();
    let r = level(&m, 5000.0, 10_000.0, 0.6);
    assert!(r.converged, "{r:?}");
    assert!(r.iterations < 15, "{} iterations", r.iterations);
    let p = r.point;
    // C_L = W / qS = 98 067 / (0.5 * 0.7361 * 192.3^2 * 28) = 0.257: alpha about 4 deg.
    assert!(
        (p.alpha_rad.to_degrees() - 4.1).abs() < 0.3,
        "{}",
        p.alpha_rad.to_degrees()
    );
    assert!(
        p.elevator_rad < 0.0 && p.elevator_rad.to_degrees() > -3.0,
        "{}",
        p.elevator_rad
    );
    assert!(
        p.throttle > 0.0 && p.throttle < 1.0,
        "dry power: {}",
        p.throttle
    );
    // Flown for 5 s hands-off from the trim, it stays level: the residual really is zero.
    let s0 = setup(&m, 5000.0, 10_000.0);
    let mut state = state_of(&m, &s0, &p, &world);
    state.mass_frozen = true;
    let cmd = commands_of(&m, &s0, &p);
    for tick in 0..300 {
        m.step_tick(&mut state, &cmd, &world, f64::from(tick) / 60.0);
    }
    let climb = world.up_bf(state.body.pos_bf).dot(state.body.vel_bf);
    assert!(climb.abs() < 0.05, "vertical speed {climb}");
    assert!((state.body.vel_bf.length() - p.speed_mps).abs() < 0.05);
    // The same solve again gives the same bits and the same count.
    assert_eq!(level(&m, 5000.0, 10_000.0, 0.6), r);
}

/// Can the Kestrel fly level here at all? Lift below 85 % of `C_L,max` and full-afterburner thrust
/// 10 % above the clean polar's drag, estimated independently of the solver.
fn trim_should_exist(m: &AircraftModel, alt_m: f64, mass_kg: f64, mach: f64) -> bool {
    let world = SphereSea::open();
    let air = m
        .atmosphere()
        .sample(alt_m, crate::atmosphere::NonStandardDay::ISA);
    let v = mach * air.a_mps;
    let qs = 0.5 * air.rho_kgpm3 * v * v * 28.0;
    let cl = mass_kg * G0 / qs;
    let wave = [
        (0.75, 0.0),
        (0.85, 0.003),
        (0.95, 0.01),
        (1.05, 0.026),
        (1.2, 0.025),
        (1.5, 0.019),
        (2.0, 0.013),
    ];
    let mut cdw: f64 = 0.0;
    for w in wave.windows(2) {
        if mach >= w[0].0 && mach <= w[1].0 {
            cdw = w[0].1 + (w[1].1 - w[0].1) * (mach - w[0].0) / (w[1].0 - w[0].0);
        }
    }
    let drag = qs * (0.0235 + cdw + 0.135 * cl * cl);
    let s = setup(m, alt_m, mass_kg);
    let mut probe = state_of(
        m,
        &s,
        &TrimPoint {
            speed_mps: v,
            throttle: 2.0,
            ..TrimPoint::default()
        },
        &world,
    );
    probe.surfaces = Surfaces::NEUTRAL;
    let thrust = m.telemetry(&probe, &world, 0.0).thrust_n;
    cl < 0.85 * m.aero.cl_max(mach)
        && thrust > 1.1 * drag
        && v * dmath_sqrt(air.rho_kgpm3 / 1.225) < 360.0
}

fn dmath_sqrt(x: f64) -> f64 {
    simmath::dmath::sqrt(x)
}

#[test]
fn ac_11_1_level_trim_across_the_envelope() {
    // M 0.3 to 1.8, sea level to 15 km, 8 500 to 11 500 kg (the most fuel alone can carry; the
    // GDD's 14 500 kg needs stores, which arrive later). Wherever level flight exists the trim
    // must converge with every residual below 0.01 m/s^2 (and rad/s^2).
    let m = kestrel();
    let mut trimmed = 0;
    for mach in [0.3, 0.45, 0.6, 0.75, 0.9, 1.2, 1.5, 1.8] {
        for alt in [0.0, 5000.0, 10_000.0, 15_000.0] {
            for mass in [8500.0, 10_000.0, 11_500.0] {
                if !trim_should_exist(&m, alt, mass, mach) {
                    continue;
                }
                let r = level(&m, alt, mass, mach);
                assert!(
                    r.converged && r.residual < 0.01,
                    "M {mach}, {alt} m, {mass} kg: {r:?}"
                );
                assert!(
                    r.point.elevator_rad.abs() < 23f64.to_radians(),
                    "M {mach}, {alt} m"
                );
                trimmed += 1;
            }
        }
    }
    assert!(trimmed >= 50, "only {trimmed} points have level flight");
}

#[test]
fn the_trimmed_cl_max_is_c4s() {
    // GDD C.4: 1.60 at low Mach, trimmed (the elevator cancels the moment). Scan alpha through
    // the stall at M 0.2, trimming the elevator at each angle.
    let m = kestrel();
    let world = SphereSea::open();
    let s = setup(&m, 1000.0, 9500.0);
    let v = speed_of(&m, 1000.0, 0.2);
    let mut best: f64 = 0.0;
    let mut elevator_at_best = 0.0;
    for i in 0..=200 {
        let alpha = (24.0 + f64::from(i) * 0.06).to_radians();
        let guess = TrimPoint {
            speed_mps: v,
            alpha_rad: alpha,
            throttle: 0.0,
            ..TrimPoint::default()
        };
        let r = solve(&m, &TrimProblem::elevator(), &s, guess, &world);
        assert!(r.converged, "alpha {}: {r:?}", alpha.to_degrees());
        let state = state_of(&m, &s, &r.point, &world);
        let t = m.telemetry(&state, &world, 0.0);
        let input = crate::aero::AeroInput {
            air: t.air,
            alpha_dot_rps: 0.0,
            surfaces: state.surfaces,
            height_agl_m: 1000.0,
            cg_body_m: m.mass.props(&s.fuel).cg_body_m,
        };
        let cl = m.aero.coefficients(&input).cl;
        if cl > best {
            best = cl;
            elevator_at_best = r.point.elevator_rad;
        }
    }
    assert!((best / 1.60 - 1.0).abs() < 0.005, "trimmed C_L,max {best}");
    // The design review's stall trim: about -7.9 deg.
    assert!(
        (elevator_at_best.to_degrees() + 7.9).abs() < 1.0,
        "{}",
        elevator_at_best.to_degrees()
    );
}

#[test]
fn trimmed_elevator_stays_within_23_degrees_over_the_envelope() {
    // At -3 g, 1 g and the lesser of 9 g and the C_L,max limit, inside Vmo and Mmo.
    let m = kestrel();
    let world = SphereSea::open();
    let mut points = 0;
    for mach in [0.3, 0.6, 0.9, 1.2, 1.5, 2.0] {
        for alt in [0.0, 5000.0, 11_000.0, 15_000.0] {
            let air = m
                .atmosphere()
                .sample(alt, crate::atmosphere::NonStandardDay::ISA);
            let v = mach * air.a_mps;
            if crate::airdata::cas_mps(air.p_pa, mach) > m.limits.vmo_mps {
                continue;
            }
            let s = setup(&m, alt, 10_000.0);
            let w = 10_000.0 * G0;
            let qs = 0.5 * air.rho_kgpm3 * v * v * 28.0;
            let n_stall = 0.97 * qs * m.aero.cl_max(mach) / w;
            let n_push = 0.6 * 0.9 * qs * m.aero.cl_max(mach) / w;
            for n in [(-3.0_f64).max(-n_push), 1.0, 9.0_f64.min(n_stall)] {
                if n_stall < 1.0 {
                    continue;
                }
                let mut guess = guess_level(&m, &s, v, n, &world);
                guess.pitch_rate_rps = (n - 1.0) * G0 / v;
                let r = solve(&m, &TrimProblem::lift_and_moment(), &s, guess, &world);
                assert!(r.converged, "M {mach}, {alt} m, n {n}: {r:?}");
                let de = r.point.elevator_rad.to_degrees();
                assert!(
                    de.abs() <= 23.0,
                    "M {mach}, {alt} m, n {n}: elevator {de} deg"
                );
                points += 1;
            }
        }
    }
    assert!(points > 40, "{points}");
}

#[test]
fn a_coordinated_five_g_turn() {
    let m = kestrel();
    let world = SphereSea::open();
    let s = setup(&m, 3000.0, 10_000.0);
    let v = speed_of(&m, 3000.0, 0.7);
    let n = 5.0;
    let omega = G0 * (n * n - 1.0_f64).sqrt() / v;
    let mut guess = guess_level(&m, &s, v, n, &world);
    guess.turn_rate_rps = omega;
    guess.bank_rad = (1.0 / n).acos_approx();
    let r = solve(&m, &TrimProblem::turn(), &s, guess, &world);
    assert!(r.converged, "{r:?}");
    let p = r.point;
    assert!((level_turn_load_factor(v, p.turn_rate_rps) - n).abs() < 1e-6);
    // A right turn banks right; the ball is centred and the sideslip small.
    assert!(
        p.bank_rad > 1.2 && p.bank_rad < 1.4,
        "bank {}",
        p.bank_rad.to_degrees()
    );
    assert!(p.beta_rad.abs() < 0.01, "beta {}", p.beta_rad.to_degrees());
    // Held for 3 s, the heading turns at the trimmed rate.
    let mut state = state_of(&m, &s, &p, &world);
    state.mass_frozen = true;
    let cmd = commands_of(&m, &s, &p);
    let h0 = crate::frames::euler(world.up_bf(state.body.pos_bf), state.body.att).heading_rad;
    for tick in 0..180 {
        m.step_tick(&mut state, &cmd, &world, f64::from(tick) / 60.0);
    }
    let h1 = crate::frames::euler(world.up_bf(state.body.pos_bf), state.body.att).heading_rad;
    assert!(
        ((h1 - h0) - 3.0 * omega).abs() < 0.02,
        "turned {} rad",
        h1 - h0
    );
}

trait AcosApprox {
    fn acos_approx(self) -> f64;
}

impl AcosApprox for f64 {
    fn acos_approx(self) -> f64 {
        simmath::dmath::acos(self)
    }
}

#[test]
fn sustained_turn_and_top_speed_cross_checks() {
    // Static versions of three C.9 points (the conformance test flies them dynamically).
    let m = kestrel();
    let world = SphereSea::open();
    // nsust_sl_m07: 10 500 kg, SL, M 0.7, full afterburner: 8.45 g (5 %).
    let s = setup(&m, 0.0, 10_500.0);
    let v = speed_of(&m, 0.0, 0.7);
    let mut guess = guess_level(&m, &s, v, 8.0, &world);
    guess.turn_rate_rps = G0 * 63.0_f64.sqrt() / v;
    guess.bank_rad = 1.44;
    guess.throttle = 2.0;
    let r = solve(&m, &TrimProblem::turn_at_throttle(), &s, guess, &world);
    assert!(r.converged, "{r:?}");
    let n = level_turn_load_factor(v, r.point.turn_rate_rps);
    assert!((n / 8.45 - 1.0).abs() < 0.05, "n {n}");
    // vmax_sl: 9 500 kg, SL, full afterburner: M 1.22 (3 %); C.8's exact crossing is 1.2288.
    let s = setup(&m, 0.0, 9500.0);
    let mut guess = guess_level(&m, &s, speed_of(&m, 0.0, 1.2), 1.0, &world);
    guess.throttle = 2.0;
    let r = solve(&m, &TrimProblem::level_at_throttle(), &s, guess, &world);
    assert!(r.converged, "{r:?}");
    let mach = r.point.speed_mps / speed_of(&m, 0.0, 1.0);
    assert!((mach / 1.22 - 1.0).abs() < 0.03, "M_max {mach}");
}

#[test]
fn an_idle_glide() {
    let m = kestrel();
    let world = SphereSea::open();
    let s = setup(&m, 4000.0, 9500.0);
    let mut guess = guess_level(&m, &s, 150.0, 1.0, &world);
    guess.throttle = 0.0;
    guess.gamma_rad = -0.1;
    let r = solve(&m, &TrimProblem::glide(), &s, guess, &world);
    assert!(r.converged, "{r:?}");
    // Best glide is near L/D max = 1 / (2 sqrt(C_D0 K)) = 8.9; at 150 m/s a little shallower
    // than -1/8.9 net of the idle thrust.
    assert!(
        r.point.gamma_rad < -0.04 && r.point.gamma_rad > -0.15,
        "{}",
        r.point.gamma_rad
    );
}

#[test]
fn an_impossible_trim_says_so() {
    // M 0.3 at 15 km needs far more lift than the wing has.
    let r = level(&kestrel(), 15_000.0, 11_500.0, 0.3);
    assert!(!r.converged);
    assert!(r.iterations <= MAX_ITERATIONS);
}
