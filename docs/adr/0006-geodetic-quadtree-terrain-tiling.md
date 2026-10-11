# ADR-06: Geodetic quadtree terrain tiling

- **Status:** Accepted
- **Owner:** Bl4202
- **Source:** GDD §0.4 (imported verbatim at v0.0a)
- **Affects:** §44, §45, §46

## Decision

**Terrain tiling: geodetic (lat/lon) quadtree**, level 0 = two 180° × 180° tiles; tiles carry 129 × 129 height posts (128 intervals) plus skirts. Deepest source-backed level is **z13** (≈19.1 m post spacing at the equator, consistent with 30 m source DEMs). Levels z14+ are procedural amplification only (visual).

## Alternatives considered

Web Mercator quadtree (poles unusable); cube-sphere quadtree (uniform, but costlier tooling and re-projection)

## Rationale

Maps directly to lat/lon search and DEM sources; poles representable (to ±85° at launch, §45.8). Known inefficiency: E–W oversampling at high latitude (factor 1/cos φ), mitigated by LOD selection on the *larger* metric dimension.
