//! Structural limits and their warnings (GDD 11.15, ADR-38).
//!
//! Per configuration the aircraft has a load limit (`n_max`, lower with the gear or the flaps
//! out), a negative one (`n_min`), a maximum operating speed and Mach number, and the speeds
//! above which the gear and the flaps may not be out. Exceeding a limit raises a flag for the
//! warning strip; load above 1.1 times the limit also accumulates `overstress_s`, the damage
//! clock. Nothing breaks in v0.1b: break-up and the damage model belong to GDD 18 (v0.3c).
//!
//! The model is a pure function of the tick's air data, so it is the same on every machine.

use crate::aircraft::Limits;
use crate::gear::GEAR_UP_MAX;

/// Load beyond this multiple of the limit counts as overstress. // DATA: GDD 11.15
pub const OVERSTRESS_FACTOR: f64 = 1.1;
/// Flaps count as out from this fraction of their travel. // ALGO: ADR-38
pub const FLAPS_OUT_MIN: f64 = 0.01;

impl Limits {
    /// The positive load limit for a configuration, g: the lowest of the clean limit and those of
    /// whatever is out. `gear_pos` and `flap_fraction` run 0 (in) to 1 (out).
    #[must_use]
    pub fn n_max_for(&self, gear_pos: f64, flap_fraction: f64) -> f64 {
        let mut n = self.n_max;
        if gear_pos > GEAR_UP_MAX {
            n = n.min(self.n_max_gear);
        }
        if flap_fraction > FLAPS_OUT_MIN {
            n = n.min(self.n_max_flaps);
        }
        n
    }
}

/// What the structure model reads each tick.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StructureInput {
    /// Load factor at the CG (accelerometer `n_z`), g.
    pub nz_g: f64,
    /// Calibrated airspeed, m/s.
    pub cas_mps: f64,
    /// Mach number.
    pub mach: f64,
    /// Gear extension, 0 to 1.
    pub gear_pos: f64,
    /// Trailing-edge flap travel used, 0 to 1.
    pub flap_fraction: f64,
}

/// What is wrong this tick.
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent warnings, one per snapshot flag bit"
)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StructureFlags {
    /// Load beyond 1.1 times the limit for the configuration.
    pub overstress: bool,
    /// Beyond the maximum operating speed or Mach number.
    pub overspeed: bool,
    /// Gear out above its speed limit.
    pub gear_overspeed: bool,
    /// Flaps out above their speed limit.
    pub flap_overspeed: bool,
}

/// What the structure has been through.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StructureState {
    /// Time spent beyond 1.1 times the load limit, s.
    pub overstress_s: f64,
    /// Highest positive load factor seen, g.
    pub max_n_g: f64,
    /// Time spent beyond the maximum operating speed or Mach, s.
    pub overspeed_s: f64,
}

impl StructureState {
    /// A fresh airframe.
    pub const NEW: Self = Self {
        overstress_s: 0.0,
        max_n_g: 0.0,
        overspeed_s: 0.0,
    };

    /// Advances the clocks by `dt_s` and reports what is wrong. Non-finite input is ignored.
    pub fn step(&mut self, limits: &Limits, i: &StructureInput, dt_s: f64) -> StructureFlags {
        let mut flags = StructureFlags::default();
        if i.nz_g.is_finite() {
            self.max_n_g = self.max_n_g.max(i.nz_g);
            let over = if i.nz_g >= 0.0 {
                i.nz_g > OVERSTRESS_FACTOR * limits.n_max_for(i.gear_pos, i.flap_fraction)
            } else {
                -i.nz_g > OVERSTRESS_FACTOR * limits.n_min.abs()
            };
            if over {
                self.overstress_s += dt_s;
            }
            flags.overstress = over;
        }
        if i.cas_mps.is_finite() && i.mach.is_finite() {
            flags.overspeed = i.cas_mps > limits.vmo_mps || i.mach > limits.mmo;
            if flags.overspeed {
                self.overspeed_s += dt_s;
            }
            flags.gear_overspeed = i.gear_pos > GEAR_UP_MAX && i.cas_mps > limits.gear_limit_mps;
            flags.flap_overspeed =
                i.flap_fraction > FLAPS_OUT_MIN && i.cas_mps > limits.flap_limit_mps;
        }
        flags
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f64 = 1.0 / 60.0;

    /// The Kestrel's limits (content/core/aircraft/kestrel.json).
    fn limits() -> Limits {
        Limits {
            n_max: 9.0,
            n_min: -3.0,
            n_max_gear: 4.0,
            n_max_flaps: 4.0,
            alpha_limit_rad: 0.45,
            roll_rate_max_rps: 4.0,
            mmo: 2.0,
            vmo_mps: 360.0,
            gear_limit_mps: 128.6,
            flap_limit_mps: 154.3,
        }
    }

    fn clean(nz_g: f64) -> StructureInput {
        StructureInput {
            nz_g,
            cas_mps: 200.0,
            mach: 0.6,
            ..StructureInput::default()
        }
    }

    #[test]
    fn the_load_limit_follows_the_configuration() {
        let l = limits();
        assert_eq!(l.n_max_for(0.0, 0.0), 9.0);
        assert_eq!(l.n_max_for(1.0, 0.0), 4.0);
        assert_eq!(l.n_max_for(0.0, 0.5), 4.0);
        assert_eq!(l.n_max_for(0.5, 1.0), 4.0, "gear in transit counts as out");
        assert_eq!(
            l.n_max_for(GEAR_UP_MAX, FLAPS_OUT_MIN),
            9.0,
            "the thresholds are open"
        );
    }

    #[test]
    fn overstress_starts_at_one_point_one_times_the_limit_and_accumulates() {
        let l = limits();
        let mut s = StructureState::NEW;
        assert!(
            !s.step(&l, &clean(9.9), DT).overstress,
            "1.1 n_max exactly is allowed"
        );
        assert!(s.step(&l, &clean(9.95), DT).overstress);
        for _ in 0..59 {
            s.step(&l, &clean(10.5), DT);
        }
        assert!(
            (s.overstress_s - 60.0 * DT).abs() < 1e-12,
            "{}",
            s.overstress_s
        );
        // Damage does not heal.
        s.step(&l, &clean(1.0), DT);
        assert!((s.overstress_s - 60.0 * DT).abs() < 1e-12);
        assert_eq!(s.max_n_g, 10.5);
    }

    #[test]
    fn with_the_gear_out_the_limit_is_four() {
        let l = limits();
        let mut s = StructureState::NEW;
        let gear = StructureInput {
            gear_pos: 1.0,
            ..clean(4.5)
        };
        assert!(s.step(&l, &gear, DT).overstress);
        assert!(
            !s.step(&l, &StructureInput { nz_g: 4.4, ..gear }, DT)
                .overstress
        );
        assert!(!s.step(&l, &clean(4.5), DT).overstress, "clean it is fine");
    }

    #[test]
    fn negative_load_is_judged_against_n_min() {
        let l = limits();
        let mut s = StructureState::NEW;
        assert!(!s.step(&l, &clean(-3.3), DT).overstress);
        assert!(s.step(&l, &clean(-3.4), DT).overstress);
        assert_eq!(
            s.max_n_g, 0.0,
            "the positive maximum is not lowered by negative load"
        );
    }

    #[test]
    fn operating_speed_gear_speed_and_flap_speed_each_have_a_flag() {
        let l = limits();
        let mut s = StructureState::NEW;
        let f = s.step(
            &l,
            &StructureInput {
                cas_mps: 361.0,
                ..clean(1.0)
            },
            DT,
        );
        assert!(f.overspeed && !f.gear_overspeed && !f.flap_overspeed);
        let f = s.step(
            &l,
            &StructureInput {
                mach: 2.1,
                ..clean(1.0)
            },
            DT,
        );
        assert!(f.overspeed);
        let f = s.step(
            &l,
            &StructureInput {
                cas_mps: 130.0,
                gear_pos: 1.0,
                ..clean(1.0)
            },
            DT,
        );
        assert!(f.gear_overspeed && !f.overspeed && !f.flap_overspeed);
        let f = s.step(
            &l,
            &StructureInput {
                cas_mps: 130.0,
                flap_fraction: 1.0,
                ..clean(1.0)
            },
            DT,
        );
        assert!(!f.flap_overspeed, "flaps are good to 154.3");
        let f = s.step(
            &l,
            &StructureInput {
                cas_mps: 160.0,
                flap_fraction: 0.5,
                ..clean(1.0)
            },
            DT,
        );
        assert!(f.flap_overspeed && !f.gear_overspeed);
        assert!((s.overspeed_s - 2.0 * DT).abs() < 1e-12);
        // Gear up and flaps up, nothing at 130 m/s.
        assert_eq!(s.step(&l, &clean(1.0), DT), StructureFlags::default());
    }

    #[test]
    fn non_finite_input_changes_nothing() {
        let l = limits();
        let mut s = StructureState::NEW;
        let bad = StructureInput {
            nz_g: f64::NAN,
            cas_mps: f64::NAN,
            mach: f64::INFINITY,
            ..StructureInput::default()
        };
        assert_eq!(s.step(&l, &bad, DT), StructureFlags::default());
        assert_eq!(s, StructureState::NEW);
    }
}
