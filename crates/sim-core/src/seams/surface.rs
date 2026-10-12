//! `SurfaceSim`: ships and ground vehicles (GDD 61.5). The first implementation is a set of
//! units that stay where they were placed; route-following convoys and ship motion replace it in
//! M3.

use crate::math::DVec3;

/// One ship or vehicle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SurfaceUnit {
    /// Stable identifier, unique within the simulation.
    pub id: u32,
    /// Position, BF.
    pub pos_bf: DVec3,
    /// Compass heading, radians, 0 north, clockwise.
    pub heading_rad: f64,
    /// Speed over the surface, metres per second.
    pub speed_mps: f64,
}

/// The surface units of a mission.
pub trait SurfaceSim: Send {
    /// Advances every unit by `dt` seconds. A negative or non-finite `dt` does nothing.
    fn step(&mut self, dt: f64);

    /// The units, in ascending id order.
    fn units(&self) -> &[SurfaceUnit];
}

/// Units that never move.
#[derive(Clone, Debug, Default)]
pub struct StaticShips {
    units: Vec<SurfaceUnit>,
}

impl StaticShips {
    /// Places units. They are stopped, sorted by id, and any with a non-finite position or a
    /// repeated id are left out.
    #[must_use]
    pub fn new(mut units: Vec<SurfaceUnit>) -> Self {
        units.retain(|u| u.pos_bf.is_finite());
        units.sort_by_key(|u| u.id);
        units.dedup_by_key(|u| u.id);
        for u in &mut units {
            u.speed_mps = 0.0;
            if !u.heading_rad.is_finite() {
                u.heading_rad = 0.0;
            }
        }
        Self { units }
    }
}

impl SurfaceSim for StaticShips {
    fn step(&mut self, _dt: f64) {}

    fn units(&self) -> &[SurfaceUnit] {
        &self.units
    }
}

/// Behaviour every `SurfaceSim` must have (`AC-61.5`). `fresh` makes a new simulation.
///
/// # Panics
/// Panics, with a message naming the property, if the implementation violates the contract.
pub fn contract(fresh: &dyn Fn() -> Box<dyn SurfaceSim>) {
    let check = |s: &dyn SurfaceSim| {
        let u = s.units();
        assert!(
            u.windows(2).all(|w| w[0].id < w[1].id),
            "ids ascend and are unique"
        );
        for unit in u {
            assert!(
                unit.pos_bf.is_finite(),
                "unit {} has a finite position",
                unit.id
            );
            assert!(
                unit.heading_rad.is_finite(),
                "unit {} has a finite heading",
                unit.id
            );
            assert!(
                unit.speed_mps >= 0.0 && unit.speed_mps < 60.0,
                "unit {} speed",
                unit.id
            );
        }
    };

    let mut a = fresh();
    let mut b = fresh();
    check(&*a);
    assert_eq!(a.units(), b.units(), "two fresh simulations start the same");

    // Bad time steps change nothing.
    let before = a.units().to_vec();
    for bad in [f64::NAN, f64::INFINITY, -1.0] {
        a.step(bad);
    }
    assert_eq!(a.units(), &before[..], "bad time steps are ignored");

    // Nothing moves faster than its own speed, and the set of units is stable.
    let dt = 1.0 / 60.0;
    for _ in 0..600 {
        let prev = a.units().to_vec();
        a.step(dt);
        b.step(dt);
        check(&*a);
        assert_eq!(
            a.units().len(),
            prev.len(),
            "units do not appear or vanish in a step"
        );
        for (now, was) in a.units().iter().zip(&prev) {
            assert_eq!(now.id, was.id);
            let moved = (now.pos_bf - was.pos_bf).length();
            assert!(
                moved <= was.speed_mps.max(now.speed_mps) * dt + 1e-9,
                "unit {} moved {moved} m in one step",
                now.id
            );
        }
    }
    assert_eq!(a.units(), b.units(), "the same steps give the same result");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ship(id: u32, x: f64) -> SurfaceUnit {
        SurfaceUnit {
            id,
            pos_bf: DVec3::new(x, 20_000.0, 0.0),
            heading_rad: 1.0,
            speed_mps: 12.0,
        }
    }

    #[test]
    fn static_ships_pass_the_contract() {
        contract(&|| {
            Box::new(StaticShips::new(vec![
                ship(3, 0.0),
                ship(1, 500.0),
                ship(2, 900.0),
            ]))
        });
        contract(&|| Box::new(StaticShips::default()));
    }

    #[test]
    fn construction_sorts_stops_and_cleans() {
        let s = StaticShips::new(vec![
            ship(9, 0.0),
            ship(2, 1.0),
            ship(9, 5.0),
            ship(4, f64::NAN),
        ]);
        let ids: Vec<u32> = s.units().iter().map(|u| u.id).collect();
        assert_eq!(ids, [2, 9]);
        assert!(s.units().iter().all(|u| u.speed_mps == 0.0));
    }

    #[test]
    fn static_ships_do_not_move() {
        let mut s = StaticShips::new(vec![ship(1, 100.0)]);
        let before = s.units()[0];
        s.step(3_600.0);
        assert_eq!(s.units()[0], before);
    }
}
