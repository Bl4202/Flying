//! The missile (GDD 15.5.2, 15.5.3, `AC-15.3`, ADR-46): a 3-DOF point mass with a constant-thrust
//! motor, Mach-dependent drag (boost and coast tables plus induced drag), a lateral acceleration
//! that follows the guidance command through a first-order autopilot, and true proportional
//! navigation with gravity compensation. Integrated with RK4 over two sub-steps per 60 Hz tick
//! (120 Hz), always.
//!
//! State `(p, v, m)`, time since launch `t`:
//! - `T = thrust` while `t < burn_s`, else 0; `ṁ = −propellant / burn_s` while burning.
//! - `q̄ = ½ ρ V²`, `V = |v − w|`, `M = V / a`; `C_N = m |a_lat| / (q̄ S)` (0 when `q̄ < 1`);
//!   `C_D = cd0(M) + k C_N²`; `D = q̄ S C_D`.
//! - `dv/dt = (T − D) / m · (v − w) / V + a_lat + g(p)`, `dp/dt = v`.
//!
//! The autopilot: over a sub-step `h` with the command `a_cmd` held, `a_lat` relaxes towards it as
//! `a_cmd + (a_lat − a_cmd) e^{−t/τ}`. The sub-step's RK4 holds `a_lat` at that curve's mean over
//! the sub-step (exact for a held command), and the sub-step ends with `a_lat` at the curve's end
//! value `a_lat + (a_cmd − a_lat)(1 − e^{−h/τ})`; both lose their component along `v̂` and are
//! held to the airframe's limit `|a_lat| ≤ min(g_max G0, q̄ S C_N,max / m)` (GDD 15.5.2) at the
//! sub-step's start, and each RK4 stage removes the held value's component along that stage's
//! velocity, so the lateral acceleration never pushes the missile along its path.

use content::WeaponDef;
use flight::atmosphere::Atmosphere;
use flight::env::FlightEnv;
use flight::physical::G0;
use flight::table::Table1D;
use simmath::dmath;
use simmath::math::DVec3;

use crate::orthonormal_pair;
use crate::seeker::{SeekerMode, SeekerParams, SeekerState, TargetView};

/// Times closer than this count as equal (burnout inside a sub-step), s. // ALGO: rounding of t
const TIME_EPS_S: f64 = 1e-9;
/// Dynamic pressure below which the normal-force coefficient is taken as 0, Pa. // ALGO: no 1/0
const MIN_QBAR_PA: f64 = 1.0;

/// What the launching aircraft must satisfy (GDD 15.6).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LaunchLimits {
    /// Lowest launcher true airspeed, m/s.
    pub min_speed_mps: f64,
    /// Largest launcher load factor, g.
    pub max_g: f64,
    /// Largest launcher angle of attack, rad.
    pub max_alpha_rad: f64,
    /// Time after launch before the fuze may arm, s.
    pub safe_sep_s: f64,
    /// Shortest range at which a launch is allowed, m.
    pub min_range_m: f64,
}

/// Why a launch is refused by the launch limits (the checks that need no lock or stores).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LaunchRefusal {
    /// The target is closer than `min_range_m`.
    TooClose,
    /// The launcher's load factor is beyond `max_g`.
    GLimit,
    /// The launcher's angle of attack is beyond `max_alpha_rad`.
    AlphaLimit,
    /// The launcher is slower than `min_speed_mps`.
    Slow,
}

impl LaunchLimits {
    /// The first limit a launch now would break, in the order range, g, angle of attack, speed;
    /// `None` if it may go. NaN inputs refuse.
    #[must_use]
    pub fn check(
        &self,
        range_m: f64,
        n_z: f64,
        alpha_rad: f64,
        tas_mps: f64,
    ) -> Option<LaunchRefusal> {
        if range_m.is_nan() || range_m < self.min_range_m {
            Some(LaunchRefusal::TooClose)
        } else if n_z.is_nan() || n_z.abs() > self.max_g {
            Some(LaunchRefusal::GLimit)
        } else if alpha_rad.is_nan() || alpha_rad.abs() > self.max_alpha_rad {
            Some(LaunchRefusal::AlphaLimit)
        } else if tas_mps.is_nan() || tas_mps < self.min_speed_mps {
            Some(LaunchRefusal::Slow)
        } else {
            None
        }
    }
}

/// A missile type's parameters, in SI units.
#[derive(Clone, Debug, PartialEq)]
pub struct MissileParams {
    /// Mass at launch, kg.
    pub mass_kg: f64,
    /// Motor thrust, N.
    pub thrust_n: f64,
    /// Burn time, s.
    pub burn_s: f64,
    /// Propellant mass, kg.
    pub propellant_kg: f64,
    /// Reference area, m².
    pub s_ref_m2: f64,
    /// Zero-lift drag coefficient against Mach while burning.
    pub cd0_boost: Table1D,
    /// Zero-lift drag coefficient against Mach after burnout.
    pub cd0_coast: Table1D,
    /// Largest normal-force coefficient.
    pub cn_max: f64,
    /// Induced-drag factor.
    pub k_induced: f64,
    /// Autopilot time constant, s.
    pub tau_s: f64,
    /// Structural lateral limit, g.
    pub g_max: f64,
    /// Speed below which it cannot manoeuvre, m/s.
    pub g_min_speed_mps: f64,
    /// Navigation constant N′.
    pub n_prime: f64,
    /// The seeker.
    pub seeker: SeekerParams,
    /// Distance flown before the fuze arms, m.
    pub arm_dist_m: f64,
    /// Fuze trigger distance, m.
    pub trigger_radius_m: f64,
    /// Lethal radius of the warhead, m.
    pub lethal_radius_m: f64,
    /// Energy the warhead delivers at zero distance, J.
    pub damage_j: f64,
    /// Launch limits.
    pub launch: LaunchLimits,
    /// Longest flight, s.
    pub max_time_s: f64,
    /// Speed below which a coasting missile self-destructs, m/s.
    pub min_speed_mps: f64,
    /// Time after its target is destroyed before it self-destructs, s.
    pub target_lost_s: f64,
    /// Effective size to the eye while the motor burns, m.
    pub l_eff_burning_m: f64,
    /// Effective size to the eye while coasting, m.
    pub l_eff_m: f64,
}

impl MissileParams {
    /// The parameters of a missile entry for a tick of `dt_s`; `None` for a gun or malformed drag
    /// tables (the baker refuses both).
    #[must_use]
    pub fn from_def(def: &WeaponDef, dt_s: f64) -> Option<Self> {
        let m = def.missile.as_ref()?;
        let f = f64::from;
        let table = |c: &content::Curve| Table1D::from_f32(&c.xs, &c.ys).ok();
        Some(Self {
            mass_kg: f(def.mass_kg),
            thrust_n: f(m.motor.thrust_n),
            burn_s: f(m.motor.burn_s),
            propellant_kg: f(m.motor.propellant_kg),
            s_ref_m2: f(m.aero.s_ref_m2),
            cd0_boost: table(&m.aero.cd0_boost)?,
            cd0_coast: table(&m.aero.cd0_coast)?,
            cn_max: f(m.aero.cn_max),
            k_induced: f(m.aero.k_induced),
            tau_s: f(m.autopilot.tau_s),
            g_max: f(m.autopilot.g_max),
            g_min_speed_mps: f(m.autopilot.g_min_speed_mps),
            n_prime: f(m.guidance.n_prime),
            seeker: SeekerParams::from_def(def, dt_s)?,
            arm_dist_m: f(m.fuze.arm_dist_m),
            trigger_radius_m: f(m.fuze.trigger_radius_m),
            lethal_radius_m: f(m.warhead.lethal_radius_m),
            damage_j: f(m.warhead.damage_j),
            launch: LaunchLimits {
                min_speed_mps: f(m.launch.min_speed_mps),
                max_g: f(m.launch.max_g),
                max_alpha_rad: f(m.launch.max_alpha_deg).to_radians(),
                safe_sep_s: f(m.launch.safe_sep_s),
                min_range_m: f(m.launch.min_range_m),
            },
            max_time_s: f(m.limits.max_time_s),
            min_speed_mps: f(m.limits.min_speed_mps),
            target_lost_s: f(m.limits.target_lost_s),
            l_eff_burning_m: f(m.visual.l_eff_burning_m),
            l_eff_m: f(m.visual.l_eff_m),
        })
    }
}

/// Where a missile slot is in its life.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum MissilePhase {
    /// The slot is empty.
    #[default]
    Free = 0,
    /// Flying, motor burning.
    Boost = 1,
    /// Flying, motor out.
    Coast = 2,
    /// Detonated or expired this tick (the slot frees on the next).
    Done = 3,
}

impl MissilePhase {
    /// The phase's code (the snapshot's missile state).
    #[must_use]
    pub const fn code(self) -> u8 {
        self as u8
    }

    /// Boost or coast.
    #[must_use]
    pub const fn flying(self) -> bool {
        matches!(self, Self::Boost | Self::Coast)
    }
}

/// One missile (BF).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Missile {
    /// Its phase.
    pub phase: MissilePhase,
    /// Position, m.
    pub pos: DVec3,
    /// Velocity, m/s.
    pub vel: DVec3,
    /// Mass, kg.
    pub mass: f64,
    /// Time since launch, s.
    pub t_s: f64,
    /// Distance flown, m.
    pub flown_m: f64,
    /// Lateral acceleration the airframe is pulling, m/s².
    pub a_lat: DVec3,
    /// Its seeker.
    pub seeker: SeekerState,
    /// Where it was launched, m.
    pub launch_pos: DVec3,
    /// Time since its target was destroyed, s (0 while the target lives).
    pub target_lost_s: f64,
}

/// A missile leaving the rail at `pos_bf` with the launcher's velocity, its seeker as it was on
/// the rail.
#[must_use]
pub fn launch(
    params: &MissileParams,
    pos_bf: DVec3,
    vel_bf: DVec3,
    seeker: SeekerState,
) -> Missile {
    Missile {
        phase: MissilePhase::Boost,
        pos: pos_bf,
        vel: vel_bf,
        mass: params.mass_kg,
        t_s: 0.0,
        flown_m: 0.0,
        a_lat: DVec3::ZERO,
        seeker,
        launch_pos: pos_bf,
        target_lost_s: 0.0,
    }
}

/// The lateral acceleration the missile can pull now, m/s²: `min(g_max G0, q̄ S cn_max / m)`, and
/// 0 below `g_min_speed_mps` of airspeed.
#[must_use]
pub fn lateral_limit<E: FlightEnv>(
    m: &Missile,
    params: &MissileParams,
    env: &E,
    atmo: &Atmosphere,
    t_s: f64,
) -> f64 {
    let speed = (m.vel - env.wind_bf(m.pos, t_s)).length();
    if speed.is_nan() || speed < params.g_min_speed_mps {
        return 0.0;
    }
    let rho = atmo.sample(env.height_msl_m(m.pos), env.day()).rho_kgpm3;
    let qbar = 0.5 * rho * speed * speed;
    (params.g_max * G0).min(qbar * params.s_ref_m2 * params.cn_max / m.mass)
}

/// True proportional navigation (GDD 15.5.3): `a = N′ Vc (Ω × R̂)`, `Ω = (R × Vr) / (R·R)`,
/// `Vc = −(R·Vr) / |R|`, for the line of sight `R` (missile to target) and the relative velocity
/// `Vr` (target less missile). Perpendicular to the line of sight; zero on a collision course.
#[must_use]
pub fn pn_accel(r: DVec3, vr: DVec3, n_prime: f64) -> DVec3 {
    let rr = r.length_squared();
    if !(rr > 0.0 && rr.is_finite() && vr.is_finite()) {
        return DVec3::ZERO;
    }
    let range = dmath::sqrt(rr);
    let omega = r.cross(vr) * (1.0 / rr);
    let closing = -r.dot(vr) / range;
    omega.cross(r * (1.0 / range)) * (n_prime * closing)
}

/// The guidance command, BF, m/s²: zero unless the seeker is locked. The line of sight is to the
/// target displaced by the seeker noise `|R| σ (n1 e1 + n2 e2)` (`n` standard normals drawn by the
/// caller, `e1, e2` perpendicular to the line of sight); PN plus the cancellation of gravity's
/// lateral part, less any component along the velocity, limited to [`lateral_limit`].
#[must_use]
pub fn guide<E: FlightEnv>(
    m: &Missile,
    params: &MissileParams,
    target: &TargetView,
    noise: [f64; 2],
    env: &E,
    atmo: &Atmosphere,
    t_s: f64,
) -> DVec3 {
    if m.seeker.mode != SeekerMode::Locked {
        return DVec3::ZERO;
    }
    let r_true = target.pos_bf - m.pos;
    let range = r_true.length();
    let (e1, e2) = orthonormal_pair(r_true.normalized_or(DVec3::X));
    let r = r_true + (e1 * noise[0] + e2 * noise[1]) * (range * params.seeker.noise_rad);
    let a_pn = pn_accel(r, target.vel_bf - m.vel, params.n_prime);
    let v_hat = m.vel.normalized_or(DVec3::X);
    let g = env.gravity_bf(m.pos);
    let a_g = -(g - v_hat * g.dot(v_hat));
    let mut a = a_pn + a_g;
    a -= v_hat * a.dot(v_hat);
    let limit = lateral_limit(m, params, env, atmo, t_s);
    let len = a.length();
    if !(len.is_finite() && limit > 0.0) {
        return DVec3::ZERO;
    }
    if len > limit { a * (limit / len) } else { a }
}

/// `(dp, dv, dm)` of the point mass.
#[allow(clippy::too_many_arguments)] // ALGO: the RK4 stage's whole state
fn derivative<E: FlightEnv>(
    params: &MissileParams,
    env: &E,
    atmo: &Atmosphere,
    a_lat: DVec3,
    burning: bool,
    p: DVec3,
    v: DVec3,
    mass: f64,
    t_s: f64,
) -> (DVec3, DVec3, f64) {
    let v_rel = v - env.wind_bf(p, t_s);
    let speed = v_rel.length();
    let air = atmo.sample(env.height_msl_m(p), env.day());
    let qbar = 0.5 * air.rho_kgpm3 * speed * speed;
    let mach = speed / air.a_mps;
    let (thrust, mdot, cd0) = if burning {
        (
            params.thrust_n,
            -params.propellant_kg / params.burn_s,
            params.cd0_boost.eval(mach),
        )
    } else {
        (0.0, 0.0, params.cd0_coast.eval(mach))
    };
    // The held lateral acceleration stays across the flight path as the velocity turns.
    let v_hat = v.normalized_or(DVec3::X);
    let a_lat = a_lat - v_hat * a_lat.dot(v_hat);
    let cn = if qbar >= MIN_QBAR_PA {
        mass * a_lat.length() / (qbar * params.s_ref_m2)
    } else {
        0.0
    };
    let drag = qbar * params.s_ref_m2 * (cd0 + params.k_induced * cn * cn);
    let along = if speed > 0.0 {
        v_rel * ((thrust - drag) / (mass * speed))
    } else {
        DVec3::ZERO
    };
    (v, along + a_lat + env.gravity_bf(p), mdot)
}

/// One classic RK4 step of `h` from `t_s` with `a_lat` and the motor held.
#[allow(clippy::too_many_arguments)] // ALGO: as `derivative`
fn rk4<E: FlightEnv>(
    m: &mut Missile,
    params: &MissileParams,
    env: &E,
    atmo: &Atmosphere,
    a_lat: DVec3,
    burning: bool,
    t_s: f64,
    h: f64,
) {
    let d = |p, v, mass, t| derivative(params, env, atmo, a_lat, burning, p, v, mass, t);
    let (p0, v0, m0) = (m.pos, m.vel, m.mass);
    let k1 = d(p0, v0, m0, t_s);
    let k2 = d(
        p0 + k1.0 * (0.5 * h),
        v0 + k1.1 * (0.5 * h),
        m0 + k1.2 * (0.5 * h),
        t_s + 0.5 * h,
    );
    let k3 = d(
        p0 + k2.0 * (0.5 * h),
        v0 + k2.1 * (0.5 * h),
        m0 + k2.2 * (0.5 * h),
        t_s + 0.5 * h,
    );
    let k4 = d(p0 + k3.0 * h, v0 + k3.1 * h, m0 + k3.2 * h, t_s + h);
    let sixth = h / 6.0;
    m.pos = p0 + (k1.0 + (k2.0 + k3.0) * 2.0 + k4.0) * sixth;
    m.vel = v0 + (k1.1 + (k2.1 + k3.1) * 2.0 + k4.1) * sixth;
    m.mass = m0 + (k1.2 + (k2.2 + k3.2) * 2.0 + k4.2) * sixth;
}

/// Advances a flying missile by one tick of `dt` seconds from time `t_s` with the guidance command
/// `a_cmd` held through the tick: two RK4 sub-steps (120 Hz at a 60 Hz tick). Updates the phase
/// at burnout. A missile that is not flying is left alone.
pub fn step_missile<E: FlightEnv>(
    m: &mut Missile,
    params: &MissileParams,
    a_cmd: DVec3,
    env: &E,
    atmo: &Atmosphere,
    t_s: f64,
    dt: f64,
) {
    step_missile_n(m, params, a_cmd, env, atmo, t_s, dt, 2);
}

/// [`step_missile`] with `substeps` RK4 sub-steps per tick (the fly-out predictor uses 1, the
/// tests' reference 8).
#[allow(clippy::too_many_arguments)] // ALGO: `step_missile` plus the sub-step count
pub fn step_missile_n<E: FlightEnv>(
    m: &mut Missile,
    params: &MissileParams,
    a_cmd: DVec3,
    env: &E,
    atmo: &Atmosphere,
    t_s: f64,
    dt: f64,
    substeps: u32,
) {
    if !m.phase.flying() || substeps == 0 {
        return;
    }
    let h = dt / f64::from(substeps);
    // Fraction of the way to the command after one sub-step, and the mean fraction over it.
    let x = h / params.tau_s;
    let end_frac = -dmath::exp_m1(-x);
    let mean_frac = 1.0 - end_frac / x;
    for i in 0..substeps {
        let t0 = t_s + f64::from(i) * h;
        let v_hat = m.vel.normalized_or(DVec3::X);
        let limit = lateral_limit(m, params, env, atmo, t0);
        let lateral = |a: DVec3| {
            let a = a - v_hat * a.dot(v_hat);
            let len = a.length();
            if len > limit { a * (limit / len) } else { a }
        };
        let held = lateral(m.a_lat + (a_cmd - m.a_lat) * mean_frac);
        let a_end = lateral(m.a_lat + (a_cmd - m.a_lat) * end_frac);
        let p_before = m.pos;
        let burn_left = params.burn_s - m.t_s;
        if burn_left > TIME_EPS_S && burn_left < h - TIME_EPS_S {
            // Burnout inside the sub-step: burn to it, then coast the rest.
            rk4(m, params, env, atmo, held, true, t0, burn_left);
            rk4(
                m,
                params,
                env,
                atmo,
                held,
                false,
                t0 + burn_left,
                h - burn_left,
            );
        } else {
            rk4(m, params, env, atmo, held, burn_left > TIME_EPS_S, t0, h);
        }
        m.a_lat = a_end;
        m.t_s += h;
        m.flown_m += (m.pos - p_before).length();
    }
    if m.phase == MissilePhase::Boost && m.t_s >= params.burn_s - TIME_EPS_S {
        m.phase = MissilePhase::Coast;
    }
}

/// Why a missile ends without detonating (GDD 15.6).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Expiry {
    /// It hit the ground.
    Ground,
    /// It flew for `max_time_s`.
    Timeout,
    /// It slowed below `min_speed_mps` after burnout.
    Slow,
    /// Its target was destroyed `target_lost_s` ago.
    TargetLost,
}

/// Whether a flying missile ends now, and why (checked in the order of [`Expiry`]).
/// `target_alive` is false once its target has been destroyed; the caller adds the tick to
/// `target_lost_s` while it is.
#[must_use]
pub fn missile_expiry<E: FlightEnv>(
    m: &Missile,
    params: &MissileParams,
    env: &E,
    target_alive: bool,
) -> Option<Expiry> {
    if env.height_msl_m(m.pos) < env.ground_height_m(m.pos) {
        Some(Expiry::Ground)
    } else if m.t_s >= params.max_time_s - TIME_EPS_S {
        Some(Expiry::Timeout)
    } else if m.t_s >= params.burn_s - TIME_EPS_S && m.vel.length() < params.min_speed_mps {
        Some(Expiry::Slow)
    } else if !target_alive && m.target_lost_s >= params.target_lost_s - TIME_EPS_S {
        Some(Expiry::TargetLost)
    } else {
        None
    }
}

/// The missile's effective size to the eye now (GDD 16.6.2), m.
#[must_use]
pub fn visual_l_eff_m(m: &Missile, params: &MissileParams) -> f64 {
    if m.phase == MissilePhase::Boost {
        params.l_eff_burning_m
    } else {
        params.l_eff_m
    }
}

#[cfg(test)]
mod tests;
