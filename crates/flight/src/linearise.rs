//! Small-perturbation linearisation about a trimmed state and the classical modes (GDD C.5).
//! Informational in v0.1a: the conformance test does not depend on it.
//!
//! The longitudinal state is `(u, w, q, theta)` and the lateral one `(v, p, r, phi)`, with body
//! velocities in m/s, rates in rad/s and angles in rad. The matrices are central differences of
//! the full nonlinear model; their eigenvalues come from the characteristic polynomial
//! (Faddeev-LeVerrier) and a fixed number of Durand-Kerner iterations, so the result is
//! deterministic and needs no allocation.

use simmath::dmath;
use simmath::math::{DQuat, DVec3};

use crate::aircraft::{AircraftModel, AircraftState};
use crate::env::FlightEnv;
use crate::frames::euler;

/// A 4 x 4 matrix.
pub type Mat4 = [[f64; 4]; 4];

/// Perturbation sizes: m/s for velocities, rad/s for rates, rad for angles. // ALGO
const STEPS: [f64; 4] = [1e-3, 1e-3, 1e-5, 1e-5];

/// An oscillatory or first-order mode.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Mode {
    /// Real part of the eigenvalue, 1/s.
    pub re: f64,
    /// Imaginary part (zero for a real mode), rad/s.
    pub im: f64,
}

impl Mode {
    /// Natural frequency `|lambda|`, rad/s.
    #[must_use]
    pub fn omega_n_rps(&self) -> f64 {
        dmath::sqrt(self.re * self.re + self.im * self.im)
    }

    /// Damping ratio `-re / |lambda|`.
    #[must_use]
    pub fn zeta(&self) -> f64 {
        -self.re / self.omega_n_rps()
    }

    /// Time constant `-1 / re` of a real mode, s (negative when divergent).
    #[must_use]
    pub fn tau_s(&self) -> f64 {
        -1.0 / self.re
    }
}

/// The classical modes of a trimmed state. `None` where the roots did not take the usual form
/// (an overdamped short period, say).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Modes {
    /// Short period: the faster longitudinal oscillation.
    pub short_period: Option<Mode>,
    /// Phugoid: the slower longitudinal oscillation.
    pub phugoid: Option<Mode>,
    /// Dutch roll: the lateral oscillation.
    pub dutch_roll: Option<Mode>,
    /// Roll subsidence: the fastest real lateral root.
    pub roll: Option<Mode>,
    /// Spiral: the slowest real lateral root.
    pub spiral: Option<Mode>,
}

/// Which axes a perturbation acts on.
#[derive(Clone, Copy)]
enum Axis {
    Longitudinal,
    Lateral,
}

/// The state with perturbation `dx` applied to component `i` of the reduced state.
fn perturbed(s: &AircraftState, axis: Axis, i: usize, dx: f64) -> AircraftState {
    let mut p = *s;
    let att = s.body.att;
    let mut v_body = att.conjugate().rotate(s.body.vel_bf);
    match (axis, i) {
        (Axis::Longitudinal, 0) => v_body.x += dx,
        (Axis::Longitudinal, 1) => v_body.z += dx,
        (Axis::Longitudinal, 2) => p.body.omega_body.y += dx,
        (Axis::Longitudinal, _) => {
            p.body.att = (att * DQuat::from_axis_angle(DVec3::Y, dx)).normalized();
        }
        (Axis::Lateral, 0) => v_body.y += dx,
        (Axis::Lateral, 1) => p.body.omega_body.x += dx,
        (Axis::Lateral, 2) => p.body.omega_body.z += dx,
        (Axis::Lateral, _) => {
            p.body.att = (att * DQuat::from_axis_angle(DVec3::X, dx)).normalized();
        }
    }
    p.body.vel_bf = p.body.att.rotate(v_body);
    p
}

/// The reduced state derivative at `s`.
fn rates<E: FlightEnv>(model: &AircraftModel, s: &AircraftState, axis: Axis, env: &E) -> [f64; 4] {
    let (d, _) = model.derivative(s, env, 0.0);
    let to_body = s.body.att.conjugate();
    let v_body = to_body.rotate(s.body.vel_bf);
    let w = s.body.omega_body;
    let lin = to_body.rotate(d.vel) - w.cross(v_body);
    let e = euler(env.up_bf(s.body.pos_bf), s.body.att);
    let (sphi, cphi) = (dmath::sin(e.roll_rad), dmath::cos(e.roll_rad));
    match axis {
        Axis::Longitudinal => [lin.x, lin.z, d.omega.y, w.y * cphi - w.z * sphi],
        Axis::Lateral => {
            let tan_theta = dmath::tan(e.pitch_rad);
            [
                lin.y,
                d.omega.x,
                d.omega.z,
                w.x + (w.y * sphi + w.z * cphi) * tan_theta,
            ]
        }
    }
}

fn matrix<E: FlightEnv>(model: &AircraftModel, s: &AircraftState, axis: Axis, env: &E) -> Mat4 {
    let mut a = [[0.0; 4]; 4];
    for (j, h) in STEPS.iter().enumerate() {
        let hi = rates(model, &perturbed(s, axis, j, *h), axis, env);
        let lo = rates(model, &perturbed(s, axis, j, -*h), axis, env);
        for i in 0..4 {
            a[i][j] = (hi[i] - lo[i]) / (2.0 * h);
        }
    }
    a
}

/// The longitudinal system matrix about `s` (state `(u, w, q, theta)`), surfaces and engines
/// held.
#[must_use]
pub fn longitudinal<E: FlightEnv>(model: &AircraftModel, s: &AircraftState, env: &E) -> Mat4 {
    matrix(model, s, Axis::Longitudinal, env)
}

/// The lateral system matrix about `s` (state `(v, p, r, phi)`), surfaces and engines held.
#[must_use]
pub fn lateral<E: FlightEnv>(model: &AircraftModel, s: &AircraftState, env: &E) -> Mat4 {
    matrix(model, s, Axis::Lateral, env)
}

fn mat_mul(a: &Mat4, b: &Mat4) -> Mat4 {
    let mut c = [[0.0; 4]; 4];
    for i in 0..4 {
        for j in 0..4 {
            for (k, bk) in b.iter().enumerate() {
                c[i][j] += a[i][k] * bk[j];
            }
        }
    }
    c
}

/// Coefficients `c1..c4` of `lambda^4 + c1 lambda^3 + c2 lambda^2 + c3 lambda + c4`
/// (Faddeev-LeVerrier).
fn characteristic(a: &Mat4) -> [f64; 4] {
    let mut m = [[0.0; 4]; 4];
    let mut c = [0.0; 4];
    let mut prev = 1.0;
    for k in 1..=4 {
        // M_k = A M_{k-1} + c_{k-1} I, c_k = -tr(A M_k) / k.
        let mut next = mat_mul(a, &m);
        for (i, row) in next.iter_mut().enumerate() {
            row[i] += prev;
        }
        m = next;
        let am = mat_mul(a, &m);
        let trace = am[0][0] + am[1][1] + am[2][2] + am[3][3];
        prev = -trace / f64::from(k as u8);
        c[k - 1] = prev;
    }
    c
}

/// The four roots of a monic quartic, as `(re, im)`, by Durand-Kerner.
fn quartic_roots(c: [f64; 4]) -> [(f64, f64); 4] {
    let mul = |a: (f64, f64), b: (f64, f64)| (a.0 * b.0 - a.1 * b.1, a.0 * b.1 + a.1 * b.0);
    let div = |a: (f64, f64), b: (f64, f64)| {
        let d = b.0 * b.0 + b.1 * b.1;
        ((a.0 * b.0 + a.1 * b.1) / d, (a.1 * b.0 - a.0 * b.1) / d)
    };
    let poly = |z: (f64, f64)| {
        let mut p = (1.0, 0.0);
        for ck in c {
            p = mul(p, z);
            p.0 += ck;
        }
        p
    };
    // ALGO: the usual starting points, powers of 0.4 + 0.9i scaled to the roots' size.
    let scale = 1.0 + c.iter().fold(0.0_f64, |m, v| m.max(v.abs())).sqrt();
    let mut z = [(0.0, 0.0); 4];
    let mut w = (1.0, 0.0);
    for zi in &mut z {
        w = mul(w, (0.4, 0.9));
        *zi = (w.0 * scale, w.1 * scale);
    }
    for _ in 0..500 {
        for i in 0..4 {
            let mut den = (1.0, 0.0);
            for (j, zj) in z.iter().enumerate() {
                if j != i {
                    den = mul(den, (z[i].0 - zj.0, z[i].1 - zj.1));
                }
            }
            let step = div(poly(z[i]), den);
            z[i] = (z[i].0 - step.0, z[i].1 - step.1);
        }
    }
    z
}

/// Eigenvalues of a 4 x 4 matrix, as `(re, im)`.
#[must_use]
pub fn eigenvalues(a: &Mat4) -> [(f64, f64); 4] {
    quartic_roots(characteristic(a))
}

/// Splits roots into complex pairs (upper half-plane members, largest modulus first) and real
/// roots (most negative first). Roots with `|im|` below `1e-6 |lambda| + 1e-9` count as real.
fn classify(roots: [(f64, f64); 4]) -> ([Option<Mode>; 2], [Option<Mode>; 4]) {
    let mut pairs = [None; 2];
    let mut reals = [None; 4];
    let (mut np, mut nr) = (0, 0);
    for (re, im) in roots {
        let m = Mode { re, im: im.abs() };
        if im.abs() <= 1e-6 * m.omega_n_rps() + 1e-9 {
            reals[nr] = Some(Mode { re, im: 0.0 });
            nr += 1;
        } else if im > 0.0 && np < 2 {
            pairs[np] = Some(m);
            np += 1;
        }
    }
    if let [Some(a), Some(b)] = pairs
        && b.omega_n_rps() > a.omega_n_rps()
    {
        pairs = [Some(b), Some(a)];
    }
    reals[..nr].sort_by(|a, b| {
        let (a, b) = (a.map_or(0.0, |m| m.re), b.map_or(0.0, |m| m.re));
        a.total_cmp(&b)
    });
    (pairs, reals)
}

/// The classical modes about a trimmed state.
#[must_use]
pub fn modes<E: FlightEnv>(model: &AircraftModel, s: &AircraftState, env: &E) -> Modes {
    let (long_pairs, _) = classify(eigenvalues(&longitudinal(model, s, env)));
    let (lat_pairs, lat_reals) = classify(eigenvalues(&lateral(model, s, env)));
    let n_real = lat_reals.iter().filter(|m| m.is_some()).count();
    Modes {
        short_period: long_pairs[0],
        phugoid: long_pairs[1],
        dutch_roll: lat_pairs[0],
        roll: lat_reals[0],
        spiral: if n_real >= 2 {
            lat_reals[n_real - 1]
        } else {
            None
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actuators::Surfaces;
    use crate::env::SphereSea;
    use crate::test_content::registry;
    use crate::trim::{TrimProblem, TrimSetup, guess_level, solve, state_of};

    #[test]
    fn eigenvalues_of_a_known_matrix() {
        // Block diagonal: a pair at -1 +- 2i and real roots -3 and 0.5.
        let a = [
            [-1.0, 2.0, 0.0, 0.0],
            [-2.0, -1.0, 0.0, 0.0],
            [0.0, 0.0, -3.0, 0.0],
            [0.0, 0.0, 0.0, 0.5],
        ];
        let (pairs, reals) = classify(eigenvalues(&a));
        let p = pairs[0].unwrap();
        assert!(
            (p.re + 1.0).abs() < 1e-9 && (p.im - 2.0).abs() < 1e-9,
            "{p:?}"
        );
        assert!((reals[0].unwrap().re + 3.0).abs() < 1e-9);
        assert!((reals[1].unwrap().re - 0.5).abs() < 1e-9);
        assert!((p.omega_n_rps() - 5f64.sqrt()).abs() < 1e-9);
        assert!((p.zeta() - 1.0 / 5f64.sqrt()).abs() < 1e-9);
    }

    #[test]
    fn the_kestrels_modes_against_appendix_c5() {
        // C.5 computed the short period with the 2-state approximation, 10 000 kg, I_yy 76 000.
        // Where the data has C.5's derivatives (M <= 0.8) the full model agrees within 15 %;
        // the transonic and supersonic rows carry the larger static margin of the Mach schedule,
        // so they are printed for information only.
        let r = registry();
        let m = AircraftModel::from_registry(r, r.aircraft_handles().next().unwrap(), 1.0 / 60.0)
            .unwrap();
        let world = SphereSea::open();
        let cases = [
            (0.0, 0.5, 2.52, 0.68, true),
            (0.0, 0.9, 4.54, 0.68, false),
            (3000.0, 0.8, 3.23, 0.60, true),
            (6000.0, 0.9, 2.88, 0.53, false),
            (9000.0, 1.2, 3.02, 0.46, false),
        ];
        for (alt, mach, wn, zeta, gated) in cases {
            let setup = TrimSetup {
                pos_bf: DVec3::new(0.0, 0.0, alt),
                fuel: m.mass.load_for(1600.0),
                config: Surfaces::NEUTRAL,
            };
            let a = m
                .atmosphere()
                .sample(alt, crate::atmosphere::NonStandardDay::ISA)
                .a_mps;
            let guess = guess_level(&m, &setup, mach * a, 1.0, &world);
            let t = solve(&m, &TrimProblem::level(), &setup, guess, &world);
            assert!(t.converged, "{alt} m, M {mach}: {t:?}");
            let s = state_of(&m, &setup, &t.point, &world);
            let modes = modes(&m, &s, &world);
            let sp = modes.short_period.expect("an oscillatory short period");
            let dr = modes.dutch_roll.expect("a Dutch roll");
            println!(
                "{alt:5} m M {mach}: short period {:.2} rad/s zeta {:.2} (C.5 {wn}, {zeta}); \
                 Dutch roll {:.2} rad/s zeta {:.2}; roll tau {:.2} s; phugoid {:?}",
                sp.omega_n_rps(),
                sp.zeta(),
                dr.omega_n_rps(),
                dr.zeta(),
                modes.roll.map_or(f64::NAN, |r| r.tau_s()),
                modes.phugoid.map(|p| (p.omega_n_rps(), p.zeta())),
            );
            if gated {
                assert!(
                    (sp.omega_n_rps() / wn - 1.0).abs() < 0.15,
                    "{alt} m M {mach}: wn"
                );
                assert!(
                    (sp.zeta() / zeta - 1.0).abs() < 0.15,
                    "{alt} m M {mach}: zeta"
                );
            }
            assert!(dr.zeta() > 0.0, "a damped Dutch roll");
            assert!(modes.roll.is_some_and(|r| r.re < 0.0), "a stable roll mode");
        }
    }
}
