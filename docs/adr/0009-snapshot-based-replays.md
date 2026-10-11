# ADR-09: Snapshot-based replays

- **Status:** Accepted
- **Owner:** Bl4202
- **Source:** GDD §0.4 (imported verbatim at v0.0a)
- **Affects:** §52

## Decision

**Replays are snapshot-based** (periodic entity states + discrete events), not deterministic re-simulation.

## Alternatives considered

Input-recording deterministic replay

## Rationale

Works across versions of non-gameplay code, allows free scrubbing/seeking, tolerant of tick-level changes.
