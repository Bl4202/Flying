//! Air-relative quantities and speed definitions (GDD 11.2.3, 11.5, B.2).
//!
//! [`AirData::compute`] is on the hot path (every RK4 stage): it needs only `+ - * /`, `sqrt`,
//! `atan2` and `asin`. Calibrated and equivalent airspeed are display and limit quantities and are
//! computed separately ([`cas_mps`], [`eas_mps`]).

use simmath::dmath;
use simmath::math::DVec3;

use crate::atmosphere::AtmoSample;
use crate::physical::{A0, P0, RHO0};

/// Below this true airspeed the flow angles are undefined and held at zero (GDD 11.20), m/s.
pub const MIN_TAS_FOR_ANGLES_MPS: f64 = 1.0;

/// The air-relative state the aerodynamic model needs.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AirData {
    /// True airspeed, m/s.
    pub tas_mps: f64,
    /// Angle of attack, rad: positive with the nose above the relative wind.
    pub alpha_rad: f64,
    /// Sideslip, rad: positive with the relative wind from the right.
    pub beta_rad: f64,
    /// Mach number.
    pub mach: f64,
    /// Dynamic pressure, Pa.
    pub qbar_pa: f64,
    /// Non-dimensional roll rate `p b / (2 V)`.
    pub p_hat: f64,
    /// Non-dimensional pitch rate `q c / (2 V)`.
    pub q_hat: f64,
    /// Non-dimensional yaw rate `r b / (2 V)`.
    pub r_hat: f64,
}

impl AirData {
    /// Air data from the body-axis air-relative velocity `(u, v, w)` (FRD), the body rates
    /// `(p, q, r)`, the local air and the reference lengths (span `b`, mean chord `c`).
    #[must_use]
    pub fn compute(
        vel_air_body: DVec3,
        omega_body: DVec3,
        air: &AtmoSample,
        span_m: f64,
        mac_m: f64,
    ) -> Self {
        let tas = vel_air_body.length();
        let (alpha, beta) = if tas >= MIN_TAS_FOR_ANGLES_MPS {
            (
                dmath::atan2(vel_air_body.z, vel_air_body.x),
                dmath::asin((vel_air_body.y / tas).clamp(-1.0, 1.0)),
            )
        } else {
            (0.0, 0.0)
        };
        // The rate non-dimensionals divide by V; guard the division (GDD 11.2.3).
        let v_ref = tas.max(MIN_TAS_FOR_ANGLES_MPS);
        Self {
            tas_mps: tas,
            alpha_rad: alpha,
            beta_rad: beta,
            mach: tas / air.a_mps,
            qbar_pa: 0.5 * air.rho_kgpm3 * tas * tas,
            p_hat: omega_body.x * span_m / (2.0 * v_ref),
            q_hat: omega_body.y * mac_m / (2.0 * v_ref),
            r_hat: omega_body.z * span_m / (2.0 * v_ref),
        }
    }
}

/// Equivalent airspeed, m/s: `V_T sqrt(rho / rho0)`.
#[must_use]
pub fn eas_mps(tas_mps: f64, rho_kgpm3: f64) -> f64 {
    tas_mps * dmath::sqrt(rho_kgpm3 / RHO0)
}

/// Rayleigh pitot ratio `p02 / p` for a normal shock ahead of the probe, valid for `M >= 1`:
/// `(7.2^3.5 / 6) M^7 / (7 M^2 - 1)^2.5` (GDD 11.5 quotes the constant as 166.92).
fn rayleigh_ratio(mach: f64) -> f64 {
    let k = dmath::pow(7.2, 3.5) / 6.0;
    k * dmath::pow(mach, 7.0) / dmath::pow(7.0 * mach * mach - 1.0, 2.5)
}

/// Impact pressure `q_c` sensed by a pitot probe, Pa: isentropic below Mach 1, Rayleigh above.
#[must_use]
pub fn impact_pressure_pa(p_pa: f64, mach: f64) -> f64 {
    if mach < 1.0 {
        p_pa * (dmath::pow(1.0 + 0.2 * mach * mach, 3.5) - 1.0)
    } else {
        p_pa * (rayleigh_ratio(mach) - 1.0)
    }
}

/// Calibrated airspeed, m/s: the sea-level standard-day speed with the same impact pressure.
///
/// Below `a0` the isentropic relation is inverted in closed form; above it the Rayleigh relation
/// is inverted by bisection (60 halvings, deterministic). Not for hot paths.
#[must_use]
pub fn cas_mps(p_pa: f64, mach: f64) -> f64 {
    let qc = impact_pressure_pa(p_pa, mach);
    let ratio = qc / P0;
    // q_c / p0 at a calibrated Mach of exactly 1, where the two branches meet.
    let sonic = dmath::pow(1.2, 3.5) - 1.0;
    if ratio.is_nan() || ratio <= 0.0 {
        return 0.0;
    }
    if ratio <= sonic {
        return A0 * dmath::sqrt(5.0 * (dmath::pow(ratio + 1.0, 2.0 / 7.0) - 1.0));
    }
    let (mut lo, mut hi) = (1.0_f64, 10.0_f64);
    for _ in 0..60 {
        let mid = lo.midpoint(hi);
        if rayleigh_ratio(mid) - 1.0 < ratio {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    A0 * lo.midpoint(hi)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::atmosphere::{NonStandardDay, sample_analytic, standard_geopotential};
    use crate::physical::{GAMMA, R_AIR};

    fn sea_level() -> AtmoSample {
        sample_analytic(0.0, NonStandardDay::ISA)
    }

    #[test]
    fn flow_angles_follow_the_frd_conventions() {
        let air = sea_level();
        let level = AirData::compute(DVec3::new(100.0, 0.0, 0.0), DVec3::ZERO, &air, 9.8, 3.2);
        assert_eq!((level.alpha_rad, level.beta_rad), (0.0, 0.0));
        // Air arriving from below (w > 0 in FRD) is a positive angle of attack.
        let up = AirData::compute(DVec3::new(100.0, 0.0, 10.0), DVec3::ZERO, &air, 9.8, 3.2);
        assert!((up.alpha_rad - dmath::atan(0.1)).abs() < 1e-15);
        // Velocity to the right relative to the air: wind from the right, positive sideslip.
        let side = AirData::compute(DVec3::new(100.0, 10.0, 0.0), DVec3::ZERO, &air, 9.8, 3.2);
        let expected = dmath::asin(10.0 / dmath::sqrt(10_100.0));
        assert!((side.beta_rad - expected).abs() < 1e-15);
        assert!((side.tas_mps - dmath::sqrt(10_100.0)).abs() < 1e-12);
    }

    #[test]
    fn dynamic_pressure_mach_and_rate_non_dimensionals() {
        let air = sea_level();
        let v = 0.5 * air.a_mps;
        let d = AirData::compute(
            DVec3::new(v, 0.0, 0.0),
            DVec3::new(1.0, 0.5, -0.2),
            &air,
            9.8,
            3.2,
        );
        assert!((d.mach - 0.5).abs() < 1e-15);
        // GDD C.5: q-bar at SL, M 0.5 is 17 732 Pa.
        assert!((d.qbar_pa - 17_732.0).abs() < 1.0, "{}", d.qbar_pa);
        assert!((d.p_hat - 9.8 / (2.0 * v)).abs() < 1e-15);
        assert!((d.q_hat - 0.5 * 3.2 / (2.0 * v)).abs() < 1e-15);
        assert!((d.r_hat + 0.2 * 9.8 / (2.0 * v)).abs() < 1e-15);
    }

    #[test]
    fn slow_flight_has_no_flow_angles_and_bounded_rates() {
        let air = sea_level();
        let d = AirData::compute(
            DVec3::new(0.0, 0.3, -0.5),
            DVec3::new(2.0, 0.0, 0.0),
            &air,
            9.8,
            3.2,
        );
        assert_eq!((d.alpha_rad, d.beta_rad), (0.0, 0.0));
        assert!(d.p_hat.is_finite());
        assert_eq!(d.p_hat, 2.0 * 9.8 / 2.0);
        let still = AirData::compute(DVec3::ZERO, DVec3::ZERO, &air, 9.8, 3.2);
        assert_eq!(still.qbar_pa, 0.0);
    }

    #[test]
    fn cas_equals_tas_at_sea_level_on_both_branches() {
        let air = sea_level();
        for mach in [0.1, 0.5, 0.95, 1.0, 1.2, 1.5, 2.0] {
            let tas = mach * air.a_mps;
            let cas = cas_mps(air.p_pa, mach);
            assert!(
                (cas - tas).abs() < 1e-6 * tas,
                "M {mach}: CAS {cas} vs TAS {tas}"
            );
        }
        assert!((eas_mps(150.0, air.rho_kgpm3) - 150.0).abs() < 1e-3);
    }

    #[test]
    fn cas_worked_example_at_altitude() {
        // 10 000 m geopotential, M 0.8: q_c = 26 436.2 Pa * (1.128^3.5 - 1) = 13 861.5 Pa, which
        // gives CAS = 340.294 * sqrt(5 ((1.136 802)^(2/7) - 1)) = 146.99 m/s (285.7 kn).
        let (t, p) = standard_geopotential(10_000.0);
        let cas = cas_mps(p, 0.8);
        assert!((cas - 146.99).abs() < 0.05, "{cas}");
        // Supersonic at altitude: CAS stays below TAS and the branches meet continuously.
        let a = dmath::sqrt(GAMMA * R_AIR * t);
        let cas_m2 = cas_mps(p, 2.0);
        assert!(cas_m2 < 2.0 * a);
        assert!((impact_pressure_pa(p, 1.0 - 1e-9) - impact_pressure_pa(p, 1.0)).abs() < 1e-4);
    }

    #[test]
    fn cas_is_monotone_in_mach() {
        let (_, p) = standard_geopotential(11_000.0);
        let mut last = 0.0;
        for i in 1..=300 {
            let cas = cas_mps(p, f64::from(i) * 0.01);
            assert!(
                cas > last,
                "CAS not increasing at M {}",
                f64::from(i) * 0.01
            );
            last = cas;
        }
        assert_eq!(cas_mps(p, 0.0), 0.0);
    }
}
