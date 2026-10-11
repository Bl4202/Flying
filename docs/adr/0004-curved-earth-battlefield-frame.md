# ADR-04: Curved-Earth battlefield frame

- **Status:** Accepted
- **Owner:** Bl4202
- **Source:** GDD §0.4 (imported verbatim at v0.0a)
- **Affects:** §7, §16, §24, §44

## Decision

**BF is a curved-Earth frame** (rigid ECEF transform), not a flat tangent plane.

## Alternatives considered

Flat ENU plane with curvature "faked" in sensors; azimuthal projection

## Rationale

At 300 km a flat plane omits 7.06 km of Earth drop (500 km: 19.6 km). That error changes the horizon, radar line-of-sight, terrain masking, and low-level visibility — core gameplay (outline §28, §139).
