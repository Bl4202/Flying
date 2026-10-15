//! The test pilot: a deterministic flight-test instrument, not gameplay (ADR-31).
//!
//! It writes surface commands directly; there is no FCS. Outer loops turn the procedure's targets
//! (an altitude, a load factor or an angle of attack, and a bank angle) into body-rate commands;
//! the inner loops are incremental dynamic inversion: the model gives the angular acceleration
//! the aircraft has now and how each surface changes it, and the pilot moves the surfaces by the
//! increment that gives the acceleration it wants. Because it inverts the model's own response
//! there is no gain scheduling across dynamic pressure, and the loops stay well below the 15 rad/s
//! the integrator is designed for (GDD 11.10). The state is noise-free, so there are no filters.

use flight::actuators::Surfaces;
use flight::aircraft::{AircraftModel, AircraftState, Telemetry};
use flight::env::FlightEnv;
use flight::frames::euler;
use flight::physical::G0;
use simmath::dmath;
use simmath::math::DVec3;

/// Pitch-rate loop bandwidth, 1/s. // ALGO: plan WP7
pub const K_Q: f64 = 8.0;
/// Roll-rate loop bandwidth, 1/s. // ALGO: plan WP7
pub const K_P: f64 = 6.0;
/// Yaw-rate loop bandwidth, 1/s. // ALGO: plan WP7
pub const K_R: f64 = 4.0;
/// Angle-of-attack loop bandwidth, 1/s. // ALGO: plan WP7
pub const K_ALPHA: f64 = 3.0;
/// Flight-path loop bandwidth, 1/s. // ALGO
pub const K_GAMMA: f64 = 1.0;
/// Altitude loop: commanded climb rate per metre of error, 1/s. // ALGO: plan WP7
pub const K_H: f64 = 0.5;
/// Largest commanded climb or sink rate, m/s. // ALGO: plan WP7
pub const MAX_CLIMB_MPS: f64 = 20.0;
/// Bank loop: commanded roll rate per radian of error, 1/s. // ALGO: plan WP7
pub const K_BANK: f64 = 2.0;
/// Largest commanded roll rate, rad/s (90 deg/s). // ALGO: plan WP7
pub const MAX_ROLL_RATE_RPS: f64 = core::f64::consts::FRAC_PI_2;
/// Sideslip feedback into the yaw-rate command, 1/s. // ALGO: plan WP7
pub const K_BETA: f64 = 2.0;
/// Surface perturbation for the control-effectiveness estimate, rad. // ALGO
const PROBE_RAD: f64 = 0.01;
/// A command runs at most this far ahead of the surface: the travel its rate limit allows in
/// this time, s. Without it, at low dynamic pressure the inversion asks for steps the rate limit
/// turns into phase lag, and the pitch loop cycles between the stops. // ALGO
pub const LEAD_S: f64 = 0.1;

/// What the pilot holds in the vertical plane.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Vertical {
    /// A height above MSL, m.
    Altitude(f64),
    /// A wind-axis load factor, g.
    LoadFactor(f64),
    /// An angle of attack and its rate of change, rad and rad/s.
    Alpha(f64, f64),
}

/// The flight state the loops close on, read once per tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Observed {
    /// Telemetry at the state.
    pub tel: Telemetry,
    /// Bank (Euler roll), rad.
    pub bank_rad: f64,
    /// Pitch, rad.
    pub pitch_rad: f64,
    /// Flight-path angle, rad.
    pub gamma_rad: f64,
    /// Wind-axis load factor `(F . -z_wind) / (m g0)`, g.
    pub n_wind: f64,
    /// Specific excess power `F . v / (m g0)`, m/s.
    pub ps_mps: f64,
    /// Rate of change of the track heading, rad/s (positive turning right).
    pub turn_rate_rps: f64,
    /// Climb rate `v . up`, m/s.
    pub climb_mps: f64,
}

/// Reads the loops' inputs from a state.
#[must_use]
pub fn observe<E: FlightEnv>(
    model: &AircraftModel,
    s: &AircraftState,
    env: &E,
    t_s: f64,
) -> Observed {
    let tel = model.telemetry(s, env, t_s);
    let up = env.up_bf(s.body.pos_bf);
    let e = euler(up, s.body.att);
    let v = s.body.vel_bf;
    let speed = v.length().max(1.0);
    let climb = v.dot(up);
    let a = tel.air.alpha_rad;
    let f = tel.force_body_n;
    let weight = tel.mass_kg * G0;
    // Acceleration in BF: the non-gravitational force plus gravity.
    let accel = s.body.att.rotate(f) / tel.mass_kg + env.gravity_bf(s.body.pos_bf);
    let v_h = v - up * climb;
    let turn = if v_h.length_squared() > 1.0 {
        -v.cross(accel).dot(up) / v_h.length_squared()
    } else {
        0.0
    };
    let v_body = s.body.att.conjugate().rotate(v);
    Observed {
        tel,
        bank_rad: e.roll_rad,
        pitch_rad: e.pitch_rad,
        gamma_rad: dmath::asin((climb / speed).clamp(-1.0, 1.0)),
        n_wind: (f.x * dmath::sin(a) - f.z * dmath::cos(a)) / weight,
        ps_mps: f.dot(v_body) / weight,
        turn_rate_rps: turn,
        climb_mps: climb,
    }
}

/// What the pilot holds this tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Target {
    /// The vertical-plane target.
    pub vertical: Vertical,
    /// Bank angle, rad (positive right wing down).
    pub bank_rad: f64,
    /// Flaps, airbrake and gear, passed through.
    pub config: Surfaces,
}

/// The pilot's memory: none needed beyond the state, so it is a unit struct; kept as a type so
/// later pilots can carry integrators.
#[derive(Clone, Copy, Debug, Default)]
pub struct Pilot;

/// Solves the 3 x 3 system `a x = b` (Cramer's rule); `None` if singular.
fn solve3(a: [[f64; 3]; 3], b: [f64; 3]) -> Option<[f64; 3]> {
    let det = |m: [[f64; 3]; 3]| {
        m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
            - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
            + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
    };
    let d = det(a);
    if d.abs() < 1e-12 || !d.is_finite() {
        return None;
    }
    let mut x = [0.0; 3];
    for (k, xk) in x.iter_mut().enumerate() {
        let mut m = a;
        for (row, bi) in m.iter_mut().zip(b) {
            row[k] = bi;
        }
        *xk = det(m) / d;
    }
    Some(x)
}

impl Pilot {
    /// Surface commands for this tick: hold the target's vertical and bank, coordinated.
    #[must_use]
    pub fn command<E: FlightEnv>(
        &mut self,
        model: &AircraftModel,
        s: &AircraftState,
        o: &Observed,
        target: &Target,
        env: &E,
    ) -> Surfaces {
        let (vertical, bank_cmd_rad, config) = (target.vertical, target.bank_rad, &target.config);
        let air = &o.tel.air;
        let v = air.tas_mps.max(1.0);
        let (sphi, cphi) = (dmath::sin(o.bank_rad), dmath::cos(o.bank_rad));
        let cgam = dmath::cos(o.gamma_rad);
        let qs = air.qbar_pa * model.aero.wing_area_m2();
        let weight = o.tel.mass_kg * G0;

        // Outer loops: the vertical target becomes a load factor, then an angle of attack.
        let (alpha_cmd, alpha_rate, n_cmd) = match vertical {
            Vertical::Altitude(h) => {
                let climb_cmd =
                    (K_H * (h - o.tel.height_msl_m)).clamp(-MAX_CLIMB_MPS, MAX_CLIMB_MPS);
                let gamma_cmd = dmath::asin((climb_cmd / v).clamp(-1.0, 1.0));
                let n = (cgam + v / G0 * K_GAMMA * (gamma_cmd - o.gamma_rad)) / cphi.max(0.1);
                (alpha_for(model, o, n, qs, weight), 0.0, n)
            }
            Vertical::LoadFactor(n) => (alpha_for(model, o, n, qs, weight), 0.0, n),
            Vertical::Alpha(a, rate) => (a, rate, o.n_wind),
        };
        // The velocity vector turns in the body x-z plane at g (n - cos(gamma) cos(phi)) / V.
        let q_ff = G0 / v * (n_cmd - cgam * cphi);
        let q_cmd = q_ff + alpha_rate + K_ALPHA * (alpha_cmd - air.alpha_rad);
        let p_cmd =
            (K_BANK * (bank_cmd_rad - o.bank_rad)).clamp(-MAX_ROLL_RATE_RPS, MAX_ROLL_RATE_RPS);
        // Positive sideslip has the nose left of the airflow; yawing right (r > 0) removes it,
        // since beta' = -r + ... .
        let r_cmd = G0 * sphi * dmath::cos(o.pitch_rad) / v + K_BETA * air.beta_rad;

        // Inner loops: incremental inversion of the angular acceleration.
        let w = s.body.omega_body;
        let want = DVec3::new(
            K_P * (p_cmd - w.x),
            K_Q * (q_cmd - w.y),
            K_R * (r_cmd - w.z),
        );
        let (now, _) = model.derivative(s, env, 0.0);
        // Columns: the angular acceleration per radian of aileron, elevator and rudder.
        let cols: [DVec3; 3] = core::array::from_fn(|j| {
            let mut probe = *s;
            match j {
                0 => probe.surfaces.aileron_rad += PROBE_RAD,
                1 => probe.surfaces.elevator_rad += PROBE_RAD,
                _ => probe.surfaces.rudder_rad += PROBE_RAD,
            }
            let (d, _) = model.derivative(&probe, env, 0.0);
            (d.omega - now.omega) / PROBE_RAD
        });
        let jac = [
            [cols[0].x, cols[1].x, cols[2].x],
            [cols[0].y, cols[1].y, cols[2].y],
            [cols[0].z, cols[1].z, cols[2].z],
        ];
        let err = want - now.omega;
        let delta = solve3(jac, [err.x, err.y, err.z]).unwrap_or([0.0; 3]);
        let a = &model.actuators;
        let lead = |act: &flight::actuators::Actuator, d: f64| {
            // max_step is the travel in one sub-step.
            let most =
                act.max_step * f64::from(flight::aircraft::SUBSTEPS) * LEAD_S / model.tick_dt_s();
            d.clamp(-most, most)
        };
        Surfaces {
            aileron_rad: a
                .aileron
                .clamp(s.surfaces.aileron_rad + lead(&a.aileron, delta[0])),
            elevator_rad: a
                .elevator
                .clamp(s.surfaces.elevator_rad + lead(&a.elevator, delta[1])),
            rudder_rad: a
                .rudder
                .clamp(s.surfaces.rudder_rad + lead(&a.rudder, delta[2])),
            ..*config
        }
    }
}

/// The angle of attack that gives load factor `n`, from the present one along the lift curve
/// (thrust's share of the normal force moves with it).
fn alpha_for(model: &AircraftModel, o: &Observed, n: f64, qs: f64, weight: f64) -> f64 {
    let slope = model.aero.cl_alpha_per_rad(o.tel.air.mach) * qs.max(1.0);
    o.tel.air.alpha_rad + (n - o.n_wind) * weight / slope
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::procedures::{Flight, TICK_DT_S, world};
    use flight::engine::EngineCommand;
    use flight::trim::{TrimProblem, TrimSetup, guess_level, solve, state_of};

    fn kestrel() -> AircraftModel {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content/core");
        let baked = content_bake::bake::bake_dir(&dir).expect("the core pack bakes");
        let r = content::Registry::from_cpack(&baked.cpack).unwrap();
        AircraftModel::from_registry(&r, r.aircraft_handles().next().unwrap(), TICK_DT_S).unwrap()
    }

    /// Level and trimmed at 5 000 m, M 0.6, 10 000 kg, throttle held at the trim.
    fn trimmed(m: &AircraftModel) -> (Flight<'_>, f64) {
        let env = world();
        let setup = TrimSetup {
            pos_bf: DVec3::new(0.0, 0.0, 5000.0),
            fuel: m.mass.load_for(1600.0),
            config: Surfaces::NEUTRAL,
        };
        let guess = guess_level(m, &setup, 192.3, 1.0, &env);
        let t = solve(m, &TrimProblem::level(), &setup, guess, &env);
        assert!(t.converged);
        let cmd = flight::trim::engine_command(m, t.point.throttle);
        let state = state_of(m, &setup, &t.point, &env);
        (
            Flight::new(
                m,
                state,
                EngineCommand { ..cmd },
                t.point.throttle,
                Surfaces::NEUTRAL,
                "t",
            ),
            t.point.alpha_rad,
        )
    }

    #[test]
    fn a_bank_capture_holds_altitude_and_the_ball() {
        let m = kestrel();
        let (mut f, _) = trimmed(&m);
        let bank = 30f64.to_radians();
        let mut worst_dh = 0.0_f64;
        for _ in 0..(15.0 / TICK_DT_S) as u32 {
            let o = f.fly(Vertical::Altitude(5000.0), bank);
            worst_dh = worst_dh.max((o.tel.height_msl_m - 5000.0).abs());
        }
        let o = f.observe();
        assert!(
            (o.bank_rad - bank).abs() < 0.5f64.to_radians(),
            "bank {}",
            o.bank_rad.to_degrees()
        );
        assert!(worst_dh < 10.0, "altitude wandered {worst_dh} m");
        assert!(
            o.tel.air.beta_rad.abs() < 0.2f64.to_radians(),
            "beta {}",
            o.tel.air.beta_rad.to_degrees()
        );
        assert!(o.turn_rate_rps > 0.0, "banked right, turning right");
        assert_eq!(f.faults, 0);
    }

    #[test]
    fn an_alpha_step_settles_and_repeats_exactly() {
        let m = kestrel();
        let fly = || {
            let (mut f, alpha0) = trimmed(&m);
            let target = alpha0 + 2f64.to_radians();
            for _ in 0..(5.0 / TICK_DT_S) as u32 {
                f.fly(Vertical::Alpha(target, 0.0), 0.0);
            }
            (f.observe().tel.air.alpha_rad - target, f.state)
        };
        let (err, a) = fly();
        assert!(
            err.abs() < 0.05f64.to_radians(),
            "alpha error {} deg",
            err.to_degrees()
        );
        let (_, b) = fly();
        assert_eq!(a, b, "the pilot is deterministic");
    }

    #[test]
    fn cramer_solves_a_small_system() {
        let x = solve3(
            [[2.0, 0.0, 1.0], [0.0, 3.0, 0.0], [1.0, 0.0, 1.0]],
            [3.0, 6.0, 2.0],
        )
        .unwrap();
        assert!(
            (x[0] - 1.0).abs() < 1e-12 && (x[1] - 2.0).abs() < 1e-12 && (x[2] - 1.0).abs() < 1e-12
        );
        assert!(solve3([[1.0; 3]; 3], [1.0; 3]).is_none());
    }
}
