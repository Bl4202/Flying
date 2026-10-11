# ADR-14: Campaign calls the mission generator

- **Status:** Accepted
- **Owner:** Bl4202
- **Source:** GDD §0.4 (imported verbatim at v0.0a)
- **Affects:** §34–43

## Decision

**The campaign is a strategic layer that calls the same mission generator** (an *Operation* is a generator input set plus persistence rules).

## Alternatives considered

Separate campaign mission pipeline

## Rationale

Outline §6: campaign "added without rebuilding the underlying flight and mission architecture".
