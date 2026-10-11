# ADR-05: MSL-as-ellipsoid vertical datum

- **Status:** Accepted
- **Owner:** Bl4202
- **Source:** GDD §0.4 (imported verbatim at v0.0a)
- **Affects:** §7, §45

## Decision

**Vertical datum: MSL height is treated directly as height above the WGS84 ellipsoid** ("MSL-as-ellipsoid"). No geoid undulation correction is applied by default.

## Alternatives considered

Full EGM2008 geoid correction

## Rationale

DEMs, bathymetry and airport elevations are all MSL-referenced, so terrain, ocean surface, runways and aircraft altitude share one consistent surface. The geoid-ellipsoid separation (roughly −106 m … +85 m worldwide) would only matter when comparing against GNSS ellipsoidal heights, which the game never does. Residual inter-dataset datum differences (EGM96 vs EGM2008, typically on the order of a metre) are recorded per dataset and normalised in the pipeline (§45.6).
