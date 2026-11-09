//! Gun rounds (GDD 15.4.2, `AC-15.1`): point masses with quadratic drag, integrated with two
//! midpoint (RK2) sub-steps per tick.
//!
//! `a(p, v) = −k |v − w| (v − w) + g(p)`, `k = ρ(h) C_d(M) A / (2 m)`, `A = π (caliber / 2)²`,
//! `M = |v − w| / a(h)`, with the air from the atmosphere table at the round's height on the day,
//! `g` and the wind `w` from the [`FlightEnv`].

use content::WeaponDef;
use flight::atmosphere::Atmosphere;
use flight::env::FlightEnv;
use flight::table::Table1D;
use simmath::dmath;
use simmath::math::{DQuat, DVec3};

use crate::orthonormal_pair;

/// A gun's exterior ballistics and firing data, in SI units.
#[derive(Clone, Debug, PartialEq)]
pub struct GunBallistics {
    /// Cross-section of a round, m².
    pub area_m2: f64,
    /// Mass of a round, kg.
    pub mass_kg: f64,
    /// Drag coefficient against Mach number.
    pub cd: Table1D,
    /// Muzzle velocity relative to the gun, m/s.
    pub muzzle_mps: f64,
    /// Rate of fire, rounds per second.
    pub rate_rps: f64,
    /// Dispersion: the standard deviation of the Gaussian spread about the gun axis, on each of
    /// two axes, rad (GDD 15.4.1).
    pub dispersion_rad: f64,
    /// Age at which a round is removed, s.
    pub max_age_s: f64,
    /// Speed below which a round is removed, m/s.
    pub min_speed_mps: f64,
    /// Every n-th round is a tracer (0: none).
    pub tracer_every: u8,
    /// Rounds the gun holds when full.
    pub rounds: u16,
}

impl GunBallistics {
    /// The ballistics of a gun entry; `None` if the weapon is not a gun or its drag table is
    /// malformed (the baker refuses both).
    #[must_use]
    pub fn from_def(def: &WeaponDef) -> Option<Self> {
        let g = def.gun.as_ref()?;
        let radius_m = f64::from(g.caliber_mm) * 0.5e-3; // PHYS: mm to m, half the calibre
        Some(Self {
            area_m2: core::f64::consts::PI * radius_m * radius_m,
            mass_kg: f64::from(g.round_mass_kg),
            cd: Table1D::from_f32(&g.drag_cd.xs, &g.drag_cd.ys).ok()?,
            muzzle_mps: f64::from(g.muzzle_mps),
            rate_rps: f64::from(g.rpm) / 60.0, // PHYS: per minute to per second
            dispersion_rad: f64::from(g.dispersion_mrad) * 1e-3, // PHYS: mrad to rad
            max_age_s: f64::from(g.max_age_s),
            min_speed_mps: f64::from(g.min_speed_mps),
            tracer_every: g.ammo_types.first().map_or(0, |a| a.tracer_every),
            rounds: g.rounds,
        })
    }
}

/// One round in flight (BF).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Round {
    /// Position, m.
    pub pos: DVec3,
    /// Velocity, m/s.
    pub vel: DVec3,
    /// Time since it left the muzzle, s.
    pub age_s: f64,
}

/// The acceleration of a round at `p` moving at `v` at time `t_s`, m/s².
#[must_use]
pub fn round_accel<E: FlightEnv>(
    g: &GunBallistics,
    env: &E,
    atmo: &Atmosphere,
    p: DVec3,
    v: DVec3,
    t_s: f64,
) -> DVec3 {
    let air = atmo.sample(env.height_msl_m(p), env.day());
    let v_rel = v - env.wind_bf(p, t_s);
    let speed = v_rel.length();
    let k = air.rho_kgpm3 * g.cd.eval(speed / air.a_mps) * g.area_m2 / (2.0 * g.mass_kg);
    v_rel * (-k * speed) + env.gravity_bf(p)
}

/// One midpoint (RK2) step of `h` seconds from time `t_s`:
/// `v½ = v + a(p, v) h/2; p½ = p + v h/2; v ← v + a(p½, v½) h; p ← p + v½ h`. The age is not
/// changed.
pub fn rk2_substep<E: FlightEnv>(
    r: &mut Round,
    g: &GunBallistics,
    env: &E,
    atmo: &Atmosphere,
    t_s: f64,
    h: f64,
) {
    let v_half = r.vel + round_accel(g, env, atmo, r.pos, r.vel, t_s) * (0.5 * h);
    let p_half = r.pos + r.vel * (0.5 * h);
    r.vel += round_accel(g, env, atmo, p_half, v_half, t_s + 0.5 * h) * h;
    r.pos += v_half * h;
}

/// Advances a round by one tick of `dt` seconds starting at time `t_s` (two RK2 sub-steps of
/// `dt / 2`) and ages it by `dt`.
pub fn step_round<E: FlightEnv>(
    r: &mut Round,
    g: &GunBallistics,
    env: &E,
    atmo: &Atmosphere,
    t_s: f64,
    dt: f64,
) {
    let h = 0.5 * dt;
    rk2_substep(r, g, env, atmo, t_s, h);
    rk2_substep(r, g, env, atmo, t_s + h, h);
    r.age_s += dt;
}

/// True when a round is to be removed: older than `max_age_s`, slower than `min_speed_mps`, or
/// below the ground.
#[must_use]
pub fn round_expired<E: FlightEnv>(r: &Round, g: &GunBallistics, env: &E) -> bool {
    r.age_s > g.max_age_s
        || r.vel.length() < g.min_speed_mps
        || env.height_msl_m(r.pos) < env.ground_height_m(r.pos)
}

/// The gun's axis in body axes for a harmonisation angle `h` (rad, nose up positive):
/// `(cos h, 0, −sin h)`.
#[must_use]
pub fn gun_axis_body(harmonisation_rad: f64) -> DVec3 {
    DVec3::new(
        dmath::cos(harmonisation_rad),
        0.0,
        -dmath::sin(harmonisation_rad),
    )
}

/// One round's direction in body axes (GDD 15.4.1): the gun axis turned by a Gaussian offset,
/// `normalize(axis + σ (n1 e1 + n2 e2))`, with `n` two standard normals drawn by the caller and
/// `e1, e2` from [`orthonormal_pair`].
#[must_use]
pub fn dispersed_dir_body(axis_body: DVec3, sigma_rad: f64, n: [f64; 2]) -> DVec3 {
    let (e1, e2) = orthonormal_pair(axis_body);
    let d = axis_body + (e1 * n[0] + e2 * n[1]) * sigma_rad;
    if d.is_finite() {
        d.normalized_or(axis_body)
    } else {
        axis_body
    }
}

/// Where a round leaves the muzzle and how fast, BF:
/// `pos = ref + att(muzzle)`, `vel = v_ref + att(ω × muzzle) + muzzle_mps · att(dir)`.
#[must_use]
pub fn spawn_round(
    g: &GunBallistics,
    ref_bf: DVec3,
    vel_ref_bf: DVec3,
    att: DQuat,
    omega_body: DVec3,
    muzzle_m: DVec3,
    dir_body: DVec3,
) -> Round {
    Round {
        pos: ref_bf + att.rotate(muzzle_m),
        vel: spawn_velocity(
            vel_ref_bf,
            att,
            omega_body,
            muzzle_m,
            dir_body,
            g.muzzle_mps,
        ),
        age_s: 0.0,
    }
}

/// A round's velocity as it leaves the muzzle, BF: the reference point's velocity, the muzzle's
/// velocity from the aircraft's rotation, and the muzzle velocity along `dir_body`.
#[must_use]
pub fn spawn_velocity(
    vel_ref_bf: DVec3,
    att: DQuat,
    omega_body: DVec3,
    muzzle_m: DVec3,
    dir_body: DVec3,
    muzzle_mps: f64,
) -> DVec3 {
    vel_ref_bf + att.rotate(omega_body.cross(muzzle_m)) + att.rotate(dir_body) * muzzle_mps
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_content;
    use flight::env::FlatSea;
    use flight::physical::G0;

    fn gs27() -> GunBallistics {
        GunBallistics::from_def(test_content::gun()).expect("a gun")
    }

    #[test]
    fn the_gs27_from_its_entry() {
        let g = gs27();
        // 27 mm: A = π 0.0135² = 5.7256e-4 m².
        assert!(
            (g.area_m2 - core::f64::consts::PI * 0.0135 * 0.0135).abs() < 1e-15,
            "{}",
            g.area_m2
        );
        assert!((g.area_m2 - 5.7256e-4).abs() < 1e-8);
        assert_eq!((g.mass_kg, g.muzzle_mps), (f64::from(0.26f32), 1_025.0));
        assert!((g.rate_rps - 28.333_333_333_333_332).abs() < 1e-12);
        assert!((g.dispersion_rad - 0.5e-3).abs() < 1e-12);
        assert_eq!(
            (g.max_age_s, g.min_speed_mps, g.tracer_every, g.rounds),
            (3.0, 300.0, 4, 150)
        );
        assert_eq!(g.cd.eval(0.5), f64::from(0.25f32));
        assert!(GunBallistics::from_def(test_content::missile()).is_none());
    }

    #[test]
    fn the_acceleration_worked_example() {
        // At sea level, still air, 340.29 m/s (Mach 1): Cd lerps 0.25 → 0.38 over Mach 0.8 to 1.2,
        // so Cd(1.0) = 0.315; k = 1.225 · 0.315 · A / (2 · 0.26); a = −k v² along v, plus g.
        let g = gs27();
        let atmo = Atmosphere::new();
        let v = 340.294;
        let a = round_accel(
            &g,
            &FlatSea,
            &atmo,
            DVec3::ZERO,
            DVec3::new(v, 0.0, 0.0),
            0.0,
        );
        let mach = v / atmo.sample(0.0, FlatSea.day()).a_mps;
        let cd = g.cd.eval(mach);
        assert!((cd - 0.315).abs() < 1e-4, "{cd}");
        let rho = atmo.sample(0.0, FlatSea.day()).rho_kgpm3;
        assert!((rho - 1.225).abs() < 1e-5);
        let k = rho * cd * g.area_m2 / (2.0 * g.mass_kg);
        assert!((a.x + k * v * v).abs() < 1e-6 * k * v * v, "{a:?}");
        assert!((a.z + G0).abs() < 1e-12 && a.y == 0.0);
    }

    /// The step at which a round fired along +x from `p0` first reaches `x`, interpolated: (t, z).
    fn at_range(g: &GunBallistics, p0: DVec3, x: f64, dt: f64) -> (f64, DVec3, DVec3) {
        let atmo = Atmosphere::new();
        let mut r = Round {
            pos: p0,
            vel: DVec3::new(g.muzzle_mps, 0.0, 0.0),
            age_s: 0.0,
        };
        let mut t = 0.0;
        loop {
            let prev = r;
            step_round(&mut r, g, &FlatSea, &atmo, t, dt);
            t += dt;
            if r.pos.x - p0.x >= x {
                let f = (x - (prev.pos.x - p0.x)) / (r.pos.x - prev.pos.x);
                return (
                    t - dt + f * dt,
                    prev.pos.lerp(r.pos, f),
                    prev.vel.lerp(r.vel, f),
                );
            }
        }
    }

    #[test]
    fn gn1_ac_15_1_vacuum_flight_matches_the_analytic_solution() {
        let mut g = gs27();
        g.cd = Table1D::constant(0.0);
        let p0 = DVec3::new(0.0, 0.0, 3_000.0);
        let (t, p, v) = at_range(&g, p0, 1_000.0, 1.0 / 60.0);
        let t_exact = 1_000.0 / 1_025.0;
        assert!((t - t_exact).abs() < 1e-9, "{t}");
        // Gravity at 3 000 m on the flat sea: g0 (R / (R + h))².
        let r = flight::physical::EARTH_RADIUS_M;
        let g_h = G0 * (r / (r + 3_000.0)) * (r / (r + 3_000.0));
        let drop = p0.z - p.z;
        let drop_exact = 0.5 * g_h * t_exact * t_exact;
        assert!(
            ((drop - drop_exact) / drop_exact).abs() < 0.005,
            "{drop} vs {drop_exact}"
        );
        assert!(((v.x - 1_025.0) / 1_025.0).abs() < 0.005, "{v:?}");
        assert!(
            ((-v.z - g_h * t_exact) / (g_h * t_exact)).abs() < 0.005,
            "{v:?}"
        );
    }

    #[test]
    fn gn1_ac_15_1_drag_flight_matches_a_ten_times_finer_integration() {
        let g = gs27();
        let atmo = Atmosphere::new();
        let fly = |steps_per_s: u32| {
            let dt = 1.0 / f64::from(steps_per_s);
            let mut r = Round {
                pos: DVec3::new(0.0, 0.0, 3_000.0),
                vel: DVec3::new(1_025.0 + 230.0, 0.0, 0.0),
                age_s: 0.0,
            };
            for i in 0..steps_per_s {
                step_round(&mut r, &g, &FlatSea, &atmo, f64::from(i) * dt, dt);
            }
            r
        };
        let (game, fine) = (fly(60), fly(600));
        let travelled = (fine.pos - DVec3::new(0.0, 0.0, 3_000.0)).length();
        let err = (game.pos - fine.pos).length();
        assert!(err < 0.01 * travelled, "{err} m over {travelled} m");
        // The drag is real: the round has lost over 10 % of its speed in the second.
        assert!(fine.vel.length() < 0.9 * 1_255.0, "{:?}", fine.vel);
        assert!((fine.age_s - 1.0).abs() < 1e-9);
    }

    #[test]
    fn a_round_expires_by_age_speed_or_the_ground() {
        let g = gs27();
        let r = Round {
            pos: DVec3::new(0.0, 0.0, 100.0),
            vel: DVec3::new(800.0, 0.0, 0.0),
            age_s: 1.0,
        };
        assert!(!round_expired(&r, &g, &FlatSea));
        assert!(round_expired(&Round { age_s: 3.01, ..r }, &g, &FlatSea));
        assert!(round_expired(
            &Round {
                vel: DVec3::new(299.0, 0.0, 0.0),
                ..r
            },
            &g,
            &FlatSea
        ));
        assert!(round_expired(
            &Round {
                pos: DVec3::new(0.0, 0.0, -0.1),
                ..r
            },
            &g,
            &FlatSea
        ));
    }

    #[test]
    fn spawning_worked_example() {
        // Harmonised 10 mrad up: the axis is (cos 0.01, 0, −sin 0.01).
        let axis = gun_axis_body(0.01);
        assert!(
            (axis - DVec3::new(0.999_950_000_416_665_3, 0.0, -0.009_999_833_334_166_664)).length()
                < 1e-15
        );
        // Level, nose east (body x to BF x, body y to −y, body z to −z), 200 m/s east, pitching up
        // at 0.1 rad/s with the muzzle 3 m ahead: the muzzle moves ω × r = (0, 0.1, 0) × (3, 0, 0)
        // = (0, 0, −0.3) in body axes (up), so +0.3 m/s up in BF.
        let att = DQuat::from_axis_angle(DVec3::X, core::f64::consts::PI);
        let g = gs27();
        let r = spawn_round(
            &g,
            DVec3::new(10.0, 20.0, 1_000.0),
            DVec3::new(200.0, 0.0, 0.0),
            att,
            DVec3::new(0.0, 0.1, 0.0),
            DVec3::new(3.0, 0.0, 0.0),
            DVec3::X,
        );
        assert!(
            (r.pos - DVec3::new(13.0, 20.0, 1_000.0)).length() < 1e-12,
            "{:?}",
            r.pos
        );
        assert!(
            (r.vel - DVec3::new(1_225.0, 0.0, 0.3)).length() < 1e-9,
            "{:?}",
            r.vel
        );
        assert_eq!(r.age_s, 0.0);
    }

    #[test]
    fn dispersion_is_gaussian_with_sigma_on_each_axis() {
        // No offset for zero normals or zero spread.
        assert_eq!(dispersed_dir_body(DVec3::X, 0.5e-3, [0.0, 0.0]), DVec3::X);
        assert_eq!(dispersed_dir_body(DVec3::X, 0.0, [1.0, -2.0]), DVec3::X);
        // Worked example: n = (2, -1) with σ = 0.5 mrad moves the axis 1 mrad along e1 (left, -y)
        // and 0.5 mrad along -e2 (down, +z).
        let d = dispersed_dir_body(DVec3::X, 0.5e-3, [2.0, -1.0]);
        assert!(
            (d.y / d.x + 1e-3).abs() < 1e-15 && (d.z / d.x - 0.5e-3).abs() < 1e-15,
            "{d:?}"
        );
        // Over many draws: each axis has standard deviation σ, and the share of rounds inside a
        // circle of radius σ √(2 ln 2) is a half (the Rayleigh median).
        let mut state = 99u64;
        let mut uniform = || {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            ((state >> 11) as f64 + 0.5) / (1u64 << 53) as f64
        };
        let sigma = 0.5e-3;
        let median = sigma * dmath::sqrt(2.0 * dmath::ln(2.0));
        let (n_total, mut inner, mut sum_y2, mut sum_z2) = (20_000, 0, 0.0, 0.0);
        for _ in 0..n_total {
            let (u1, u2) = (uniform(), uniform());
            let rad = dmath::sqrt(-2.0 * dmath::ln(u1));
            let ang = 2.0 * core::f64::consts::PI * u2;
            let n = [rad * dmath::cos(ang), rad * dmath::sin(ang)];
            let d = dispersed_dir_body(DVec3::X, sigma, n);
            let (y, z) = (d.y / d.x, d.z / d.x);
            sum_y2 += y * y;
            sum_z2 += z * z;
            inner += u32::from(dmath::hypot(y, z) < median);
        }
        let n = f64::from(n_total);
        for s2 in [sum_y2, sum_z2] {
            let sd = dmath::sqrt(s2 / n);
            assert!((sd / sigma - 1.0).abs() < 0.02, "{sd}");
        }
        assert!((f64::from(inner) / n - 0.5).abs() < 0.015, "{inner}");
    }
}
