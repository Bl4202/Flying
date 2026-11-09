//! The weapons crate's per-tick functions allocate nothing (GDD 61.4, ADR-46): the gunsight's
//! virtual round, firing and flying rounds, the swept hit tests, the seeker, the missile's guidance
//! and integration, the fuze and the fly-out predictor.
//!
//! It lives here because the counting allocator needs `unsafe`, which only this crate permits.
#![allow(unsafe_code)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::path::Path;

use content::{Registry, WeaponDef};
use flight::atmosphere::Atmosphere;
use flight::env::SphereSea;
use simmath::math::{DQuat, DVec3};
use weapons::Round;
use weapons::ballistics::{
    GunBallistics, dispersed_dir_body, gun_axis_body, round_expired, spawn_round, step_round,
};
use weapons::capsule::{Capsule, capsules_distance, relative_segment, segment_hits_capsules};
use weapons::fuze::{damage_energy, fuze_armed, fuze_check};
use weapons::ir::{IrSignature, aspect_angle_rad, intensity};
use weapons::missile::{Missile, MissileParams, guide, launch, missile_expiry, step_missile};
use weapons::predict::fly_out;
use weapons::seeker::{SeekerState, TargetView, seeker_step};
use weapons::sight::{GunMount, Mover, Shooter, gun_solution};

thread_local! {
    static ALLOCATIONS: Cell<u64> = const { Cell::new(0) };
}

struct Counting;

// SAFETY: defers to `System` for every operation; the counter is a plain thread-local `Cell`
// with a const initialiser, so counting never allocates.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.with(|n| n.set(n.get() + 1));
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.with(|n| n.set(n.get() + 1));
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        ALLOCATIONS.with(|n| n.set(n.get() + 1));
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

fn allocations() -> u64 {
    ALLOCATIONS.with(Cell::get)
}

const DT: f64 = 1.0 / 60.0;

/// The Kestrel's gun, missile, capsules and signature, from the baked core pack.
fn kit() -> (GunBallistics, MissileParams, Vec<Capsule>, IrSignature) {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content/core");
    let baked = content_bake::bake::bake_dir(&dir).expect("the core pack bakes");
    let r = Registry::from_cpack(&baked.cpack).unwrap();
    let ac = r.aircraft(r.aircraft_handles().next().unwrap()).unwrap();
    let weapon = |h| -> &WeaponDef { r.weapon(h).unwrap() };
    let gun = GunBallistics::from_def(weapon(ac.gun.as_ref().unwrap().weapon)).unwrap();
    let missile = MissileParams::from_def(weapon(ac.rails[0].store), DT).unwrap();
    let caps = ac
        .damage
        .as_ref()
        .unwrap()
        .capsules
        .iter()
        .map(Capsule::from_def)
        .collect();
    (
        gun,
        missile,
        caps,
        IrSignature::from_def(ac.ir.as_ref().unwrap()),
    )
}

/// Everything one tick of the test touches.
struct Scene {
    gun: GunBallistics,
    params: MissileParams,
    caps: Vec<Capsule>,
    sig: IrSignature,
    world: SphereSea,
    atmo: Atmosphere,
    shooter: Shooter,
    mount: GunMount,
    target: TargetView,
    seeker: SeekerState,
    missile: Missile,
    rounds: [Round; 64],
    fuzed: u32,
    in_range: u32,
}

impl Scene {
    /// Shooter northbound at 4 000 m; target 1.5 km ahead, drifting east.
    fn new() -> Self {
        let (gun, params, caps, sig) = kit();
        let att = DQuat::from_axis_angle(DVec3::Z, core::f64::consts::FRAC_PI_2)
            * DQuat::from_axis_angle(DVec3::X, core::f64::consts::PI);
        let shooter = Shooter {
            ref_bf: DVec3::new(0.0, 0.0, 4_000.0),
            vel_bf: DVec3::new(0.0, 230.0, 0.0),
            att,
            omega_body: DVec3::new(0.0, 0.05, 0.0),
        };
        let seeker = SeekerState::searching();
        let missile = launch(&params, shooter.ref_bf, shooter.vel_bf, seeker);
        Self {
            gun,
            params,
            caps,
            sig,
            world: SphereSea::open(),
            atmo: Atmosphere::new(),
            shooter,
            mount: GunMount {
                muzzle_m: DVec3::new(3.0, -0.6, 0.1),
                harmonisation_rad: 0.0,
            },
            target: TargetView {
                pos_bf: DVec3::new(30.0, 1_500.0, 4_000.0),
                vel_bf: DVec3::new(10.0, 210.0, 0.0),
                att,
                intensity: 0.5,
            },
            seeker,
            missile,
            rounds: [Round::default(); 64],
            fuzed: 0,
            in_range: 0,
        }
    }

    /// The gunsight, one more round, and every round in flight against the target's capsules.
    fn guns(&mut self, k: u32, t: f64, n: [f64; 2], t_next: DVec3) {
        let (g, sh) = (&self.gun, &self.shooter);
        let target = Mover {
            pos_bf: self.target.pos_bf,
            vel_bf: self.target.vel_bf,
        };
        let aim = gun_solution(g, &self.mount, sh, &target, &self.world, &self.atmo, t);
        assert!(aim.is_some(), "tick {k}");
        let dir = dispersed_dir_body(gun_axis_body(0.0), g.dispersion_rad, n);
        let m = self.mount.muzzle_m;
        self.rounds[(k % 64) as usize] =
            spawn_round(g, sh.ref_bf, sh.vel_bf, sh.att, sh.omega_body, m, dir);
        for r in &mut self.rounds {
            let p0 = r.pos;
            step_round(r, g, &self.world, &self.atmo, t, DT);
            let (s0, s1) = relative_segment(p0, r.pos, target.pos_bf, t_next, self.target.att);
            let _ = segment_hits_capsules(s0, s1, &self.caps);
            let _ = round_expired(r, g, &self.world);
        }
    }

    /// The seeker on the rail, the missile in flight and its fuze, and the predictor.
    fn missiles(&mut self, k: u32, t: f64, n: [f64; 2], t_next: DVec3) {
        let (p, sh) = (&self.params, &self.shooter);
        let tgt = &mut self.target;
        let aa = aspect_angle_rad(tgt.nose_bf(), tgt.pos_bf, sh.ref_bf);
        tgt.intensity = intensity(&self.sig, 1.0, 0.0, 0.7, aa);
        let nose = sh.att.rotate(DVec3::X);
        let _ = seeker_step(&mut self.seeker, &p.seeker, nose, sh.ref_bf, tgt, true);
        let m = &mut self.missile;
        let axis = m.vel.normalized_or(DVec3::X);
        let _ = seeker_step(&mut m.seeker, &p.seeker, axis, m.pos, tgt, true);
        let a_cmd = guide(m, p, tgt, n, &self.world, &self.atmo, t);
        let m_prev = m.pos;
        step_missile(m, p, a_cmd, &self.world, &self.atmo, t, DT);
        if fuze_armed(m, p)
            && let Some(d) = fuze_check(m_prev, m.pos, tgt.pos_bf, t_next, tgt.att, &self.caps, p)
        {
            self.fuzed += 1;
            let _ = damage_energy(p, d.miss_m);
        }
        let _ = missile_expiry(m, p, &self.world, true);
        if k.is_multiple_of(30) {
            let next = launch(p, sh.ref_bf, sh.vel_bf, self.seeker);
            let caps = &self.caps;
            self.in_range += u32::from(fly_out(p, next, tgt, caps, &self.world, &self.atmo, t, DT));
        }
    }

    fn tick(&mut self, k: u32) {
        let t = f64::from(k) * DT;
        let n = [f64::from(k % 7) * 0.3 - 0.9, f64::from(k % 5) * 0.4 - 0.8];
        let t_next = self.target.pos_bf + self.target.vel_bf * DT;
        self.guns(k, t, n, t_next);
        self.missiles(k, t, n, t_next);
        let (sh, tg) = (&self.shooter, &self.target);
        let _ = capsules_distance(&self.caps, sh.att, sh.ref_bf, &self.caps, tg.att, tg.pos_bf);
        self.shooter.ref_bf += self.shooter.vel_bf * DT;
        self.target.pos_bf = t_next;
    }
}

#[test]
fn the_weapons_tick_allocates_nothing() {
    let mut scene = Scene::new();
    let before = allocations();
    for k in 0..600 {
        scene.tick(k);
    }
    assert_eq!(allocations() - before, 0);
    // The run did what it set out to: the predictor said yes at least once.
    assert!(scene.in_range > 0, "never in range");
    let _ = scene.fuzed;
}
