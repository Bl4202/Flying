//! GN3 (`AC-54.2`): fire from the pipper at a constant-velocity Kestrel with the GS-27's
//! dispersion on, and count the hits.
//!
//! The shooter holds its aim so the target sits exactly on the director solution (no pilot error):
//! its attitude is turned until the aim point coincides with the target. Then 100 rounds leave the
//! gun at that instant, each with its own dispersion draw, and fly at 60 Hz while the target flies
//! on; each tick a round's path relative to the target is swept against the target's five
//! capsules. Ranges 300, 600 and 1 000 m; aspects 0°, 45°, 90° and 135° off the target's tail.

use std::fmt::Write as _;
use std::sync::OnceLock;

use content::{AircraftDef, Registry};
use flight::atmosphere::Atmosphere;
use flight::env::FlatSea;
use simmath::dmath;
use simmath::math::{DQuat, DVec3};
use weapons::ballistics::{
    GunBallistics, dispersed_dir_body, gun_axis_body, round_expired, spawn_round, step_round,
};
use weapons::capsule::{Capsule, relative_segment, segment_hits_capsules};
use weapons::sight::{GunMount, Mover, Shooter, gun_solution};

const DT: f64 = 1.0 / 60.0;
const ALT_M: f64 = 4_000.0;
const TARGET_TAS: f64 = 200.0;
const SHOOTER_TAS: f64 = 230.0;

fn registry() -> &'static Registry {
    static PACK: OnceLock<Registry> = OnceLock::new();
    PACK.get_or_init(|| {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content/core");
        let baked = content_bake::bake::bake_dir(&dir)
            .unwrap_or_else(|d| panic!("the core pack bakes: {d:?}"));
        Registry::from_cpack(&baked.cpack).expect("the baked pack reads back")
    })
}

fn kestrel() -> &'static AircraftDef {
    let r = registry();
    r.aircraft(r.aircraft_handles().next().expect("an aircraft"))
        .expect("aircraft")
}

/// splitmix64, and standard normals by Box–Muller.
struct Dice(u64);

impl Dice {
    fn uniform(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        (((z ^ (z >> 31)) >> 11) as f64 + 0.5) / (1u64 << 53) as f64
    }

    fn normals(&mut self) -> [f64; 2] {
        let r = dmath::sqrt(-2.0 * dmath::ln(self.uniform()));
        let a = 2.0 * core::f64::consts::PI * self.uniform();
        [r * dmath::cos(a), r * dmath::sin(a)]
    }
}

/// The rotation taking unit vector `from` to unit vector `to`.
fn rotation_between(from: DVec3, to: DVec3) -> DQuat {
    let c = from.cross(to);
    DQuat::new(1.0 + from.dot(to), c.x, c.y, c.z).normalized()
}

/// Hits out of `shots` from `range` at `aspect_deg` off the target's tail.
fn hits(range: f64, aspect_deg: f64, shots: u32, seed: u64) -> u32 {
    let def = kestrel();
    let install = def.gun.as_ref().expect("a gun");
    let g = GunBallistics::from_def(registry().weapon(install.weapon).expect("the gun"))
        .expect("ballistics");
    let caps: Vec<Capsule> = def
        .damage
        .as_ref()
        .expect("damage data")
        .capsules
        .iter()
        .map(Capsule::from_def)
        .collect();
    let m = install.muzzle_m;
    let mount = GunMount {
        muzzle_m: DVec3::new(f64::from(m[0]), f64::from(m[1]), f64::from(m[2])),
        harmonisation_rad: f64::from(install.harmonisation_mrad) * 1e-3,
    };
    let atmo = Atmosphere::new();

    // The target: level, northbound.
    let att_t = DQuat::from_axis_angle(DVec3::Z, core::f64::consts::FRAC_PI_2)
        * DQuat::from_axis_angle(DVec3::X, core::f64::consts::PI);
    let nose = DVec3::Y;
    let right = DVec3::X;
    let p_t = DVec3::new(0.0, 0.0, ALT_M);
    let v_t = nose * TARGET_TAS;
    // The shooter `range` from it, `aspect` off its tail, flying at it.
    let a = aspect_deg.to_radians();
    let p_s = p_t + (nose * -dmath::cos(a) + right * dmath::sin(a)) * range;
    let los = (p_t - p_s).normalized_or(DVec3::Y);
    let level =
        rotation_between(DVec3::X, los) * DQuat::from_axis_angle(DVec3::X, core::f64::consts::PI);
    let mut shooter = Shooter {
        ref_bf: p_s,
        vel_bf: los * SHOOTER_TAS,
        att: level,
        omega_body: DVec3::ZERO,
    };
    let target = Mover {
        pos_bf: p_t,
        vel_bf: v_t,
    };
    // Turn the shooter until the target sits on the pipper.
    for _ in 0..20 {
        let s =
            gun_solution(&g, &mount, &shooter, &target, &FlatSea, &atmo, 0.0).expect("in reach");
        let to_aim = (s.aim_point_bf - p_s).normalized_or(los);
        shooter.att = (rotation_between(to_aim, los) * shooter.att).normalized();
    }
    let s = gun_solution(&g, &mount, &shooter, &target, &FlatSea, &atmo, 0.0).expect("in reach");
    assert!(
        (s.aim_point_bf - p_t).length() < 0.01,
        "{range} m {aspect_deg}°: aim off by {:?}",
        s.aim_point_bf - p_t
    );

    let mut dice = Dice(seed);
    let axis = gun_axis_body(mount.harmonisation_rad);
    let mut count = 0;
    for _ in 0..shots {
        let dir = dispersed_dir_body(axis, g.dispersion_rad, dice.normals());
        let mut r = spawn_round(
            &g,
            shooter.ref_bf,
            shooter.vel_bf,
            shooter.att,
            shooter.omega_body,
            mount.muzzle_m,
            dir,
        );
        let mut pos_t = p_t;
        let mut t = 0.0;
        while !round_expired(&r, &g, &FlatSea) {
            let (r0, t0) = (r.pos, pos_t);
            step_round(&mut r, &g, &FlatSea, &atmo, t, DT);
            pos_t += v_t * DT;
            t += DT;
            let (s0, s1) = relative_segment(r0, r.pos, t0, pos_t, att_t);
            if segment_hits_capsules(s0, s1, &caps).is_some() {
                count += 1;
                break;
            }
            // Long past the target: stop.
            if (r.pos - pos_t).dot(r.vel - v_t) > 0.0 && (r.pos - pos_t).length() > 50.0 {
                break;
            }
        }
    }
    count
}

#[test]
fn gn3_ac_54_2_firing_from_the_pipper_hits_nine_times_in_ten() {
    let mut table = String::new();
    let mut worst = (u32::MAX, 0.0, 0.0);
    for (i, range) in [300.0, 600.0, 1_000.0].into_iter().enumerate() {
        for (j, aspect) in [0.0, 45.0, 90.0, 135.0].into_iter().enumerate() {
            let n = hits(range, aspect, 100, 1_000 + (i * 4 + j) as u64);
            writeln!(table, "{range:>6} m {aspect:>5}°: {n:>3}/100").expect("a String takes it");
            if n < worst.0 {
                worst = (n, range, aspect);
            }
        }
    }
    println!("{table}");
    assert!(
        worst.0 >= 90,
        "fewest hits {} at {} m, {}°\n{table}",
        worst.0,
        worst.1,
        worst.2
    );
}
