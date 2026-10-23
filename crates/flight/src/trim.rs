//! Trim: the controls and attitude that make a flight condition steady (GDD 11.11, `AC-11.1`).
//!
//! A [`TrimPoint`] describes a condition by its wind-axis flight path (speed, flight-path angle,
//! heading, wind-axis bank), the flow angles, the turn and pitch rates, the surfaces and a
//! continuous throttle `tau` (0 idle to 1 military, 1 to 2 the afterburner's level). A
//! [`TrimProblem`] picks which of those the solver may change and which accelerations it drives
//! to zero; the rest stay as given. The solver is a damped Newton iteration on a central-difference
//! Jacobian with a backtracking line search, on fixed-size arrays: it allocates nothing and runs
//! the same on every target, so a respawn can trim inside a tick.
//!
//! Residuals are the accelerations of the steady condition in body axes: the rate of the body
//! velocity `F/m + g - w x v` (zero when the velocity turns with the body, as in a steady turn
//! or the bottom of a pull-up) and the angular acceleration.

use simmath::math::{DQuat, DVec3};

use crate::actuators::Surfaces;
use crate::aircraft::{AircraftModel, AircraftState, Commands};
use crate::body::RigidState;
use crate::engine::{EngineCommand, EngineModel, EngineState, MAX_ENGINES};
use crate::env::FlightEnv;
use crate::frames::{Euler, attitude};
use crate::gear::{GroundState, WheelCommands};
use crate::mass::FuelLoad;

/// Most variables (and residuals) in a problem.
pub const MAX_VARS: usize = 8;
/// Newton iterations before giving up. // ALGO: plan WP6
pub const MAX_ITERATIONS: u32 = 50;
/// Converged when every residual is below this (m/s^2 or rad/s^2). // ALGO: plan WP6
pub const TOLERANCE: f64 = 1e-6;
/// Line-search halvings before an iteration gives up. // ALGO
const MAX_HALVINGS: u32 = 12;

/// A trim variable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Var {
    /// Angle of attack.
    Alpha,
    /// Sideslip.
    Beta,
    /// Wind-axis bank.
    Bank,
    /// Turn rate (heading rate).
    TurnRate,
    /// Flight-path angle.
    Gamma,
    /// True airspeed.
    Speed,
    /// Elevator.
    Elevator,
    /// Aileron.
    Aileron,
    /// Rudder.
    Rudder,
    /// Throttle `tau`.
    Throttle,
}

/// A residual: one acceleration that a steady condition must not have.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Residual {
    /// Along body x, m/s^2.
    AxialAccel,
    /// Along body y, m/s^2.
    SideAccel,
    /// Along body z, m/s^2.
    NormalAccel,
    /// Roll acceleration, rad/s^2.
    RollAccel,
    /// Pitch acceleration, rad/s^2.
    PitchAccel,
    /// Yaw acceleration, rad/s^2.
    YawAccel,
    /// Side force per unit mass, m/s^2 (zero: a coordinated, ball-centred turn).
    SideForce,
}

/// Which variables are free and which residuals must vanish (as many of each).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TrimProblem {
    vars: [Var; MAX_VARS],
    residuals: [Residual; MAX_VARS],
    n: usize,
}

const LONGITUDINAL: [Residual; 3] = [
    Residual::AxialAccel,
    Residual::NormalAccel,
    Residual::PitchAccel,
];
const ALL_AXES: [Residual; 7] = [
    Residual::AxialAccel,
    Residual::SideAccel,
    Residual::NormalAccel,
    Residual::RollAccel,
    Residual::PitchAccel,
    Residual::YawAccel,
    Residual::SideForce,
];

impl TrimProblem {
    /// A problem from variables and residuals.
    ///
    /// # Panics
    /// If the counts differ or exceed [`MAX_VARS`].
    #[must_use]
    pub const fn new(vars: &[Var], residuals: &[Residual]) -> Self {
        assert!(vars.len() == residuals.len() && vars.len() <= MAX_VARS);
        let mut v = [Var::Alpha; MAX_VARS];
        let mut r = [Residual::AxialAccel; MAX_VARS];
        let mut i = 0;
        while i < vars.len() {
            v[i] = vars[i];
            r[i] = residuals[i];
            i += 1;
        }
        Self {
            vars: v,
            residuals: r,
            n: vars.len(),
        }
    }

    /// Straight and level at a given speed: angle of attack, elevator and throttle.
    #[must_use]
    pub const fn level() -> Self {
        Self::new(&[Var::Alpha, Var::Elevator, Var::Throttle], &LONGITUDINAL)
    }

    /// Straight and level at a given throttle: angle of attack, elevator and the speed where
    /// thrust equals drag (the solver finds the one nearest the guess).
    #[must_use]
    pub const fn level_at_throttle() -> Self {
        Self::new(&[Var::Alpha, Var::Elevator, Var::Speed], &LONGITUDINAL)
    }

    /// Lift and pitching moment only, at a given speed, flight-path angle and pitch rate: angle
    /// of attack and elevator. The axial residual is then the excess (or shortfall) of thrust:
    /// specific excess power at 1 g, or the instantaneous state at the bottom of a pull-up.
    #[must_use]
    pub const fn lift_and_moment() -> Self {
        Self::new(
            &[Var::Alpha, Var::Elevator],
            &[Residual::NormalAccel, Residual::PitchAccel],
        )
    }

    /// A steady glide at a given speed and throttle: angle of attack, elevator and flight path.
    #[must_use]
    pub const fn glide() -> Self {
        Self::new(&[Var::Alpha, Var::Elevator, Var::Gamma], &LONGITUDINAL)
    }

    /// Pitching moment only, at a given angle of attack: the elevator.
    #[must_use]
    pub const fn elevator() -> Self {
        Self::new(&[Var::Elevator], &[Residual::PitchAccel])
    }

    /// A steady, coordinated, level turn at a given speed and throttle: the sustained turn rate,
    /// with angle of attack, sideslip, bank and the three surfaces.
    #[must_use]
    pub const fn turn_at_throttle() -> Self {
        Self::new(
            &[
                Var::Alpha,
                Var::Beta,
                Var::Bank,
                Var::TurnRate,
                Var::Elevator,
                Var::Aileron,
                Var::Rudder,
            ],
            &ALL_AXES,
        )
    }

    /// A steady, coordinated, level turn at a given speed and turn rate: the throttle, with
    /// angle of attack, sideslip, bank and the three surfaces.
    #[must_use]
    pub const fn turn() -> Self {
        Self::new(
            &[
                Var::Alpha,
                Var::Beta,
                Var::Bank,
                Var::Elevator,
                Var::Aileron,
                Var::Rudder,
                Var::Throttle,
            ],
            &ALL_AXES,
        )
    }

    /// The free variables.
    #[must_use]
    pub fn vars(&self) -> &[Var] {
        &self.vars[..self.n]
    }

    /// The residuals driven to zero.
    #[must_use]
    pub fn residuals(&self) -> &[Residual] {
        &self.residuals[..self.n]
    }
}

/// A flight condition, every value given or solved.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TrimPoint {
    /// True airspeed, m/s.
    pub speed_mps: f64,
    /// Flight-path angle, rad (positive climbing).
    pub gamma_rad: f64,
    /// Track heading, rad from north.
    pub heading_rad: f64,
    /// Wind-axis bank, rad (positive right wing down).
    pub bank_rad: f64,
    /// Angle of attack, rad.
    pub alpha_rad: f64,
    /// Sideslip, rad.
    pub beta_rad: f64,
    /// Turn rate (rate of change of heading), rad/s: positive turning right.
    pub turn_rate_rps: f64,
    /// Body pitch rate on top of the turn, rad/s (a pull-up).
    pub pitch_rate_rps: f64,
    /// Elevator, rad.
    pub elevator_rad: f64,
    /// Aileron, rad.
    pub aileron_rad: f64,
    /// Rudder, rad.
    pub rudder_rad: f64,
    /// Throttle `tau`: 0 idle to 1 military, then 1 to 2 the afterburner level.
    pub throttle: f64,
}

/// What stays fixed while trimming.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TrimSetup {
    /// Centre of gravity, BF.
    pub pos_bf: DVec3,
    /// Fuel per tank.
    pub fuel: FuelLoad,
    /// Flaps, airbrake and gear (the control surfaces in it are ignored).
    pub config: Surfaces,
}

/// A solve's outcome.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TrimResult {
    /// The condition reached (the solution if converged).
    pub point: TrimPoint,
    /// Every residual below [`TOLERANCE`].
    pub converged: bool,
    /// Newton iterations used.
    pub iterations: u32,
    /// The largest residual at `point`.
    pub residual: f64,
}

fn get(p: &TrimPoint, v: Var) -> f64 {
    match v {
        Var::Alpha => p.alpha_rad,
        Var::Beta => p.beta_rad,
        Var::Bank => p.bank_rad,
        Var::TurnRate => p.turn_rate_rps,
        Var::Gamma => p.gamma_rad,
        Var::Speed => p.speed_mps,
        Var::Elevator => p.elevator_rad,
        Var::Aileron => p.aileron_rad,
        Var::Rudder => p.rudder_rad,
        Var::Throttle => p.throttle,
    }
}

fn set(p: &mut TrimPoint, v: Var, x: f64) {
    match v {
        Var::Alpha => p.alpha_rad = x,
        Var::Beta => p.beta_rad = x,
        Var::Bank => p.bank_rad = x,
        Var::TurnRate => p.turn_rate_rps = x,
        Var::Gamma => p.gamma_rad = x,
        Var::Speed => p.speed_mps = x,
        Var::Elevator => p.elevator_rad = x,
        Var::Aileron => p.aileron_rad = x,
        Var::Rudder => p.rudder_rad = x,
        Var::Throttle => p.throttle = x,
    }
}

/// Finite-difference step for each variable. // ALGO: plan WP6
fn step_of(v: Var) -> f64 {
    match v {
        Var::Speed => 1e-4,
        Var::Throttle => 1e-5,
        Var::TurnRate => 1e-7,
        _ => 1e-6,
    }
}

/// Where each variable may go: the surfaces' travel, the throttle's range, sane flow angles.
fn bounds(model: &AircraftModel, v: Var) -> (f64, f64) {
    let a = &model.actuators;
    match v {
        Var::Alpha => (-0.6, 1.0),
        Var::Beta => (-0.5, 0.5),
        Var::Bank | Var::TurnRate | Var::Gamma => (-1.5, 1.5),
        Var::Speed => (1.0, 2500.0),
        Var::Elevator => (a.elevator.min, a.elevator.max),
        Var::Aileron => (a.aileron.min, a.aileron.max),
        Var::Rudder => (a.rudder.min, a.rudder.max),
        Var::Throttle => (0.0, max_throttle(model)),
    }
}

fn max_throttle(model: &AircraftModel) -> f64 {
    if model.ab_stages() > 0 { 2.0 } else { 1.0 }
}

/// The engines running steadily at throttle `tau`.
#[must_use]
pub fn engine_states(model: &AircraftModel, tau: f64) -> [EngineState; MAX_ENGINES] {
    let mut out = [EngineState::running_at(0.0); MAX_ENGINES];
    for (slot, e) in out.iter_mut().zip(model.engines()) {
        *slot = if tau <= 1.0 {
            e.model.steady(EngineCommand {
                dry: tau.max(0.0),
                ab_stage: 0,
            })
        } else {
            let mut s = e.model.steady(EngineCommand::MILITARY);
            s.ab_level = (tau - 1.0).min(1.0);
            s.ab_lit = true;
            s
        };
    }
    out
}

/// The engine command that holds throttle `tau`: dry power directly, the afterburner at the
/// stage that covers the level.
#[must_use]
pub fn engine_command(model: &AircraftModel, tau: f64) -> EngineCommand {
    if tau <= 1.0 {
        return EngineCommand {
            dry: tau.max(0.0),
            ab_stage: 0,
        };
    }
    let n = f64::from(model.ab_stages());
    EngineCommand {
        dry: 1.0,
        ab_stage: ((tau - 1.0).min(1.0) * n).ceil().clamp(1.0, n.max(1.0)) as u8,
    }
}

/// `R_z(-beta) R_y(alpha)`: takes body components to wind-axis components.
fn body_to_wind(alpha: f64, beta: f64) -> DQuat {
    DQuat::from_axis_angle(DVec3::Z, -beta) * DQuat::from_axis_angle(DVec3::Y, alpha)
}

/// The aircraft state of a trim point: surfaces at their trim positions, engines steady.
#[must_use]
pub fn state_of<E: FlightEnv>(
    model: &AircraftModel,
    setup: &TrimSetup,
    p: &TrimPoint,
    env: &E,
) -> AircraftState {
    let up = env.up_bf(setup.pos_bf);
    let wind_axes = attitude(
        up,
        Euler {
            heading_rad: p.heading_rad,
            pitch_rad: p.gamma_rad,
            roll_rad: p.bank_rad,
        },
    );
    let att = (wind_axes * body_to_wind(p.alpha_rad, p.beta_rad)).normalized();
    let vel_bf = wind_axes.rotate(DVec3::X) * p.speed_mps + env.wind_bf(setup.pos_bf, 0.0);
    // Heading increases clockwise seen from above: a right turn rotates about -up.
    let omega_body =
        att.conjugate().rotate(up * -p.turn_rate_rps) + DVec3::new(0.0, p.pitch_rate_rps, 0.0);
    AircraftState {
        body: RigidState {
            pos_bf: setup.pos_bf,
            vel_bf,
            att,
            omega_body,
        },
        surfaces: Surfaces {
            elevator_rad: p.elevator_rad,
            aileron_rad: p.aileron_rad,
            rudder_rad: p.rudder_rad,
            ..setup.config
        },
        engines: engine_states(model, p.throttle),
        fuel: setup.fuel,
        trim_pitch_rad: p.elevator_rad,
        wheels: WheelCommands::NONE,
        ground: GroundState::AIRBORNE,
        mass_frozen: false,
        numerical_fault: false,
    }
}

/// The commands that hold a trim point.
#[must_use]
pub fn commands_of(model: &AircraftModel, setup: &TrimSetup, p: &TrimPoint) -> Commands {
    Commands {
        surfaces: Surfaces {
            elevator_rad: p.elevator_rad,
            aileron_rad: p.aileron_rad,
            rudder_rad: p.rudder_rad,
            ..setup.config
        },
        engine: engine_command(model, p.throttle),
        wheels: WheelCommands::NONE,
    }
}

/// Every residual of a condition, in [`Residual`] order.
#[must_use]
pub fn all_residuals<E: FlightEnv>(
    model: &AircraftModel,
    setup: &TrimSetup,
    p: &TrimPoint,
    env: &E,
) -> [f64; 7] {
    let s = state_of(model, setup, p, env);
    let (d, force_body) = model.derivative(&s, env, 0.0);
    let to_body = s.body.att.conjugate();
    let v_body = to_body.rotate(s.body.vel_bf);
    let lin = to_body.rotate(d.vel) - s.body.omega_body.cross(v_body);
    let mass = model.mass.props(&setup.fuel).mass_kg;
    [
        lin.x,
        lin.y,
        lin.z,
        d.omega.x,
        d.omega.y,
        d.omega.z,
        force_body.y / mass,
    ]
}

fn index(r: Residual) -> usize {
    match r {
        Residual::AxialAccel => 0,
        Residual::SideAccel => 1,
        Residual::NormalAccel => 2,
        Residual::RollAccel => 3,
        Residual::PitchAccel => 4,
        Residual::YawAccel => 5,
        Residual::SideForce => 6,
    }
}

fn residuals<E: FlightEnv>(
    model: &AircraftModel,
    problem: &TrimProblem,
    setup: &TrimSetup,
    p: &TrimPoint,
    env: &E,
) -> [f64; MAX_VARS] {
    let all = all_residuals(model, setup, p, env);
    let mut out = [0.0; MAX_VARS];
    for (o, r) in out.iter_mut().zip(problem.residuals()) {
        *o = all[index(*r)];
    }
    out
}

fn norm(r: &[f64; MAX_VARS], n: usize) -> f64 {
    r[..n].iter().map(|v| v * v).sum::<f64>().sqrt()
}

fn max_abs(r: &[f64; MAX_VARS], n: usize) -> f64 {
    r[..n].iter().fold(0.0, |m, v| m.max(v.abs()))
}

/// Solves `a x = b` by Gaussian elimination with partial pivoting; `None` if singular.
fn solve_linear(
    mut a: [[f64; MAX_VARS]; MAX_VARS],
    mut b: [f64; MAX_VARS],
    n: usize,
) -> Option<[f64; MAX_VARS]> {
    for col in 0..n {
        let mut pivot = col;
        for row in col + 1..n {
            if a[row][col].abs() > a[pivot][col].abs() {
                pivot = row;
            }
        }
        if a[pivot][col].abs() < 1e-14 || !a[pivot][col].is_finite() {
            return None;
        }
        a.swap(col, pivot);
        b.swap(col, pivot);
        let pivot_row = a[col];
        for row in col + 1..n {
            let f = a[row][col] / pivot_row[col];
            for (k, p) in pivot_row.iter().enumerate().take(n).skip(col) {
                a[row][k] -= f * p;
            }
            b[row] -= f * b[col];
        }
    }
    let mut x = [0.0; MAX_VARS];
    for row in (0..n).rev() {
        let mut s = b[row];
        for k in row + 1..n {
            s -= a[row][k] * x[k];
        }
        x[row] = s / a[row][row];
    }
    Some(x)
}

/// Solves a trim problem from a starting guess (which also supplies every fixed value).
#[must_use]
pub fn solve<E: FlightEnv>(
    model: &AircraftModel,
    problem: &TrimProblem,
    setup: &TrimSetup,
    guess: TrimPoint,
    env: &E,
) -> TrimResult {
    let n = problem.n;
    let vars = problem.vars();
    let clamp_all = |p: &mut TrimPoint| {
        for &v in vars {
            let (lo, hi) = bounds(model, v);
            set(p, v, get(p, v).clamp(lo, hi));
        }
    };
    let mut p = guess;
    clamp_all(&mut p);
    let mut r = residuals(model, problem, setup, &p, env);
    let mut iterations = 0;
    while iterations < MAX_ITERATIONS {
        if max_abs(&r, n) < TOLERANCE {
            break;
        }
        iterations += 1;
        // Central-difference Jacobian.
        let mut jac = [[0.0; MAX_VARS]; MAX_VARS];
        for (j, &v) in vars.iter().enumerate() {
            let h = step_of(v);
            let (mut hi, mut lo) = (p, p);
            set(&mut hi, v, get(&p, v) + h);
            set(&mut lo, v, get(&p, v) - h);
            let r_hi = residuals(model, problem, setup, &hi, env);
            let r_lo = residuals(model, problem, setup, &lo, env);
            for i in 0..n {
                jac[i][j] = (r_hi[i] - r_lo[i]) / (2.0 * h);
            }
        }
        let mut rhs = [0.0; MAX_VARS];
        for i in 0..n {
            rhs[i] = -r[i];
        }
        let Some(dx) = solve_linear(jac, rhs, n) else {
            break;
        };
        // Backtracking line search on the residual norm.
        let start = norm(&r, n);
        let mut lambda = 1.0;
        let mut accepted = false;
        for _ in 0..MAX_HALVINGS {
            let mut trial = p;
            for (j, &v) in vars.iter().enumerate() {
                set(&mut trial, v, get(&p, v) + lambda * dx[j]);
            }
            clamp_all(&mut trial);
            let r_trial = residuals(model, problem, setup, &trial, env);
            let n_trial = norm(&r_trial, n);
            if n_trial.is_finite() && n_trial < start * (1.0 - 1e-4 * lambda) {
                p = trial;
                r = r_trial;
                accepted = true;
                break;
            }
            lambda *= 0.5;
        }
        if !accepted {
            break;
        }
    }
    let residual = max_abs(&r, n);
    TrimResult {
        point: p,
        converged: residual < TOLERANCE,
        iterations,
        residual,
    }
}

/// A starting guess for straight and level flight at `speed_mps`: the angle of attack whose lift
/// carries the weight on the linear lift curve, the elevator that cancels the moment there, and
/// a throttle for the drag of the clean polar.
#[must_use]
pub fn guess_level<E: FlightEnv>(
    model: &AircraftModel,
    setup: &TrimSetup,
    speed_mps: f64,
    load_factor: f64,
    env: &E,
) -> TrimPoint {
    let h = env.height_msl_m(setup.pos_bf);
    let air = model.atmosphere().sample(h, env.day());
    let mach = speed_mps / air.a_mps;
    let qs = 0.5 * air.rho_kgpm3 * speed_mps * speed_mps * model.aero.wing_area_m2();
    let weight = model.mass.props(&setup.fuel).mass_kg * env.gravity_bf(setup.pos_bf).length();
    let cl = load_factor * weight / qs.max(1.0);
    // Keep the guess on the attached-flow side of both stalls, where Newton heads the right way.
    let peak = model.aero.alpha_peak_deg(mach).to_radians();
    let alpha = (cl / model.aero.cl_alpha_per_rad(mach)).clamp(-0.5 * peak, 0.8 * peak);
    let (_, cm0) = model.aero.base_coefficients(alpha, mach);
    let (_, cm_de) = model.aero.elevator_power_per_rad(mach);
    let elevator = if cm_de == 0.0 { 0.0 } else { -cm0 / cm_de };
    TrimPoint {
        speed_mps,
        alpha_rad: alpha,
        elevator_rad: elevator,
        throttle: 0.6,
        ..TrimPoint::default()
    }
}

#[cfg(test)]
mod tests;
