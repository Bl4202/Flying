# ADR-16: Sensors and AI never read the render world

- **Status:** Accepted
- **Owner:** Bl4202
- **Source:** GDD §0.4 (imported verbatim at v0.0a)
- **Affects:** §16, §19, §61

## Decision

**Gameplay sensors and AI never read the render world.** They use sim-side queries (terrain height service, LOS service, spatial hash).

## Alternatives considered

Raycasts against render geometry

## Rationale

Headless servers; performance; authority.
