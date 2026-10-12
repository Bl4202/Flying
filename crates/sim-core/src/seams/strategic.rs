//! `StrategicContext`: the campaign a mission belongs to (GDD 34.6, 61.5). The first
//! implementation is "no campaign": every mission stands alone. The operations layer replaces it
//! after 1.0 without touching the mission generator.

/// Identifies one operation of a campaign.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct OperationId(pub u64);

/// What the mission generator is told about the wider situation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GeneratorContext {
    /// The operation the mission belongs to, if any.
    pub operation: Option<OperationId>,
    /// Seed the generator must use, derived from the mission seed and the campaign state.
    pub seed: u64,
    /// Opposition strength, 0 (none) to 1 (full).
    pub threat: f64,
    /// Friendly support available, 0 (none) to 1 (full).
    pub support: f64,
}

/// How a mission ended, as far as the campaign cares.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Objectives met.
    Success,
    /// Objectives not met.
    Failure,
    /// Left before the end.
    Abandoned,
}

/// The campaign layer as seen from one mission.
pub trait StrategicContext: Send {
    /// The operation in progress, if any.
    fn operation(&self) -> Option<OperationId>;

    /// Context for generating a mission from `mission_seed`. A pure function of the seed and
    /// the campaign state.
    fn generator_context(&self, mission_seed: u64) -> GeneratorContext;

    /// Feeds a result back into the campaign.
    fn record_outcome(&mut self, outcome: Outcome);
}

/// No campaign: missions stand alone, at a middling threat with middling support.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoStrategicContext;

impl StrategicContext for NoStrategicContext {
    fn operation(&self) -> Option<OperationId> {
        None
    }

    fn generator_context(&self, mission_seed: u64) -> GeneratorContext {
        GeneratorContext {
            operation: None,
            seed: mission_seed,
            threat: 0.5,
            support: 0.5,
        }
    }

    fn record_outcome(&mut self, _outcome: Outcome) {}
}

/// Behaviour every `StrategicContext` must have (`AC-61.5`).
///
/// # Panics
/// Panics, with a message naming the property, if the implementation violates the contract.
pub fn contract(s: &mut dyn StrategicContext) {
    for seed in [0, 1, 0xDEAD_BEEF, u64::MAX] {
        let a = s.generator_context(seed);
        assert_eq!(a, s.generator_context(seed), "a pure function of the seed");
        assert_eq!(
            a.operation,
            s.operation(),
            "the context names the current operation"
        );
        assert!(
            (0.0..=1.0).contains(&a.threat),
            "threat in [0, 1]: {}",
            a.threat
        );
        assert!(
            (0.0..=1.0).contains(&a.support),
            "support in [0, 1]: {}",
            a.support
        );
    }
    assert_ne!(
        s.generator_context(1).seed,
        s.generator_context(2).seed,
        "different missions get different seeds"
    );
    for outcome in [Outcome::Success, Outcome::Failure, Outcome::Abandoned] {
        s.record_outcome(outcome);
        let c = s.generator_context(7);
        assert!((0.0..=1.0).contains(&c.threat) && (0.0..=1.0).contains(&c.support));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_campaign_passes_the_contract() {
        contract(&mut NoStrategicContext);
    }

    #[test]
    fn no_campaign_passes_the_seed_through() {
        let c = NoStrategicContext.generator_context(42);
        assert_eq!(c.seed, 42);
        assert_eq!(c.operation, None);
    }
}
