//! Mass properties and fuel feed (GDD 11.9).
//!
//! The airframe is the empty aircraft plus fuel tanks as point masses. Mass, centre of gravity
//! and inertia are recomputed from the tank contents whenever they are needed (once per tick), so
//! the only state is the fuel in each tank. Positions are body axes (x forward, y right, z down)
//! from the airframe's reference point.

use content::aircraft::Mass;
use simmath::math::{DMat3, DVec3};

use crate::error::ModelError;

/// Most fuel tanks an aircraft may have.
pub const MAX_TANKS: usize = 8;

/// Fuel in each tank, kg; entries past the aircraft's tank count stay zero.
pub type FuelLoad = [f64; MAX_TANKS];

/// One fuel tank.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Tank {
    /// Capacity, kg.
    pub capacity_kg: f64,
    /// Centroid, m.
    pub pos_body_m: DVec3,
    /// Feed order: lower feeds first; equal priorities feed together in proportion to contents.
    pub feed_priority: u8,
}

/// Mass, centre of gravity and inertia about it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MassProps {
    /// Total mass, kg.
    pub mass_kg: f64,
    /// Centre of gravity, m from the reference point.
    pub cg_body_m: DVec3,
    /// Inertia tensor about the CG in body axes, kg m^2 (off-diagonal terms are minus the
    /// products of inertia).
    pub inertia_kgm2: DMat3,
    /// Its inverse.
    pub inertia_inv: DMat3,
}

impl MassProps {
    /// Product of inertia `I_xz = integral(x z dm)` about the CG, kg m^2.
    #[must_use]
    pub fn ixz_kgm2(&self) -> f64 {
        -self.inertia_kgm2.m[0][2]
    }
}

/// The mass model of one airframe: immutable, built once from content.
#[derive(Clone, Debug, PartialEq)]
pub struct MassModel {
    empty_kg: f64,
    cg_empty_body_m: DVec3,
    inertia_empty: DMat3,
    tanks: [Tank; MAX_TANKS],
    n_tanks: usize,
    /// Tank indices sorted by feed priority (stable, so equal priorities keep content order).
    feed_order: [usize; MAX_TANKS],
}

impl MassModel {
    /// Builds the model from an aircraft's mass data.
    ///
    /// # Errors
    /// [`ModelError::TooMany`] for more than [`MAX_TANKS`] tanks; [`ModelError::Invalid`] if the
    /// empty mass or inertia is not positive.
    pub fn from_def(def: &Mass) -> Result<Self, ModelError> {
        if def.tanks.len() > MAX_TANKS {
            return Err(ModelError::TooMany {
                what: "fuel tanks",
                found: def.tanks.len(),
                max: MAX_TANKS,
            });
        }
        let v3 = |a: [f32; 3]| DVec3::new(f64::from(a[0]), f64::from(a[1]), f64::from(a[2]));
        let mut tanks = [Tank::default(); MAX_TANKS];
        for (slot, t) in tanks.iter_mut().zip(&def.tanks) {
            *slot = Tank {
                capacity_kg: f64::from(t.capacity_kg),
                pos_body_m: v3(t.pos_m),
                feed_priority: t.feed_priority,
            };
        }
        let n_tanks = def.tanks.len();
        let mut feed_order = [0; MAX_TANKS];
        for (i, slot) in feed_order.iter_mut().enumerate() {
            *slot = i;
        }
        feed_order[..n_tanks].sort_by_key(|&i| tanks[i].feed_priority);
        let [ixx, iyy, izz] = def.inertia_kgm2.map(f64::from);
        let inertia_empty = DMat3::from_inertia(ixx, iyy, izz, f64::from(def.inertia_xz_kgm2));
        let model = Self {
            empty_kg: f64::from(def.empty_kg),
            cg_empty_body_m: v3(def.cg_empty_m),
            inertia_empty,
            tanks,
            n_tanks,
            feed_order,
        };
        let det = inertia_empty.determinant();
        if model.empty_kg.is_nan() || model.empty_kg <= 0.0 || det.is_nan() || det <= 0.0 {
            return Err(ModelError::Invalid("empty mass or inertia"));
        }
        Ok(model)
    }

    /// The tanks.
    #[must_use]
    pub fn tanks(&self) -> &[Tank] {
        &self.tanks[..self.n_tanks]
    }

    /// Empty mass, kg.
    #[must_use]
    pub fn empty_kg(&self) -> f64 {
        self.empty_kg
    }

    /// Total fuel capacity, kg.
    #[must_use]
    pub fn capacity_kg(&self) -> f64 {
        self.tanks().iter().map(|t| t.capacity_kg).sum()
    }

    /// Total fuel in a load, kg.
    #[must_use]
    pub fn fuel_kg(&self, fuel: &FuelLoad) -> f64 {
        fuel[..self.n_tanks].iter().sum()
    }

    /// The groups of tank indices that feed together, in feed order.
    fn groups(&self) -> impl DoubleEndedIterator<Item = &[usize]> {
        self.feed_order[..self.n_tanks]
            .chunk_by(|a, b| self.tanks[*a].feed_priority == self.tanks[*b].feed_priority)
    }

    /// `fuel_kg` of fuel distributed as if full tanks had burned down in feed order: the tanks
    /// that feed last are filled first, a group of equal priority in proportion to capacity.
    /// More than the capacity fills every tank.
    #[must_use]
    pub fn load_for(&self, fuel_kg: f64) -> FuelLoad {
        let mut load = [0.0; MAX_TANKS];
        let mut left = fuel_kg.max(0.0);
        for group in self.groups().rev() {
            let cap: f64 = group.iter().map(|&i| self.tanks[i].capacity_kg).sum();
            if cap <= 0.0 {
                continue;
            }
            let fill = left.min(cap);
            for &i in group {
                load[i] = self.tanks[i].capacity_kg * fill / cap;
            }
            left -= fill;
        }
        load
    }

    /// Draws `demand_kg` from the tanks in feed order and returns what was actually drawn (less
    /// when the fuel runs out). Tanks of equal priority give in proportion to their contents, so
    /// their ratio, and with it the CG, does not drift.
    pub fn draw(&self, fuel: &mut FuelLoad, demand_kg: f64) -> f64 {
        let mut left = demand_kg.max(0.0);
        for group in self.groups() {
            if left <= 0.0 {
                break;
            }
            let have: f64 = group.iter().map(|&i| fuel[i]).sum();
            if have <= 0.0 {
                continue;
            }
            if left < have {
                let keep = (have - left) / have;
                for &i in group {
                    fuel[i] *= keep;
                }
                left = 0.0;
            } else {
                for &i in group {
                    fuel[i] = 0.0;
                }
                left -= have;
            }
        }
        demand_kg.max(0.0) - left
    }

    /// Mass properties for a fuel load (GDD 11.9: parallel-axis sum of the empty airframe and the
    /// tanks as point masses).
    #[must_use]
    pub fn props(&self, fuel: &FuelLoad) -> MassProps {
        let mut mass = self.empty_kg;
        let mut moment = self.cg_empty_body_m * self.empty_kg;
        for (t, &kg) in self.tanks().iter().zip(fuel.iter()) {
            mass += kg;
            moment += t.pos_body_m * kg;
        }
        let cg = moment / mass;
        let point = |m: f64, r: DVec3| {
            let d = r - cg;
            (DMat3::IDENTITY * d.length_squared() - DMat3::outer(d, d)) * m
        };
        let mut inertia = self.inertia_empty + point(self.empty_kg, self.cg_empty_body_m);
        for (t, &kg) in self.tanks().iter().zip(fuel.iter()) {
            inertia = inertia + point(kg, t.pos_body_m);
        }
        MassProps {
            mass_kg: mass,
            cg_body_m: cg,
            inertia_kgm2: inertia,
            inertia_inv: inertia.inverse().unwrap_or(DMat3::ZERO),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use content::aircraft::Tank as TankDef;

    /// The Kestrel's mass data (content/core/aircraft/kestrel.json).
    fn kestrel() -> MassModel {
        let tank = |id: &str, kg: f32, pos: [f32; 3], p: u8| TankDef {
            id: id.into(),
            capacity_kg: kg,
            pos_m: pos,
            feed_priority: p,
        };
        MassModel::from_def(&Mass {
            empty_kg: 8400.0,
            internal_fuel_kg: 3100.0,
            max_takeoff_kg: 14500.0,
            max_landing_kg: 9500.0,
            cg_empty_m: [0.0; 3],
            inertia_kgm2: [13000.0, 72436.0, 82436.0],
            inertia_xz_kgm2: 1300.0,
            fuel_density_kgpl: 0.8,
            tanks: vec![
                tank("fwd", 900.0, [1.65, 0.0, 0.0], 2),
                tank("aft", 1100.0, [-1.35, 0.0, 0.0], 2),
                tank("wing_l", 550.0, [0.0, -2.2, 0.0], 1),
                tank("wing_r", 550.0, [0.0, 2.2, 0.0], 1),
            ],
        })
        .unwrap()
    }

    fn within(actual: f64, expected: f64, rel: f64) -> bool {
        (actual - expected).abs() <= rel * expected.abs()
    }

    #[test]
    fn the_10_tonne_state_matches_appendix_c2() {
        // C.2: 10 000 kg, I = (13 000, 76 000, 86 000) kg m^2, I_xz 1 300 kg m^2, within 1 %.
        let m = kestrel();
        let fuel = m.load_for(1600.0);
        assert_eq!(&fuel[..4], &[720.0, 880.0, 0.0, 0.0], "wings feed first");
        let p = m.props(&fuel);
        assert!((p.mass_kg - 10_000.0).abs() < 1e-9);
        let i = &p.inertia_kgm2.m;
        assert!(within(i[0][0], 13_000.0, 0.01), "Ixx {}", i[0][0]);
        assert!(within(i[1][1], 76_000.0, 0.01), "Iyy {}", i[1][1]);
        assert!(within(i[2][2], 86_000.0, 0.01), "Izz {}", i[2][2]);
        assert!(within(p.ixz_kgm2(), 1300.0, 0.01), "Ixz {}", p.ixz_kgm2());
        // The tank moments balance to the f32 precision of their positions (4e-5 kg m).
        assert!(
            p.cg_body_m.length() < 1e-8,
            "balanced tanks keep the CG put"
        );
        let check = p.inertia_kgm2 * p.inertia_inv;
        assert!(
            (check - DMat3::IDENTITY)
                .m
                .iter()
                .flatten()
                .all(|v| v.abs() < 1e-12)
        );
    }

    #[test]
    fn fuel_feeds_in_priority_order_and_in_proportion() {
        let m = kestrel();
        let mut fuel = m.load_for(m.capacity_kg());
        assert_eq!(&fuel[..4], &[900.0, 1100.0, 550.0, 550.0]);
        // The wing tanks go first, evenly.
        assert_eq!(m.draw(&mut fuel, 500.0), 500.0);
        assert_eq!(&fuel[..4], &[900.0, 1100.0, 300.0, 300.0]);
        // Crossing into the fuselage group: the wings empty, then fwd and aft give 9 : 11.
        assert_eq!(m.draw(&mut fuel, 800.0), 800.0);
        assert_eq!(&fuel[2..4], &[0.0, 0.0]);
        assert!((fuel[0] - 810.0).abs() < 1e-9 && (fuel[1] - 990.0).abs() < 1e-9);
        // Running dry returns what was there.
        let drawn = m.draw(&mut fuel, 5000.0);
        assert!((drawn - 1800.0).abs() < 1e-9);
        assert_eq!(m.fuel_kg(&fuel), 0.0);
        assert_eq!(m.draw(&mut fuel, 1.0), 0.0);
        assert_eq!(m.draw(&mut fuel, -1.0), 0.0);
    }

    #[test]
    fn the_cg_stays_put_and_inertia_stays_positive_definite_through_the_burn() {
        let m = kestrel();
        let mut fuel = m.load_for(m.capacity_kg());
        let mut worst_cg = 0.0_f64;
        loop {
            let p = m.props(&fuel);
            worst_cg = worst_cg.max(p.cg_body_m.length());
            // Positive definite: leading principal minors positive.
            let i = &p.inertia_kgm2.m;
            assert!(i[0][0] > 0.0);
            assert!(i[0][0] * i[1][1] - i[0][1] * i[1][0] > 0.0);
            assert!(p.inertia_kgm2.determinant() > 0.0);
            if m.draw(&mut fuel, 25.0) < 25.0 {
                break;
            }
        }
        // Proportional feed of the balanced fuselage tanks: the CG moves by rounding only.
        assert!(worst_cg < 1e-8, "CG moved {worst_cg} m");
        let empty = m.props(&[0.0; MAX_TANKS]);
        assert_eq!(empty.mass_kg, 8400.0);
        assert_eq!(
            empty.inertia_kgm2,
            DMat3::from_inertia(13000.0, 72436.0, 82436.0, 1300.0)
        );
    }

    #[test]
    fn an_offset_tank_moves_the_cg_and_adds_parallel_axis_inertia() {
        let m = MassModel::from_def(&Mass {
            empty_kg: 1000.0,
            internal_fuel_kg: 100.0,
            max_takeoff_kg: 1100.0,
            max_landing_kg: 1100.0,
            cg_empty_m: [0.0; 3],
            inertia_kgm2: [100.0, 200.0, 300.0],
            inertia_xz_kgm2: 0.0,
            fuel_density_kgpl: 0.8,
            tanks: vec![TankDef {
                id: "t".into(),
                capacity_kg: 100.0,
                pos_m: [1.1, 0.0, 0.0],
                feed_priority: 1,
            }],
        })
        .unwrap();
        let p = m.props(&m.load_for(100.0));
        // CG 0.1 m forward; Iyy gains 1000 * 0.1^2 + 100 * 1.0^2 = 110.
        assert!((p.cg_body_m.x - 0.1).abs() < 1e-8);
        assert!((p.inertia_kgm2.m[1][1] - 310.0).abs() < 1e-5);
        assert!(
            (p.inertia_kgm2.m[0][0] - 100.0).abs() < 1e-12,
            "no lever about x"
        );
    }

    #[test]
    fn bad_mass_data_is_refused() {
        let mut def = Mass {
            empty_kg: 0.0,
            internal_fuel_kg: 0.0,
            max_takeoff_kg: 1.0,
            max_landing_kg: 1.0,
            cg_empty_m: [0.0; 3],
            inertia_kgm2: [1.0; 3],
            inertia_xz_kgm2: 0.0,
            fuel_density_kgpl: 0.8,
            tanks: Vec::new(),
        };
        assert!(MassModel::from_def(&def).is_err());
        def.empty_kg = 1.0;
        assert!(MassModel::from_def(&def).is_ok());
        def.tanks = vec![
            TankDef {
                id: "t".into(),
                capacity_kg: 1.0,
                pos_m: [0.0; 3],
                feed_priority: 1,
            };
            MAX_TANKS + 1
        ];
        assert!(matches!(
            MassModel::from_def(&def),
            Err(ModelError::TooMany { .. })
        ));
    }
}
