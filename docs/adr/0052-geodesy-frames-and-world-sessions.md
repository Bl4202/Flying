# ADR-52: Geodesy, the WGS84 battlefield frame and world sessions

- **Status:** Proposed
- **Owner:** Bl4202
- **Date:** 2026-11-13
- **Affects:** GDD 7.2–7.9, 44.3, 44.6, 61.5; `crates/geo`, `crates/sim-core` (world sessions, added
  in v0.2a's WP5), `crates/sim-wasm`

## Context

Until v0.2a the simulation had no WGS84 code. The two test arenas of ADR-43 (`rugged`, `plain`) put
the battlefield frame on a sphere of R = 6 371 km (`ProceduralGeo`, spherical geocentric
`bf_to_geo`), and their five determinism goldens depend on it bit for bit.

v0.2a puts real places on the Earth (roadmap 7.1). GDD 7.4.4 requires the ellipsoid: "the ellipsoid
is *not* replaced by a sphere anywhere in the sim", because a sphere of curvature differs from the
ellipsoid by about 66 m in height at 500 km. The gates of ADR-51 (GE1 to GE9) fix the accuracy of the
geodesy and require identical results natively and in WebAssembly (`AC-7.4`).

This ADR has two parts:

- the geodesy, written in WP1 (this version of the ADR);
- world sessions in the core: `EarthGeo`, the sim terrain, ABI v6, snapshot v6 and the world
  clock. That part is added in WP5.

## Decision

### Two geodesy worlds

- The test arenas keep `ProceduralGeo` on the R = 6 371 km sphere, byte for byte, so their goldens do
  not move.
- Real places use a new `EarthGeo` on the **WGS84 ellipsoid**. It is built in WP5 on the `geo`
  crate's `Wgs84Frame`.
- Both implement `GeoService`, so nothing above the seam changes.

This is a GDD clarification (GDD 0.2.6, 7.4.4): the ellipsoid is not replaced by a sphere anywhere in
the sim, except in the two static test arenas of ADR-43.

### The `geo` crate

`crates/geo` (lib `geo`) is a sim crate. It depends only on `simmath`; `geographiclib-rs =0.2.7` is a
dev-dependency, used only as the reference of GE7. It has these modules:

- **`wgs84`**: the constants `a = 6 378 137 m` and `f = 1 / 298.257 223 563`. `e² = f (2 − f)` and
  `b = a (1 − f)` are computed from them, not typed as decimals. It also has the radii of curvature
  `N(φ)` and `M(φ)` and the geocentric radius.
- **`geodetic`**:
  - geodetic → ECEF, exactly GDD 7.2.1;
  - ECEF → geodetic, exactly GDD 7.2.2:
    - the iterative method, at most 10 passes, converged at `|Δφ| < 1e-12`;
    - the height from `Z / sin φ` above 80°;
    - the height recomputed with the final latitude;
    - on the polar axis (`p < 1e-9 m`), `φ = ±π/2`, `λ = 0` and `h = |Z| − b`.
- **`frame`**: `Wgs84Frame { anchor, a_ecef, rot }`. It follows GDD 7.2.3: the rows of `rot` are e, n
  and u at the anchor, `p_BF = R (p_ECEF − A)`, and the anchor's height is 0.
  - `up_bf` is GDD 7.2.4's `normalize(X / a², Y / a², Z / b²)`, rotated into BF.
  - Directions use `R` alone.
- **`angles`**: longitudes are normalised to (−π, π] (GDD 7.8). It also has `delta_lon`, the latitude
  clamp and `DEG`/`RAD`.
- **`distance`**:
  - haversine on R = 6 371 000 m (GDD 7.3);
  - the initial bearing (GDD 7.3);
  - `inverse`, which is Vincenty's inverse on WGS84:
    - at most 200 iterations, converged at `|Δλ| < 1e-12`;
    - the spherical answer, flagged `Accuracy::Spherical`, when the points are within 0.5° of
      antipodal or the loop does not converge (GDD 7.3's "fall back to the spherical solution with a
      flagged accuracy");
  - metres per degree `(M π/180, N cos φ π/180)`;
  - the horizon distance and dip (GDD 7.4.2).
- **`tile`**: the geodetic quadtree. The tile ids, bounds, parents, children and cells are recorded
  in ADR-54.
- **`codec`**, **`sample`**, **`crc32c`**, **`gpk`**: the height codec, the Catmull–Rom sampler,
  CRC-32C and the `GeoPack` layout (ADR-54).
- **`testkit`** (feature `testkit` or tests only): the seeded xoshiro256** points that the gate tests
  and the WebAssembly selftests draw from. The geodesy itself takes no randomness.

Every transcendental goes through `simmath::dmath` (ADR-10); clippy enforces it. The crate has no
`HashMap`, no fast-math and no allocation in its maths.

### As measured in WP1 (2026-11-13, natively)

| Gate | Result | Limit |
|---|---|---|
| GE1 | the GDD vector within 1 mm on each axis | 1 mm |
| GE2 | 10⁶ points: height 2.4e-7 m, latitude 3.7e-13°, longitude 2.8e-14° | 1 µm, 1e-12° |
| GE3 | 10⁵ BF points: 2.3e-7 m | 1 µm |
| GE4 | worst 0.29 % (90° and 270° at 10 km, where the radius of curvature is N, not R) | 0.5 % |
| GE7 | 10 000 pairs against GeographicLib: 7.9e-5 m, 1.4e-8°; near-antipodal pairs flagged spherical within 0.6 % | 1 mm, 1e-6° |
| GE8 | the GDD 7.3 table within 1 m; haversine and bearing equal their formulas | 1 m; 1e-9 relative |

GE5 (the same results in WebAssembly) is proved by `fg_selftest` in WP5. GE9 (Rust and TypeScript
agree on the tile maths) is proved by `tests/vectors/tiles.json` in WP6. GE6 (the antimeridian
flight) is proved in WP5.

## Alternatives considered

- *One sphere for everything:* it would keep one code path, but it breaks GDD 7.4.4 by up to 66 m at
  500 km.
- *Moving the test arenas to the ellipsoid:* it would move all five goldens for no gameplay gain,
  against ADR-51's hard constraint.
- *Karney's geodesic in the sim:* it is far more code than Vincenty with a flagged fallback, which
  GDD 7.3 allows. Karney stays the test's reference.
- *A closed-form ECEF → geodetic (Heikkinen, Vermeille):* GDD 7.2.2 allows it on the hot path, but
  the iterative method is the reference and fast enough (10⁶ round trips in well under a second).

## Consequences

- Real places and the test arenas use different `GeoService` implementations. Nothing above the seam
  may assume either.
- The `geo` crate is shared by the core (WP5), the offline pipeline (WP3) and, through the generated
  vectors, the TypeScript `geo-stream` package (WP6).
- WP5 adds the world-session part of this ADR.
