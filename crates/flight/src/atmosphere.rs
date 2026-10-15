//! International Standard Atmosphere with non-standard days (GDD 11.3, B.2, `AC-11.5`).
//!
//! [`standard_geopotential`] is the analytic model and the test oracle. [`Atmosphere`] is the hot
//! path: a 1 025-node table over geopotential altitude 0 to 32 km, linearly interpolated. The node
//! spacing (31.25 m) is exact in binary and puts nodes on both layer boundaries (11 and 20 km), so
//! temperature interpolation is exact and pressure interpolation errs by about 3e-6 relative.
//!
//! Inputs are geometric heights above MSL (`h_msl_m`); the conversion to geopotential altitude
//! `H = R0 h / (R0 + h)` happens here, once.

use simmath::dmath;

use crate::physical::{
    G0, GAMMA, H_STRATOSPHERE_2, H_TOP, H_TROPOPAUSE, LAPSE_STRATOSPHERE_2, LAPSE_TROPOSPHERE, P0,
    R_AIR, R0_GEOPOTENTIAL, T0,
};

/// Temperature of the isothermal layer (11 to 20 km), K.
const T_TROPOPAUSE: f64 = T0 - LAPSE_TROPOSPHERE * H_TROPOPAUSE;
/// Node count of the lookup table. // ALGO: 1 024 intervals of 31.25 m over 0 to 32 km
const NODES: usize = 1_025;
/// Node spacing, m.
const STEP_M: f64 = H_TOP / (NODES - 1) as f64;

/// The air at one point.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AtmoSample {
    /// Static temperature, K.
    pub t_k: f64,
    /// Static pressure, Pa.
    pub p_pa: f64,
    /// Density, kg/m^3.
    pub rho_kgpm3: f64,
    /// Speed of sound, m/s.
    pub a_mps: f64,
}

impl AtmoSample {
    fn from_t_p(t_k: f64, p_pa: f64) -> Self {
        Self {
            t_k,
            p_pa,
            rho_kgpm3: p_pa / (R_AIR * t_k),
            a_mps: dmath::sqrt(GAMMA * R_AIR * t_k),
        }
    }
}

/// Weather's departure from the standard day (GDD 11.3): `T = T_ISA(H) + dT`,
/// `p = p_ISA(H) * p0' / 101 325`. The pressure profile keeps its ISA shape.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NonStandardDay {
    /// Temperature offset, K (GDD range -40 to +40).
    pub delta_t_k: f64,
    /// Sea-level pressure, Pa (GDD range 95 000 to 105 000).
    pub p0_pa: f64,
}

impl NonStandardDay {
    /// The standard day.
    pub const ISA: Self = Self {
        delta_t_k: 0.0,
        p0_pa: P0,
    };
}

impl Default for NonStandardDay {
    fn default() -> Self {
        Self::ISA
    }
}

/// Geopotential altitude for a geometric height, m.
#[must_use]
pub fn geopotential_m(h_m: f64) -> f64 {
    R0_GEOPOTENTIAL * h_m / (R0_GEOPOTENTIAL + h_m)
}

/// Exponent of the troposphere pressure law, `g0 / (R L)` (about 5.255 88).
fn troposphere_exponent() -> f64 {
    G0 / (R_AIR * LAPSE_TROPOSPHERE)
}

/// Standard temperature and pressure at a geopotential altitude, analytic (the oracle).
///
/// Three layers to 32 km; the troposphere law continues below 0 m and the values are held
/// constant above 32 km. Layer base pressures are derived from the layer below, so the profile
/// is continuous (the rounded constants quoted in GDD 11.3 agree to better than 1e-6).
#[must_use]
pub fn standard_geopotential(h_geopot_m: f64) -> (f64, f64) {
    if h_geopot_m.is_nan() {
        // `f64::min` would turn NaN into the ceiling and hide the fault.
        return (f64::NAN, f64::NAN);
    }
    let h = h_geopot_m.min(H_TOP);
    let p_tropopause = P0 * dmath::pow(T_TROPOPAUSE / T0, troposphere_exponent());
    if h <= H_TROPOPAUSE {
        let t = T0 - LAPSE_TROPOSPHERE * h;
        return (t, P0 * dmath::pow(t / T0, troposphere_exponent()));
    }
    let isothermal = |dh: f64| p_tropopause * dmath::exp(-G0 * dh / (R_AIR * T_TROPOPAUSE));
    if h <= H_STRATOSPHERE_2 {
        return (T_TROPOPAUSE, isothermal(h - H_TROPOPAUSE));
    }
    let p20 = isothermal(H_STRATOSPHERE_2 - H_TROPOPAUSE);
    let t = T_TROPOPAUSE - LAPSE_STRATOSPHERE_2 * (h - H_STRATOSPHERE_2);
    let exponent = G0 / (R_AIR * LAPSE_STRATOSPHERE_2);
    (t, p20 * dmath::pow(t / T_TROPOPAUSE, exponent))
}

/// The air at a geometric height above MSL on a given day, computed analytically.
#[must_use]
pub fn sample_analytic(h_msl_m: f64, day: NonStandardDay) -> AtmoSample {
    let (t, p) = standard_geopotential(geopotential_m(h_msl_m));
    apply_day(t, p, day)
}

fn apply_day(t_std: f64, p_std: f64, day: NonStandardDay) -> AtmoSample {
    AtmoSample::from_t_p(t_std + day.delta_t_k, p_std * (day.p0_pa / P0))
}

/// Density altitude, m (geopotential): the standard-day altitude with the same density (GDD B.2).
#[must_use]
pub fn density_altitude_m(rho_kgpm3: f64) -> f64 {
    let (t11, p11) = standard_geopotential(H_TROPOPAUSE);
    let rho11 = p11 / (R_AIR * t11);
    if rho_kgpm3 >= rho11 {
        // rho / rho0 = (T / T0)^(n - 1) in the troposphere.
        let inv = 1.0 / (troposphere_exponent() - 1.0);
        (T0 / LAPSE_TROPOSPHERE) * (1.0 - dmath::pow(rho_kgpm3 / rho0_exact(), inv))
    } else {
        H_TROPOPAUSE - (R_AIR * t11 / G0) * dmath::ln(rho_kgpm3 / rho11)
    }
}

/// `p0 / (R T0)`; ISA quotes it rounded as `physical::RHO0`.
fn rho0_exact() -> f64 {
    P0 / (R_AIR * T0)
}

/// The atmosphere used on hot paths: a lookup table over geopotential altitude.
///
/// Built once (it allocates 16 KiB); sampling allocates nothing.
#[derive(Clone, Debug)]
pub struct Atmosphere {
    t_k: Box<[f64]>,
    p_pa: Box<[f64]>,
}

impl Default for Atmosphere {
    fn default() -> Self {
        Self::new()
    }
}

impl Atmosphere {
    /// Builds the table from the analytic model.
    #[must_use]
    pub fn new() -> Self {
        let mut t_k = vec![0.0; NODES];
        let mut p_pa = vec![0.0; NODES];
        for i in 0..NODES {
            let (t, p) = standard_geopotential(i as f64 * STEP_M);
            t_k[i] = t;
            p_pa[i] = p;
        }
        Self {
            t_k: t_k.into_boxed_slice(),
            p_pa: p_pa.into_boxed_slice(),
        }
    }

    /// Standard temperature and pressure at a geopotential altitude, from the table. Below 0 m
    /// the analytic troposphere is used; above 32 km the values are held.
    #[must_use]
    pub fn standard(&self, h_geopot_m: f64) -> (f64, f64) {
        if h_geopot_m < 0.0 || h_geopot_m.is_nan() {
            // Below sea level, or NaN: the analytic law (NaN stays NaN; the caller catches it).
            return standard_geopotential(h_geopot_m);
        }
        let x = h_geopot_m.min(H_TOP) / STEP_M;
        let i = (x as usize).min(NODES - 2);
        let f = x - i as f64;
        let t = self.t_k[i] + (self.t_k[i + 1] - self.t_k[i]) * f;
        let p = self.p_pa[i] + (self.p_pa[i + 1] - self.p_pa[i]) * f;
        (t, p)
    }

    /// The air at a geometric height above MSL on a given day.
    #[must_use]
    pub fn sample(&self, h_msl_m: f64, day: NonStandardDay) -> AtmoSample {
        let (t, p) = self.standard(geopotential_m(h_msl_m));
        apply_day(t, p, day)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::physical::RHO0;

    fn rel(a: f64, b: f64) -> f64 {
        ((a - b) / b).abs()
    }

    /// GDD 11.3 reference table, geopotential altitude: H, T, p, rho, a.
    const TABLE: [(f64, f64, f64, f64, f64); 9] = [
        (0.0, 288.15, 101_325.0, 1.2250, 340.29),
        (1_000.0, 281.65, 89_874.6, 1.1116, 336.43),
        (3_000.0, 268.65, 70_108.5, 0.9091, 328.58),
        (5_000.0, 255.65, 54_019.9, 0.7361, 320.53),
        (8_000.0, 236.15, 35_599.8, 0.5252, 308.06),
        (10_000.0, 223.15, 26_436.2, 0.4127, 299.46),
        (11_000.0, 216.65, 22_632.0, 0.3639, 295.07),
        (15_000.0, 216.65, 12_044.6, 0.1937, 295.07),
        (20_000.0, 216.65, 5_474.9, 0.0880, 295.07),
    ];

    #[test]
    fn ac_11_5_reference_table_within_0_05_percent() {
        let atmo = Atmosphere::new();
        for (h, t, p, rho, a) in TABLE {
            for (name, (tt, pp)) in [
                ("analytic", standard_geopotential(h)),
                ("table", atmo.standard(h)),
            ] {
                let s = AtmoSample::from_t_p(tt, pp);
                assert!(rel(s.t_k, t) < 5e-4, "{name} T at {h}: {}", s.t_k);
                assert!(rel(s.p_pa, p) < 5e-4, "{name} p at {h}: {}", s.p_pa);
                assert!(
                    rel(s.rho_kgpm3, rho) < 5e-4,
                    "{name} rho at {h}: {}",
                    s.rho_kgpm3
                );
                assert!(rel(s.a_mps, a) < 5e-4, "{name} a at {h}: {}", s.a_mps);
            }
        }
    }

    #[test]
    fn table_tracks_the_analytic_model_within_0_01_percent() {
        let atmo = Atmosphere::new();
        let mut worst = 0.0_f64;
        for i in 0..=100_000 {
            let h = H_TOP * f64::from(i) / 100_000.0;
            let (ta, pa) = standard_geopotential(h);
            let (tt, pt) = atmo.standard(h);
            worst = worst.max(rel(tt, ta)).max(rel(pt, pa));
        }
        assert!(worst < 1e-4, "worst relative error {worst:e}");
        // Nodes fall on the layer boundaries, so the error there is zero.
        for h in [0.0, H_TROPOPAUSE, H_STRATOSPHERE_2, H_TOP] {
            assert_eq!(atmo.standard(h), standard_geopotential(h), "node at {h}");
        }
    }

    #[test]
    fn profile_is_continuous_across_layers() {
        for h in [H_TROPOPAUSE, H_STRATOSPHERE_2] {
            let (t_lo, p_lo) = standard_geopotential(h - 1e-6);
            let (t_hi, p_hi) = standard_geopotential(h + 1e-6);
            assert!((t_lo - t_hi).abs() < 1e-6, "T jump at {h}");
            assert!(rel(p_lo, p_hi) < 1e-9, "p jump at {h}");
        }
    }

    #[test]
    fn geometric_height_is_converted_to_geopotential() {
        // GDD 11.3: 10 000 m geometric is 9 984 m geopotential, density 0.4135 kg/m^3.
        assert!((geopotential_m(10_000.0) - 9_984.3).abs() < 0.1);
        let s = sample_analytic(10_000.0, NonStandardDay::ISA);
        assert!(rel(s.rho_kgpm3, 0.4135) < 5e-4);
        let atmo = Atmosphere::new();
        assert!(rel(atmo.sample(10_000.0, NonStandardDay::ISA).rho_kgpm3, 0.4135) < 5e-4);
    }

    #[test]
    fn outside_the_table_the_values_are_defined() {
        let atmo = Atmosphere::new();
        // Below sea level the troposphere law continues (Dead Sea, -430 m).
        let (t, p) = atmo.standard(-430.0);
        assert!(t > T0 && p > P0);
        // Above 32 km the values are held.
        assert_eq!(atmo.standard(45_000.0), atmo.standard(H_TOP));
        assert!(atmo.standard(f64::NAN).0.is_nan());
    }

    #[test]
    fn non_standard_day_shifts_temperature_and_scales_pressure() {
        let hot = NonStandardDay {
            delta_t_k: 20.0,
            p0_pa: P0,
        };
        let s = sample_analytic(0.0, hot);
        assert_eq!(s.t_k, 308.15);
        assert_eq!(s.p_pa, P0);
        assert!(rel(s.rho_kgpm3, 1.1455) < 1e-4);
        let low = NonStandardDay {
            delta_t_k: 0.0,
            p0_pa: 98_000.0,
        };
        let (_, p_std) = standard_geopotential(3_000.0);
        let s = sample_analytic(geometric_from_geopotential(3_000.0), low);
        assert!(rel(s.p_pa, p_std * 98_000.0 / P0) < 1e-12);
    }

    fn geometric_from_geopotential(h: f64) -> f64 {
        R0_GEOPOTENTIAL * h / (R0_GEOPOTENTIAL - h)
    }

    #[test]
    fn density_altitude_inverts_the_standard_density() {
        for h in [0.0, 1_500.0, 5_000.0, 10_000.0, 13_000.0, 18_000.0] {
            let (t, p) = standard_geopotential(h);
            let rho = p / (R_AIR * t);
            assert!(
                (density_altitude_m(rho) - h).abs() < 0.01,
                "density altitude at {h}: {}",
                density_altitude_m(rho)
            );
        }
        // ISA+20 at sea level: about 700 m of density altitude.
        let hot = sample_analytic(
            0.0,
            NonStandardDay {
                delta_t_k: 20.0,
                p0_pa: P0,
            },
        );
        let da = density_altitude_m(hot.rho_kgpm3);
        assert!((da - 700.0).abs() < 60.0, "{da}");
        // The quoted rho0 is the rounded p0 / (R T0).
        assert!(rel(rho0_exact(), RHO0) < 5e-5);
    }
}
