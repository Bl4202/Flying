# ADR-07: Fixed 60 Hz simulation tick

- **Status:** Accepted
- **Owner:** Bl4202
- **Source:** GDD §0.4 (imported verbatim at v0.0a)
- **Affects:** §11, §19, §33, §60

## Decision

**Fixed 60 Hz simulation tick** (Δt = 1/60 s) with multi-rate sub-systems (§0.6).

## Alternatives considered

Variable timestep; fixed 30 Hz; fixed 120 Hz global

## Rationale

60 Hz bounds latency, matches common refresh rates, and gives 16.7 ms of sim time per tick; 120 Hz only where needed (Tier-0 flight dynamics and terminal-phase weapons).
