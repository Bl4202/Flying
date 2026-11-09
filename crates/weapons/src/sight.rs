//! The director (predictor) gunsight (GDD 15.4.3, `AC-15.2`, `AC-54.2`, ADR-46).
//!
//! A virtual round fired now along the gun axis (no dispersion) is flown with the real ballistics
//! at 120 Hz. For a target at range `R` from the shooter moving at constant velocity `v_t`, the
//! point `q(t) = b(t) − v_t t` (the round's position `b(t)` less the target's travel) is where
//! the target must be *now* for a round to meet it at time `t`. The solution is the first `t*`
//! with `|q(t*) − p_s| = R`, and the aim point is `q(t*)`: put the target there and the rounds hit
//! it. The pipper is drawn from the eye towards the aim point, so it has no parallax.

use flight::atmosphere::Atmosphere;
use flight::env::FlightEnv;
use simmath::math::{DQuat, DVec3};

use crate::ballistics::{GunBallistics, Round, gun_axis_body, rk2_substep, spawn_velocity};

/// Integration step of the virtual round, s. // ALGO: ADR-46, half a 60 Hz tick
pub const SIGHT_STEP_S: f64 = 1.0 / 120.0;

/// The shooter: its reference point, velocity, attitude and body rates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shooter {
    /// Reference point, BF, m.
    pub ref_bf: DVec3,
    /// Velocity of the reference point, BF, m/s.
    pub vel_bf: DVec3,
    /// Attitude, body to BF.
    pub att: DQuat,
    /// Body rates, rad/s.
    pub omega_body: DVec3,
}

/// Where the gun sits on the airframe.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GunMount {
    /// Muzzle position, body axes, m.
    pub muzzle_m: DVec3,
    /// Harmonisation (the barrel's angle above the body x axis), rad.
    pub harmonisation_rad: f64,
}

/// A point moving at constant velocity, BF.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mover {
    /// Position now, m.
    pub pos_bf: DVec3,
    /// Velocity, m/s.
    pub vel_bf: DVec3,
}

/// The gunsight's solution.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SightSolution {
    /// Where the target must be now for the rounds to hit it, BF, m.
    pub aim_point_bf: DVec3,
    /// The rounds' time of flight to it, s.
    pub tof_s: f64,
}

/// The director solution against a target, or `None` when the round's whole flight
/// (`max_age_s`) does not reach the target's range. Allocates nothing.
#[must_use]
pub fn gun_solution<E: FlightEnv>(
    g: &GunBallistics,
    mount: &GunMount,
    shooter: &Shooter,
    target: &Mover,
    env: &E,
    atmo: &Atmosphere,
    t_s: f64,
) -> Option<SightSolution> {
    gun_solution_with_step(g, mount, shooter, target, env, atmo, t_s, SIGHT_STEP_S)
}

/// [`gun_solution`] with the virtual round flown in steps of `h` seconds.
#[allow(clippy::too_many_arguments)] // ALGO: the public entry point bundles them
fn gun_solution_with_step<E: FlightEnv>(
    g: &GunBallistics,
    mount: &GunMount,
    shooter: &Shooter,
    target: &Mover,
    env: &E,
    atmo: &Atmosphere,
    t_s: f64,
    h: f64,
) -> Option<SightSolution> {
    let p_s = shooter.ref_bf;
    let range = (target.pos_bf - p_s).length();
    if !(range.is_finite() && h > 0.0) {
        return None;
    }
    let mut r = Round {
        pos: p_s + shooter.att.rotate(mount.muzzle_m),
        vel: spawn_velocity(
            shooter.vel_bf,
            shooter.att,
            shooter.omega_body,
            mount.muzzle_m,
            gun_axis_body(mount.harmonisation_rad),
            g.muzzle_mps,
        ),
        age_s: 0.0,
    };
    let q_at = |r: &Round| r.pos - target.vel_bf * r.age_s;
    let mut q_prev = q_at(&r);
    let mut d_prev = (q_prev - p_s).length();
    if d_prev >= range {
        return Some(SightSolution {
            aim_point_bf: q_prev,
            tof_s: 0.0,
        });
    }
    let steps = (g.max_age_s / h).ceil() as u32;
    for _ in 0..steps {
        let age = r.age_s;
        rk2_substep(&mut r, g, env, atmo, t_s + age, h);
        r.age_s = age + h;
        let q = q_at(&r);
        let d = (q - p_s).length();
        if d >= range {
            let f = (range - d_prev) / (d - d_prev);
            return Some(SightSolution {
                aim_point_bf: q_prev.lerp(q, f),
                tof_s: age + f * h,
            });
        }
        if !d.is_finite() {
            return None;
        }
        (q_prev, d_prev) = (q, d);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ballistics::{round_expired, step_round};
    use crate::test_content;
    use flight::env::FlatSea;
    use simmath::dmath;

    fn gs27() -> GunBallistics {
        GunBallistics::from_def(test_content::gun()).expect("a gun")
    }

    fn mount() -> GunMount {
        GunMount {
            muzzle_m: DVec3::new(3.0, -0.6, 0.1),
            harmonisation_rad: 0.0,
        }
    }

    /// Level, nose east (body FRD: x east, y south, z down).
    fn east() -> DQuat {
        DQuat::from_axis_angle(DVec3::X, core::f64::consts::PI)
    }

    /// The closest a round fired along the gun axis comes to the target, flying both at 60 Hz.
    fn miss_m(g: &GunBallistics, shooter: &Shooter, target: &Mover) -> f64 {
        let atmo = Atmosphere::new();
        let m = mount();
        let mut r = Round {
            pos: shooter.ref_bf + shooter.att.rotate(m.muzzle_m),
            vel: spawn_velocity(
                shooter.vel_bf,
                shooter.att,
                shooter.omega_body,
                m.muzzle_m,
                gun_axis_body(0.0),
                g.muzzle_mps,
            ),
            age_s: 0.0,
        };
        let dt = 1.0 / 60.0;
        let mut best = f64::INFINITY;
        let mut t_pos = target.pos_bf;
        while !round_expired(&r, g, &FlatSea) {
            let (r0, p0) = (r.pos, t_pos);
            let age = r.age_s;
            step_round(&mut r, g, &FlatSea, &atmo, age, dt);
            t_pos += target.vel_bf * dt;
            best = best.min(crate::capsule::seg_seg_distance(
                r0 - p0,
                r.pos - t_pos,
                DVec3::ZERO,
                DVec3::ZERO,
            ));
        }
        best
    }

    #[test]
    fn a_target_on_the_aim_point_is_hit() {
        // Worked example: shooter east-bound at 4 000 m and 230 m/s; the target 600 m off the nose
        // crossing north at 250 m/s. Placing the target on the aim point puts it within 0.3 m of
        // a round's path (60 Hz flight; the solution steps at 120 Hz).
        let g = gs27();
        let shooter = Shooter {
            ref_bf: DVec3::new(0.0, 0.0, 4_000.0),
            vel_bf: DVec3::new(230.0, 0.0, 0.0),
            att: east(),
            omega_body: DVec3::ZERO,
        };
        let vel = DVec3::new(0.0, 250.0, 0.0);
        let first = Mover {
            pos_bf: DVec3::new(600.0, 0.0, 4_000.0),
            vel_bf: vel,
        };
        let atmo = Atmosphere::new();
        let s =
            gun_solution(&g, &mount(), &shooter, &first, &FlatSea, &atmo, 0.0).expect("in reach");
        assert!(s.tof_s > 0.4 && s.tof_s < 0.8, "{}", s.tof_s);
        assert!(((s.aim_point_bf - shooter.ref_bf).length() - 600.0).abs() < 0.01);
        // The aim point is south of the gun line: a target there now flies north into the rounds.
        assert!(s.aim_point_bf.y < -50.0, "{:?}", s.aim_point_bf);
        let on_pipper = Mover {
            pos_bf: s.aim_point_bf,
            vel_bf: vel,
        };
        let miss = miss_m(&g, &shooter, &on_pipper);
        assert!(miss < 0.3, "{miss}");
    }

    /// GN2's reference, built apart from the sight: the round as the gun fires it (`step_round`,
    /// the real ballistics) flown at ten times the sight's rate, and the range crossing of
    /// `b(t) − v_t t` found between its samples.
    fn reference_aim(g: &GunBallistics, shooter: &Shooter, target: &Mover) -> DVec3 {
        let atmo = Atmosphere::new();
        let m = mount();
        let axis = gun_axis_body(m.harmonisation_rad);
        let v0 = spawn_velocity(
            shooter.vel_bf,
            shooter.att,
            shooter.omega_body,
            m.muzzle_m,
            axis,
            g.muzzle_mps,
        );
        let mut r = Round {
            pos: shooter.ref_bf + shooter.att.rotate(m.muzzle_m),
            vel: v0,
            age_s: 0.0,
        };
        let range = (target.pos_bf - shooter.ref_bf).length();
        // Two sub-steps a call: each sub-step is a tenth of the sight's.
        let dt = 2.0 * SIGHT_STEP_S / 10.0;
        let mut q_prev = r.pos;
        loop {
            let age = r.age_s;
            step_round(&mut r, g, &FlatSea, &atmo, age, dt);
            let q = r.pos - target.vel_bf * r.age_s;
            let (d0, d1) = (
                (q_prev - shooter.ref_bf).length(),
                (q - shooter.ref_bf).length(),
            );
            if d1 >= range {
                return q_prev.lerp(q, (range - d0) / (d1 - d0));
            }
            assert!(r.age_s < g.max_age_s, "out of reach");
            q_prev = q;
        }
    }

    #[test]
    fn gn2_ac_15_2_the_solution_agrees_with_a_ten_times_finer_one() {
        // A crossing target at 600 m: crossing angles 30° to 150° (between the target's track and
        // the line of sight), speeds 150 to 300 m/s; ten geometries.
        let g = gs27();
        let atmo = Atmosphere::new();
        let shooter = Shooter {
            ref_bf: DVec3::new(0.0, 0.0, 4_000.0),
            vel_bf: DVec3::new(230.0, 0.0, 0.0),
            att: east(),
            omega_body: DVec3::new(0.02, 0.05, -0.03),
        };
        for i in 0..10 {
            let cross = (30.0 + f64::from(i) * 120.0 / 9.0).to_radians();
            let speed = 150.0 + f64::from(i % 4) * 50.0;
            let los = DVec3::new(dmath::cos(0.02), dmath::sin(0.02), 0.0);
            let track = DVec3::new(dmath::cos(cross), dmath::sin(cross), 0.0);
            let target = Mover {
                pos_bf: shooter.ref_bf + los * 600.0,
                vel_bf: track * speed,
            };
            let s = gun_solution(&g, &mount(), &shooter, &target, &FlatSea, &atmo, 0.0)
                .expect("in reach");
            let fine = reference_aim(&g, &shooter, &target);
            let a = (s.aim_point_bf - shooter.ref_bf).normalized_or(DVec3::X);
            let b = (fine - shooter.ref_bf).normalized_or(DVec3::X);
            let err = dmath::acos(a.dot(b).min(1.0));
            assert!(err < 1e-3, "case {i}: {err} rad");
            // And much tighter than the gate in practice.
            assert!(err < 5e-5, "case {i}: {err} rad");
        }
    }

    #[test]
    fn out_of_reach_and_degenerate_cases() {
        let g = gs27();
        let atmo = Atmosphere::new();
        let shooter = Shooter {
            ref_bf: DVec3::new(0.0, 0.0, 4_000.0),
            vel_bf: DVec3::new(230.0, 0.0, 0.0),
            att: east(),
            omega_body: DVec3::ZERO,
        };
        let far = Mover {
            pos_bf: DVec3::new(9_000.0, 0.0, 4_000.0),
            vel_bf: DVec3::ZERO,
        };
        assert_eq!(
            gun_solution(&g, &mount(), &shooter, &far, &FlatSea, &atmo, 0.0),
            None
        );
        let nan = Mover {
            pos_bf: DVec3::new(f64::NAN, 0.0, 0.0),
            vel_bf: DVec3::ZERO,
        };
        assert_eq!(
            gun_solution(&g, &mount(), &shooter, &nan, &FlatSea, &atmo, 0.0),
            None
        );
        // Closer than the muzzle: the muzzle itself, at zero time of flight.
        let near = Mover {
            pos_bf: DVec3::new(1.0, 0.0, 4_000.0),
            vel_bf: DVec3::ZERO,
        };
        let s =
            gun_solution(&g, &mount(), &shooter, &near, &FlatSea, &atmo, 0.0).expect("a solution");
        assert_eq!(s.tof_s, 0.0);
        assert!(
            (s.aim_point_bf - (shooter.ref_bf + east().rotate(mount().muzzle_m))).length() < 1e-12
        );
    }
}
