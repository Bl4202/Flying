//! AC-58.4, scaled to the aircraft (ADR-33 section 5, GDD 0.2.2): in Standard, with the cursor
//! `theta` degrees (10 to 120) away from the nose in a random direction, the nose is within 1
//! degree of it no later than `1.0 s + theta / omega_lim` and stays so for the next 0.5 s, where
//! `omega_lim` is the aircraft's own turn rate at the angle-of-attack limit (or the load limit) at
//! that speed and altitude. 500 runs, at least 95 % must make it, and none may stall or break the
//! load limit. The pilot holds the throttle full, as one asking for the fastest turn would.
//!
//! The same runs in Full assist are reported, not gated (ADR-33): Full commands a flight-path
//! rate of at most 10 degrees per second, so it cannot keep the time the Standard law keeps.

mod common;

use common::*;
use fcs::mouse_aim::{Aim, error_rad, target_body};
use fcs::{Assist, PilotInput};
use flight::controls::PilotControls;
use flight::physical::G0;
use simmath::dmath;
use simmath::math::DVec3;

struct XorShift(u64);

impl XorShift {
    fn next(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
}

const RUNS: usize = 500;
const MACH: f64 = 0.6;
const ALT_M: f64 = 5_000.0;
const MASS_KG: f64 = 10_000.0;
/// The nose must be this close, rad, ...
const WITHIN_RAD: f64 = 1.0 * core::f64::consts::PI / 180.0;
/// ... and stay so for this long, s, from the moment it first is (for good).
const HOLD_S: f64 = 0.5;
/// The throttle lever.
const THROTTLE: f64 = 1.0;
/// The fraction that has to make it.
const REQUIRED: f64 = 0.95;

/// The outcome of 500 runs.
struct Outcome {
    made_it: usize,
    worst_alpha_deg: f64,
    worst_n: f64,
    /// Made it / runs by angle (six bins of the range) and direction around the nose (right,
    /// down, left, up).
    bins: [[usize; 2]; 24],
}

fn fly(assist: Assist, seed: u64) -> Outcome {
    let (model, fcs) = kestrel();
    let mut rng = XorShift(seed);
    let mut out = Outcome {
        made_it: 0,
        worst_alpha_deg: 0.0,
        worst_n: 0.0,
        bins: [[0; 2]; 24],
    };
    for run in 0..RUNS {
        let mut rig =
            Rig::new(&model, &fcs, assist, MACH, ALT_M, MASS_KG).expect("the condition trims");
        rig.use_lever = true;
        let s0 = rig.sensed();
        // The turn rate the aircraft has here: the load factor at the angle-of-attack limit or
        // the load limit, whichever is less.
        let n_lim = n_at_alpha_limit(&model, s0.mach, s0.qbar_pa, MASS_KG).min(model.limits.n_max);
        let omega_lim = G0 * (n_lim * n_lim - 1.0).sqrt() / s0.v;
        // The cursor: theta from the nose, in a random direction around it.
        let theta = (10.0 + 110.0 * rng.next()).to_radians();
        let around = 2.0 * core::f64::consts::PI * rng.next();
        let heading = 2.0 * core::f64::consts::PI * rng.next();
        // Turn the aircraft to the random heading: it is trimmed heading north.
        let vel_body = rig.ac.body.att.conjugate().rotate(rig.ac.body.vel_bf);
        rig.ac.body.att = flight::frames::attitude(
            DVec3::Z,
            flight::frames::Euler {
                heading_rad: heading,
                pitch_rad: s0.theta,
                roll_rad: 0.0,
            },
        );
        rig.ac.body.vel_bf = rig.ac.body.att.rotate(vel_body);
        let nose = rig.ac.body.att.rotate(DVec3::X);
        let right = rig.ac.body.att.rotate(DVec3::Y);
        let down = rig.ac.body.att.rotate(DVec3::Z);
        let aim_v = nose * dmath::cos(theta)
            + (right * dmath::cos(around) + down * dmath::sin(around)) * dmath::sin(theta);
        let aim = Aim {
            azimuth_rad: dmath::atan2(aim_v.x, aim_v.y),
            elevation_rad: dmath::asin(aim_v.z.clamp(-1.0, 1.0)),
        };
        let t_max = 1.0 + theta / omega_lim;
        let input = PilotInput {
            controls: PilotControls {
                throttle: THROTTLE,
                ..PilotControls::default()
            },
            aim: Some(aim),
            ..PilotInput::default()
        };
        // The moment the nose first got within the angle and stayed for the hold time.
        let (mut held, mut arrived) = (0.0, None);
        let (mut alpha_max, mut n_max) = (0.0f64, 0.0f64);
        let ticks = ((t_max + HOLD_S + 0.5) / DT) as usize;
        for i in 0..ticks {
            let s = rig.step(&input);
            alpha_max = alpha_max.max(s.alpha.abs());
            n_max = n_max.max(s.n_w);
            let err = error_rad(target_body(rig.ac.body.att, DVec3::Z, aim));
            if err <= WITHIN_RAD {
                held += DT;
                arrived.get_or_insert((i + 1) as f64 * DT);
                if held >= HOLD_S {
                    break;
                }
            } else {
                held = 0.0;
                arrived = None;
            }
        }
        let name = format!(
            "run {run}: theta {:.0} deg around {:.0} deg",
            theta.to_degrees(),
            around.to_degrees()
        );
        assert_eq!(rig.faults, 0, "{name}");
        out.worst_alpha_deg = out.worst_alpha_deg.max(alpha_max.to_degrees());
        out.worst_n = out.worst_n.max(n_max);
        let in_time = held >= HOLD_S && arrived.is_some_and(|t| t <= t_max);
        let angle_bin = ((theta.to_degrees() - 10.0) / 110.0 * 6.0)
            .floor()
            .clamp(0.0, 5.0) as usize;
        let quarter = ((around / (2.0 * core::f64::consts::PI) * 4.0 + 0.5).floor() as i64)
            .rem_euclid(4) as usize;
        let bin = &mut out.bins[angle_bin * 4 + quarter];
        bin[0] += 1;
        if in_time {
            out.made_it += 1;
            bin[1] += 1;
        }
        if assist == Assist::Standard {
            // The limits hold on every run, whether it made it or not.
            assert!(
                alpha_max.to_degrees() <= model.limits.alpha_limit_rad.to_degrees() + 2.0,
                "{name}: alpha {:.1}",
                alpha_max.to_degrees()
            );
            assert!(n_max <= model.limits.n_max + 0.3, "{name}: n {n_max:.2}");
        }
    }
    out
}

fn report(label: &str, o: &Outcome) {
    println!(
        "{label}: {}/{RUNS} in time ({:.1} %), worst alpha {:.1} deg, worst n {:.2} g",
        o.made_it,
        100.0 * o.made_it as f64 / RUNS as f64,
        o.worst_alpha_deg,
        o.worst_n
    );
    for tb in 0..6 {
        let b = |q: usize| format!("{}/{}", o.bins[tb * 4 + q][1], o.bins[tb * 4 + q][0]);
        println!(
            "  theta {:>3.0}-{:>3.0}: right {} down {} left {} up {}",
            10.0 + 110.0 * tb as f64 / 6.0,
            10.0 + 110.0 * (tb + 1) as f64 / 6.0,
            b(0),
            b(1),
            b(2),
            b(3)
        );
    }
}

#[test]
fn ac_58_4_mouse_aim_gets_the_nose_there_in_the_time_the_aircraft_has() {
    let o = fly(Assist::Standard, 0x5EED_5EED_1234_5678);
    report("AC-58.4 Standard", &o);
    let rate = o.made_it as f64 / RUNS as f64;
    assert!(rate >= REQUIRED, "{:.1} % made it", 100.0 * rate);
}

/// The same criterion on other random draws: the result does not hang on the lucky ones.
#[test]
fn ac_58_4_does_not_depend_on_the_draw() {
    for seed in [1, 99, 12_345, 777_777] {
        let o = fly(Assist::Standard, seed);
        let rate = o.made_it as f64 / RUNS as f64;
        println!("AC-58.4 Standard, seed {seed}: {:.1} %", 100.0 * rate);
        assert!(
            rate >= REQUIRED,
            "seed {seed}: {:.1} % made it",
            100.0 * rate
        );
    }
}

/// Informational: Full assist on the same runs (not a gate, ADR-33 section 5). It must fly them
/// without a numerical fault.
#[test]
fn ac_58_4_in_full_assist_is_reported() {
    let o = fly(Assist::Full, 0x5EED_5EED_1234_5678);
    report("AC-58.4 Full (informational)", &o);
}
