//! `DamageModel`: how much a hit hurts (GDD 18, 61.5). The first implementation is a single
//! aggregate health bar (M3); the component and subsystem model replaces it in M4.

/// A hit to apply, described by the energy delivered.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Impact {
    /// Energy delivered to the target, joules. Non-finite or non-positive values do nothing.
    pub energy_j: f64,
}

/// What a hit did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DamageOutcome {
    /// Nothing changed.
    Undamaged,
    /// Health went down but the target survives.
    Damaged,
    /// Health reached zero.
    Destroyed,
}

/// Damage state of one target.
pub trait DamageModel: Send {
    /// Applies a hit.
    fn apply(&mut self, impact: &Impact) -> DamageOutcome;
    /// Remaining health, 0 (destroyed) to 1 (undamaged).
    fn health(&self) -> f64;
}

/// One health bar: `hp` points, each worth `joules_per_hp` joules.
#[derive(Clone, Copy, Debug)]
pub struct AggregateHealth {
    hp: f64,
    max_hp: f64,
    joules_per_hp: f64,
}

impl AggregateHealth {
    /// A fresh bar. Non-positive arguments are replaced by 1.
    #[must_use]
    pub fn new(max_hp: f64, joules_per_hp: f64) -> Self {
        let pos = |v: f64| if v > 0.0 && v.is_finite() { v } else { 1.0 };
        let max_hp = pos(max_hp);
        Self {
            hp: max_hp,
            max_hp,
            joules_per_hp: pos(joules_per_hp),
        }
    }
}

impl DamageModel for AggregateHealth {
    fn apply(&mut self, impact: &Impact) -> DamageOutcome {
        if self.hp <= 0.0 {
            return DamageOutcome::Destroyed;
        }
        if !(impact.energy_j > 0.0 && impact.energy_j.is_finite()) {
            return DamageOutcome::Undamaged;
        }
        self.hp = (self.hp - impact.energy_j / self.joules_per_hp).max(0.0);
        if self.hp <= 0.0 {
            DamageOutcome::Destroyed
        } else {
            DamageOutcome::Damaged
        }
    }

    fn health(&self) -> f64 {
        self.hp / self.max_hp
    }
}

/// Behaviour every `DamageModel` must have (`AC-61.5`). `fresh` makes a new, undamaged model.
///
/// # Panics
/// Panics, with a message naming the property, if the implementation violates the contract.
pub fn contract(fresh: &dyn Fn() -> Box<dyn DamageModel>) {
    let mut m = fresh();
    assert!((m.health() - 1.0).abs() < 1e-12, "starts undamaged");

    for bad in [f64::NAN, f64::INFINITY, -5.0, 0.0] {
        assert_eq!(
            m.apply(&Impact { energy_j: bad }),
            DamageOutcome::Undamaged,
            "ignores {bad}"
        );
    }
    assert!((m.health() - 1.0).abs() < 1e-12, "bad hits change nothing");

    let mut last = m.health();
    let mut destroyed = false;
    for _ in 0..10_000 {
        let outcome = m.apply(&Impact { energy_j: 3.0e5 });
        let h = m.health();
        assert!((0.0..=1.0).contains(&h), "health stays in [0, 1]: {h}");
        assert!(h <= last, "a hit never heals");
        assert_eq!(
            outcome == DamageOutcome::Destroyed,
            h <= 0.0,
            "destroyed exactly at zero health"
        );
        last = h;
        if outcome == DamageOutcome::Destroyed {
            destroyed = true;
            break;
        }
    }
    assert!(destroyed, "enough hits destroy the target");
    assert_eq!(
        m.apply(&Impact { energy_j: 1.0 }),
        DamageOutcome::Destroyed,
        "destroyed stays destroyed"
    );
    assert!(m.health() <= 0.0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aggregate_health_passes_the_contract() {
        contract(&|| Box::new(AggregateHealth::new(100.0, 1.0e5)));
    }

    #[test]
    fn health_falls_in_proportion_to_energy() {
        let mut h = AggregateHealth::new(100.0, 1.0e5);
        assert_eq!(h.apply(&Impact { energy_j: 2.5e6 }), DamageOutcome::Damaged);
        assert!((h.health() - 0.75).abs() < 1e-12);
        assert_eq!(
            h.apply(&Impact { energy_j: 1.0e9 }),
            DamageOutcome::Destroyed
        );
        assert_eq!(h.health(), 0.0);
    }

    #[test]
    fn nonsense_construction_is_repaired() {
        let h = AggregateHealth::new(f64::NAN, -1.0);
        assert!((h.health() - 1.0).abs() < 1e-12);
    }
}
