//! The flight condition as the control laws use it: derived once per tick from the airframe's
//! telemetry and state.

use flight::aircraft::{AircraftModel, AircraftState, Sense, Telemetry};
use flight::env::FlightEnv;
use flight::frames::euler;
use flight::physical::G0;
use simmath::dmath;
use simmath::math::DVec3;

/// Below this airspeed the angles and rates the laws divide by are not trusted, m/s.
pub const MIN_V_MPS: f64 = 20.0;

/// The condition of the aircraft this tick.
#[derive(Clone, Copy, Debug)]
pub struct Sensed {
    /// What the airframe model reports.
    pub tel: Telemetry,
    /// True airspeed, at least [`MIN_V_MPS`], m/s.
    pub v: f64,
    /// Angle of attack, rad.
    pub alpha: f64,
    /// Sideslip, rad (positive with the airflow from the right).
    pub beta: f64,
    /// Dynamic pressure, Pa.
    pub qbar_pa: f64,
    /// Mach number.
    pub mach: f64,
    /// Calibrated airspeed, m/s.
    pub cas: f64,
    /// Bank, rad.
    pub phi: f64,
    /// Pitch attitude, rad.
    pub theta: f64,
    /// Flight-path angle, rad.
    pub gamma: f64,
    /// Bank of the lift vector about the velocity vector, rad. Unlike [`Self::phi`] it stays
    /// continuous over the top of a loop, where the attitude angles flip by half a turn.
    pub bank_w: f64,
    /// Gravity along the stability-axis normal axis in g: the load factor that holds the flight
    /// path (`cos(gamma) cos(bank_w)`), also continuous through a vertical attitude.
    pub turn_load: f64,
    /// Body rates, rad/s.
    pub rates: DVec3,
    /// The angular acceleration the airframe has now, rad/s^2.
    pub omega_dot: DVec3,
    /// Wind-axis load factor (lift over weight), g.
    pub n_w: f64,
    /// Side force over mass and airspeed, rad/s (the rate it turns the flow).
    pub side_rate: f64,
    /// Gear extension, 0 to 1.
    pub gear_pos: f64,
    /// Trailing-edge flap travel used, 0 to 1.
    pub flap_fraction: f64,
}

impl Sensed {
    /// Derives the condition of `ac` from the airframe's sense.
    #[must_use]
    pub fn of<E: FlightEnv>(
        model: &AircraftModel,
        ac: &AircraftState,
        sense: &Sense,
        env: &E,
    ) -> Self {
        let tel = sense.tel;
        let up = env.up_bf(ac.body.pos_bf);
        let e = euler(up, ac.body.att);
        let speed = ac.body.vel_bf.length().max(1.0);
        let climb = ac.body.vel_bf.dot(up);
        // The forces the aerodynamics and the engines give: the push of the wheels is not lift.
        let f = tel.force_body_n - tel.ground_force_body_n;
        let weight = tel.mass_kg * G0;
        let a = tel.air.alpha_rad;
        let v = tel.air.tas_mps.max(MIN_V_MPS);
        let (sin_t, cos_t) = (dmath::sin(e.pitch_rad), dmath::cos(e.pitch_rad));
        let (sin_p, cos_p) = (dmath::sin(e.roll_rad), dmath::cos(e.roll_rad));
        // Gravity in body axes is g (-sin theta, sin phi cos theta, cos phi cos theta); its normal
        // component in the stability axes is what the lift has to carry.
        let turn_load = dmath::cos(a) * cos_p * cos_t + dmath::sin(a) * sin_t;
        Self {
            tel,
            v,
            alpha: a,
            beta: tel.air.beta_rad,
            qbar_pa: tel.air.qbar_pa,
            mach: tel.air.mach,
            cas: tel.cas_mps,
            phi: e.roll_rad,
            theta: e.pitch_rad,
            gamma: dmath::asin((climb / speed).clamp(-1.0, 1.0)),
            bank_w: dmath::atan2(sin_p * cos_t, turn_load),
            turn_load,
            rates: ac.body.omega_body,
            omega_dot: sense.omega_dot_rps2,
            n_w: (f.x * dmath::sin(a) - f.z * dmath::cos(a)) / weight,
            side_rate: f.y / (tel.mass_kg * v),
            gear_pos: ac.surfaces.gear,
            flap_fraction: model.flap_fraction(&ac.surfaces),
        }
    }

    /// Dynamic pressure in kilopascals (the unit of the schedules).
    #[must_use]
    pub fn qbar_kpa(&self) -> f64 {
        self.qbar_pa * 1.0e-3
    }

    /// The yaw rate about the stability axis, rad/s: what is left of the body yaw rate once the
    /// part that a roll about the velocity vector gives (`p sin alpha`) is taken out. A commanded
    /// roll at a high angle of attack has none; a spin has it all.
    #[must_use]
    pub fn yaw_rate_stability(&self) -> f64 {
        self.rates.z * dmath::cos(self.alpha) - self.rates.x * dmath::sin(self.alpha)
    }

    /// The pitch rate that keeps the angle of attack constant, rad/s: `alpha_dot = q - q_hold`.
    /// From `alpha_dot = q - tan(beta) p_s - (g / V cos(beta)) (n - cos(gamma) cos(bank))`, with
    /// `p_s` the roll rate about the velocity vector. Rolling with sideslip changes the angle of
    /// attack as much as a pull does.
    #[must_use]
    pub fn q_hold(&self) -> f64 {
        let p_s = self.rates.x * dmath::cos(self.alpha) + self.rates.z * dmath::sin(self.alpha);
        let cos_beta = dmath::cos(self.beta).max(0.5);
        self.g_over_v() * (self.n_w - self.turn_load) / cos_beta + dmath::tan(self.beta) * p_s
    }

    /// `g / V`, 1/s.
    #[must_use]
    pub fn g_over_v(&self) -> f64 {
        G0 / self.v
    }
}
