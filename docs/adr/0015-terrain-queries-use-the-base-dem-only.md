# ADR-15: Terrain queries use the base DEM only

- **Status:** Accepted
- **Owner:** Bl4202
- **Source:** GDD §0.4 (imported verbatim at v0.0a)
- **Affects:** §44, §46, §16

## Decision

**Terrain collision, radar masking and object placement use only the base DEM** (source-backed levels). Procedural detail displacement is bounded (≤ 1.5 m) and visual-only.

## Alternatives considered

Collide against displayed mesh

## Rationale

Guarantees identical results on every client and server, which compute LOS and collision without rendering (the dedicated server has no GPU).
