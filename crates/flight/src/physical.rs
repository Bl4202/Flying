//! Physical constants (GDD 11.3, Appendix B). Nothing aircraft-specific lives here; aircraft
//! numbers are content (ADR-13).

/// Standard gravity, m/s^2. // PHYS: ISO 80000-3 / ISA
pub const G0: f64 = 9.806_65;
/// Specific gas constant of dry air, J/(kg K). // PHYS: ISA (GDD B.2)
pub const R_AIR: f64 = 287.052_87;
/// Ratio of specific heats of air. // PHYS: ISA
pub const GAMMA: f64 = 1.4;
/// Earth radius used for geopotential altitude, m. // PHYS: ISA (GDD 11.3)
pub const R0_GEOPOTENTIAL: f64 = 6_356_766.0;
/// Sea-level standard temperature, K. // PHYS: ISA
pub const T0: f64 = 288.15;
/// Sea-level standard pressure, Pa. // PHYS: ISA
pub const P0: f64 = 101_325.0;
/// Sea-level standard density, kg/m^3. // PHYS: ISA, p0 / (R T0)
pub const RHO0: f64 = 1.225;
/// Troposphere temperature lapse rate, K/m. // PHYS: ISA
pub const LAPSE_TROPOSPHERE: f64 = 0.0065;
/// Lapse rate of the second stratosphere layer (20 to 32 km), K/m. // PHYS: ISA
pub const LAPSE_STRATOSPHERE_2: f64 = -0.001;
/// Geopotential altitude of the tropopause, m. // PHYS: ISA
pub const H_TROPOPAUSE: f64 = 11_000.0;
/// Geopotential altitude where the second stratosphere layer begins, m. // PHYS: ISA
pub const H_STRATOSPHERE_2: f64 = 20_000.0;
/// Top of the modelled atmosphere (geopotential), m. // PHYS: ISA layer table ends here
pub const H_TOP: f64 = 32_000.0;
/// Sea-level standard speed of sound, m/s: `sqrt(GAMMA R_AIR T0)`. // PHYS: ISA
pub const A0: f64 = 340.293_988_026_089;
/// Mean Earth radius for gravity with height and the sphere test world, m. // PHYS: GDD 7.4.3
pub const EARTH_RADIUS_M: f64 = 6_371_000.0;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derived_constants_are_consistent() {
        // rho0 = p0 / (R T0) to the four digits ISA quotes.
        assert!((P0 / (R_AIR * T0) - RHO0).abs() < 5e-5);
        // a0 = sqrt(gamma R T0).
        assert!(((GAMMA * R_AIR * T0).sqrt() - A0).abs() < 1e-9);
    }
}
