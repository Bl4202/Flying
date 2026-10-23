//! Departure diagnostics (GDD 11.15, ADR-38).
//!
//! The dynamic directional stability parameter and the lateral control departure parameter are
//! computed from the aircraft's own lateral tables; they decide nothing at run time. They tell the
//! tuning tools where the airframe diverges, so Reduced and Manual can be left to depart on
//! purpose while Standard and Full keep the aircraft below the angle-of-attack limit.
//!
//! ```text
//! C_n,beta,dyn = C_n,beta cos(a) - (I_z / I_x) C_l,beta sin(a)     negative: directional divergence
//! LCDP         = C_n,beta - C_l,beta C_n,da / C_l,da                negative: aileron reversal
//! ```

use simmath::dmath;

use crate::aero::{LateralAt, TableAero};
use crate::mass::MassProps;

/// `C_n,beta,dyn` for lateral derivatives at `alpha_rad` and the inertia ratio `I_z / I_x`.
#[must_use]
pub fn cn_beta_dyn(lateral: &LateralAt, alpha_rad: f64, iz_over_ix: f64) -> f64 {
    lateral.cn_beta * dmath::cos(alpha_rad) - iz_over_ix * lateral.cl_beta * dmath::sin(alpha_rad)
}

/// The lateral control departure parameter. An aileron with no rolling authority leaves it
/// undefined; `C_n,beta` is returned then.
#[must_use]
pub fn lcdp(lateral: &LateralAt) -> f64 {
    if lateral.cl_da.abs() < 1e-9 {
        return lateral.cn_beta;
    }
    lateral.cn_beta - lateral.cl_beta * lateral.cn_da / lateral.cl_da
}

/// `I_z / I_x` of a mass state.
#[must_use]
pub fn inertia_ratio(mass: &MassProps) -> f64 {
    mass.inertia_kgm2.m[2][2] / mass.inertia_kgm2.m[0][0]
}

/// The departure diagnostics at one angle of attack.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DepartureRow {
    /// Angle of attack, degrees.
    pub alpha_deg: f64,
    /// Dynamic directional stability parameter.
    pub cn_beta_dyn: f64,
    /// Lateral control departure parameter.
    pub lcdp: f64,
}

/// The diagnostics of an airframe at an angle of attack (read at `|alpha|`).
#[must_use]
pub fn row(aero: &TableAero, mass: &MassProps, alpha_deg: f64) -> DepartureRow {
    let t = aero.lateral_at(alpha_deg);
    DepartureRow {
        alpha_deg,
        cn_beta_dyn: cn_beta_dyn(&t, alpha_deg.to_radians(), inertia_ratio(mass)),
        lcdp: lcdp(&t),
    }
}

/// The angle of attack in `[lo_deg, hi_deg]` at which `C_n,beta,dyn` first turns negative,
/// degrees, found by stepping 0.5 degree and refining by bisection. `None` if it never does.
#[must_use]
pub fn divergence_alpha_deg(
    aero: &TableAero,
    mass: &MassProps,
    lo_deg: f64,
    hi_deg: f64,
) -> Option<f64> {
    let f = |a: f64| row(aero, mass, a).cn_beta_dyn;
    let mut a = lo_deg;
    while a < hi_deg {
        let b = (a + 0.5).min(hi_deg);
        if f(a) >= 0.0 && f(b) < 0.0 {
            let (mut lo, mut hi) = (a, b);
            for _ in 0..40 {
                let mid = f64::midpoint(lo, hi);
                if f(mid) >= 0.0 {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            return Some(f64::midpoint(lo, hi));
        }
        a = b;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aircraft::AircraftModel;
    use crate::test_content::registry;

    fn kestrel() -> (AircraftModel, MassProps) {
        let r = registry();
        let m = AircraftModel::from_registry(r, r.aircraft_handles().next().unwrap(), 1.0 / 60.0)
            .unwrap();
        let mass = m.mass.props(&m.mass.load_for(1600.0));
        (m, mass)
    }

    #[test]
    fn the_formulas_on_hand_numbers() {
        // Cn_b 0.14, Cl_b -0.10, I_z/I_x 6.6 at 30 deg: 0.14 * 0.866 + 6.6 * 0.10 * 0.5 = 0.4512.
        let t = LateralAt {
            cy_beta: 0.0,
            cl_beta: -0.10,
            cn_beta: 0.14,
            cl_p: 0.0,
            cl_r: 0.0,
            cn_p: 0.0,
            cn_r: 0.0,
            cl_da: 0.16,
            cn_da: -0.01,
            cl_dr: 0.0,
            cn_dr: 0.0,
            cy_dr: 0.0,
        };
        let v = cn_beta_dyn(&t, 30f64.to_radians(), 6.6);
        assert!((v - (0.14 * 0.866_025_4 + 0.33)).abs() < 1e-6, "{v}");
        // LCDP = 0.14 - (-0.10)(-0.01 / 0.16) = 0.14 - 0.00625.
        assert!((lcdp(&t) - 0.13375).abs() < 1e-12);
        // Adverse yaw large enough to reverse the aileron.
        let rev = LateralAt { cn_da: -0.3, ..t };
        assert!(lcdp(&rev) < 0.0);
        // No aileron authority: the parameter falls back to C_n,beta.
        assert_eq!(lcdp(&LateralAt { cl_da: 0.0, ..t }), 0.14);
    }

    #[test]
    fn the_kestrel_is_directionally_stable_to_30_degrees_and_diverges_near_34() {
        let (m, mass) = kestrel();
        let ratio = inertia_ratio(&mass);
        assert!((6.0..7.0).contains(&ratio), "I_z / I_x = {ratio}");
        for a in [0.0, 5.0, 10.0, 15.0, 20.0, 26.0, 30.0] {
            let r = row(&m.aero, &mass, a);
            assert!(r.cn_beta_dyn > 0.0, "stable at {a}: {r:?}");
        }
        for a in [38.0, 40.0, 50.0, 60.0] {
            let r = row(&m.aero, &mass, a);
            assert!(r.cn_beta_dyn < 0.0, "divergent at {a}: {r:?}");
        }
        let a = divergence_alpha_deg(&m.aero, &mass, 0.0, 90.0).expect("it turns negative");
        println!("C_n,beta,dyn turns negative at {a:.2} deg");
        assert!((32.0..36.0).contains(&a), "{a}");
        // The limiter (26 deg) keeps Standard and Full well clear of it.
        assert!(a > 26.0 + 4.0);
    }

    #[test]
    fn the_tables_read_the_magnitude_of_alpha_and_equal_c5_below_15_degrees() {
        let (m, mass) = kestrel();
        for a in [3.0, 9.0, 14.0] {
            let t = m.aero.lateral_at(a);
            assert_eq!(t, m.aero.lateral_at(-a));
            assert!((t.cn_beta - 0.14).abs() < 1e-6 && (t.cl_beta + 0.10).abs() < 1e-6);
            // cos is even and sin odd: the sign of alpha flips only the roll-coupling term, so
            // the two values sum to twice the first term.
            let (p, n) = (row(&m.aero, &mass, a), row(&m.aero, &mass, -a));
            assert_eq!(p.lcdp, n.lcdp);
            let sum = p.cn_beta_dyn + n.cn_beta_dyn;
            assert!((sum - 2.0 * t.cn_beta * dmath::cos(a.to_radians())).abs() < 1e-12);
        }
    }

    #[test]
    fn the_lateral_control_departure_parameter_over_the_envelope() {
        let (m, mass) = kestrel();
        let low = row(&m.aero, &mass, 5.0);
        assert!((low.lcdp - 0.134).abs() < 0.01, "{low:?}");
        // The aileron loses its authority at high alpha (40 %) and the adverse yaw grows, so the
        // parameter falls; print the sweep for the tuning log.
        for a in [0.0, 10.0, 20.0, 26.0, 30.0, 34.0, 40.0, 60.0, 90.0] {
            let r = row(&m.aero, &mass, a);
            println!(
                "alpha {a:4.0}: Cn_b,dyn {:+.4}  LCDP {:+.4}",
                r.cn_beta_dyn, r.lcdp
            );
        }
        assert!(row(&m.aero, &mass, 40.0).lcdp < low.lcdp);
    }

    #[test]
    fn no_divergence_means_none() {
        let (m, mass) = kestrel();
        assert_eq!(divergence_alpha_deg(&m.aero, &mass, 0.0, 20.0), None);
    }
}
