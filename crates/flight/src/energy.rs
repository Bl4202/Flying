//! Energy-manoeuvrability quantities (GDD 11.12, B.3). Reported with standard gravity `g0`, as
//! the reference calculator defines them (GDD C.8, ADR-31).

use simmath::dmath;
use simmath::math::DVec3;

use crate::physical::G0;

/// Specific energy (energy height) `E_s = h + V^2 / (2 g0)`, m.
#[must_use]
pub fn specific_energy_m(height_m: f64, speed_mps: f64) -> f64 {
    height_m + speed_mps * speed_mps / (2.0 * G0)
}

/// Specific excess power `P_s = F . v / (m g0)`, m/s, from the sum of non-gravitational forces
/// (aerodynamic plus propulsive) and the velocity relative to the ground.
#[must_use]
pub fn specific_excess_power_mps(force_nongrav: DVec3, vel: DVec3, mass_kg: f64) -> f64 {
    force_nongrav.dot(vel) / (mass_kg * G0)
}

/// Level-turn rate `g0 sqrt(n^2 - 1) / V`, rad/s (zero for `n <= 1`).
#[must_use]
pub fn turn_rate_rps(load_factor: f64, speed_mps: f64) -> f64 {
    G0 * dmath::sqrt((load_factor * load_factor - 1.0).max(0.0)) / speed_mps
}

/// Level-turn radius `V^2 / (g0 sqrt(n^2 - 1))`, m (infinite for `n <= 1`).
#[must_use]
pub fn turn_radius_m(load_factor: f64, speed_mps: f64) -> f64 {
    speed_mps * speed_mps / (G0 * dmath::sqrt((load_factor * load_factor - 1.0).max(0.0)))
}

/// Wind-axis load factor of a level turn, `sqrt(1 + (V w / g0)^2)`, from the speed and the turn
/// rate (ADR-31: the sustained-turn metric, not the body-axis accelerometer).
#[must_use]
pub fn level_turn_load_factor(speed_mps: f64, turn_rate_rps: f64) -> f64 {
    let k = speed_mps * turn_rate_rps / G0;
    dmath::sqrt(1.0 + k * k)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::approx_constant)] // n = 6.28 is GDD C.7 data, not an approximation of tau
    fn turn_figures_match_appendix_c() {
        // GDD C.7: 10 500 kg, SL, M 0.5 (170 m/s), n 6.28 gives 20.5 deg/s.
        let w = turn_rate_rps(6.28, 170.0).to_degrees();
        assert!((w - 20.5).abs() < 0.1, "{w}");
        // Corner, SL M 0.55 (187 m/s) at 9 g: 26.9 deg/s.
        assert!((turn_rate_rps(9.0, 187.0).to_degrees() - 26.9).abs() < 0.1);
        // Round trip between n and turn rate.
        let n = level_turn_load_factor(238.0, turn_rate_rps(8.45, 238.0));
        assert!((n - 8.45).abs() < 1e-12);
        assert_eq!(turn_rate_rps(1.0, 200.0), 0.0);
        assert!(turn_radius_m(1.0, 200.0).is_infinite());
        // r = V / w.
        let r = turn_radius_m(5.0, 250.0);
        assert!((r - 250.0 / turn_rate_rps(5.0, 250.0)).abs() < 1e-9);
    }

    #[test]
    fn energy_height_and_excess_power() {
        // A 1 000 m zoom from 300 m/s at constant E_s ends near 265.4 m/s.
        let e = specific_energy_m(1_000.0, 300.0);
        let v2 = dmath::sqrt(2.0 * G0 * (e - 2_000.0));
        assert!((specific_energy_m(2_000.0, v2) - e).abs() < 1e-9);
        assert!((v2 - 265.4).abs() < 0.1);
        // 10 kN of excess thrust along 200 m/s at 10 000 kg is 20.39 m/s.
        let ps = specific_excess_power_mps(DVec3::X * 10_000.0, DVec3::X * 200.0, 10_000.0);
        assert!((ps - 2_000_000.0 / (10_000.0 * G0)).abs() < 1e-12);
    }
}
