# ADR-53: The battlefield system: shapes, the anchor, the SDF, presets, the validator, the boundary and the sim terrain plan

- **Status:** Proposed
- **Owner:** Bl4202
- **Date:** 2026-11-13
- **Affects:** GDD 0.6.4, 8.1–8.9, 44.6; `crates/world`; `crates/sim-core` (world sessions, WP5)

## Context

GDD 8 defines the battlefield: an authored shape, a curved frame anchored at the shape's centroid,
a signed distance field for boundary queries, five size presets, a validator, and four boundary
stages. v0.2a builds all of it for real places (roadmap 7.1). It is gated by BF1 to BF8 and WS1 in
ADR-51.

A world session also needs a stable ground before its first tick, and nothing streamed may reach the
sim (ADR-51). So the battlefield decides which terrain tiles the session loads in advance: the sim
terrain plan, GDD 44.6's simple form.

## Decision

`crates/world` (lib `world`) is a sim crate. It depends only on `geo` and `simmath`, takes no
randomness, and allocates only when a world is applied, never per tick.

### Shapes become polygons
- The shapes are authored in degrees (data) and sent in radians (the world configuration block):
  - rect;
  - circle;
  - ellipse;
  - capsule;
  - polygon, with at most 64 vertices.
- Every shape becomes a counter-clockwise BF polygon:
  - a rect has 4 vertices;
  - a circle or an ellipse has 256 (at most 4.5 m from the curve at r = 60 km);
  - a capsule has 2 x 129;
  - a polygon keeps its own vertices.
- Rotations are clockwise from north: +30° turns a rect's height axis towards east.
- The size class comes from the shape's own area formula (a polygon uses its shoelace area), so a
  120 km square is exactly Medium.

### The anchor (GDD 8.7)
1. The shape's reference point:
   - for a rect, circle or ellipse, the authored centre;
   - for a capsule, the surface point under the ECEF mean of its ends;
   - for a polygon, the surface point under the ECEF mean of its vertices.
2. The polygon is built in the provisional frame F0 at that point.
3. Every edge is densified to segments of at most 5 km, and their ECEF positions are summed.
4. The anchor is the geodetic of the mean, at height 0.
5. The vertices are re-expressed in the frame at the anchor.

Being an ECEF mean, the anchor is stable for shapes across the antimeridian (BF6).

### The SDF (GDD 8.2)
The field is f32, negative inside. Its sizes are:

- `W = warn_width_m` if positive, else `max(2 km, 3 % of the shorter side of the bounding box)`;
- `D = max(2 W, 10 km)`;
- the cell is `L0 / min(1 020, ceil(L0 / 100 m))`, where `L0` is the longer side of the bounding box
  expanded by `D`;
- the grid covers that box plus two border cells on each side, so no side exceeds 1 024 cells
  (4 MB).

It is built in three steps:

1. Each edge updates the squared distance of the cells within `D` of its bounding box.
2. Each row's cell centres take the non-zero winding number from the row's sorted edge crossings.
3. `sdf = ∓min(d, D)`.

The result is **exact (to f32) at every cell centre within ±D of the boundary, and clamped to ±D
beyond**: a clarification of GDD 8.2. Sampling is bilinear; outside the grid it returns `+D`. The
normal is the normalised central-difference gradient.

### Presets and corridors
These are GDD 0.6.4 and 8.6 exactly: `SizeClass::for_area` picks the smallest class whose area is at
least the shape's, and `corridor(L)` gives `r = max(30 km, 0.3 L)` and `area = 2 r L + π r²`.

### The boundary (GDD 8.5)
- **Outside:** `sdf > 0`, or above the 30 000 m ceiling (GDD 8.4).
- **Advisory:** inside, with `sdf ≥ −W`.
- **The countdown:**
  - It starts on the tick that time outside first exceeds 3 s.
  - It runs down only while outside.
  - **It holds while inside, and is reset once the aircraft has been inside for 5 s**; time
    outside resets with it. This clarifies GDD 8.5's "resets only after being inside for 5 s".
  - Leaving again before 5 s resumes the same countdown.
- **When it runs out:**
  - `mission_fail` and `deserter` lose the aircraft (`OUT_OF_AREA`, through the crash path);
  - `assist_turn` engages the turn-back pilot until the reset.
- **The comparisons** allow 1e-9 s of slack, so 1/60 s ticks land on whole seconds.
- **Where it runs:** in the core's Director stage (WP5).

### The validator (GDD 8.8, v0.2a's checks)

| Check | What it requires | Result |
|---|---|---|
| V1 | The polygon is simple (authored polygons only). Its area is at least 1 km². Its extent (the largest vertex-to-vertex distance) is at most 1 000 km. It has at most 64 authored vertices | reject |
| V2 | The anchor is within ±85° | reject |
| V3 | Every listed cell that meets the playable area + 10 km holds z10 or finer | reject |
| V4 | Every listed cell that meets the fine zone holds `spawn_level` | warning `LOW_DETAIL` |
| V5 | The spawn is inside | reject |
| V6 | The air spawn is at least 150 m above the planned terrain. The core checks this once tiles are in | the spawn is raised to 300 m above it, with warning `SPAWN_RAISED` |
| V7 | The entity budget | passes: one aircraft in v0.2a |
| V8 | A base or an air spawn exists | passes: an air spawn |
| — | The plan has at most 8 192 tiles | reject `TOO_LARGE` |

- A cell that is not listed is ocean, and is complete at every level.
- `spawn_level` (10 or 12) is the level wanted under the spawn, in `region/1` and the world block.

### The sim terrain plan (GDD 44.6's simple form)
1. **The two zones:**
   - the playable area expanded by 10 km;
   - the fine zone, the disc of 40 km around the spawn.
2. **The z10 tiles:** each z10 tile that meets the expanded area is planned. In a cell whose archive
   holds z12:
   - Small, Medium and Large plan its z12 descendants that meet the area, instead of the z10 tile;
   - Huge and Extreme plan the z10 tile, plus its z12 descendants that meet the fine zone.
3. **The fallback:** every z5 tile within 200 km of the playable area is also planned.
4. **The list:** it is sorted and deduplicated, with at most 8 192 tiles.
5. **"Meets"** is conservative: one of the tile's nine sample points lies in the region, or one of the
   region's vertices lies inside the tile. (The plan's third test, the tile centre within the region,
   is one of the nine samples.)
6. **The height lookup** tries the plan's finest level, then z10, then z5.

### As measured in WP2 (2026-11-13, natively)

| Gate | Result | Limit |
|---|---|---|
| BF1 | worst error 6.8 m (capsule), 4.5 m (circle), 0.0 m (rect), at 46.55° N 8° E, 0° 0° and 64° N 179.5° E | one cell (137 to 470 m) |
| BF3 | 90 km, 79 446.9 km², Huge | ±0.1 km², Huge |
| BF4 | 68 ms natively in the debug test build (informational; the gate is ≤ 50 ms in `sim.single.wasm`, measured in WP6) | — |
| BF5 | each of V1 to V8 and the plan size pass and fail as specified | — |
| BF6 | a 400 km circle at 0° N 180° E: 3e-11 m from the centre; a square across 180°: 2e-9 m | 1 m |
| BF7, BF8 | as specified | — |

**WS1** pins the counts. At the Alps, with cell (33, 7) at z12, the plans hold:

| Size | z5 | z10 | z12 |
|---|---|---|---|
| Small | 3 | 0 | 326 |
| Medium | 4 | 0 | 1 257 |
| Large | 6 | 0 | 4 539 |
| Huge | 6 | 1 101 | 342 |
| Extreme | 15 | 4 089 | 342 |

At the antimeridian fixture, Medium holds 2 z5 and 104 z10 tiles. BF2, the scripted boundary
flight, runs in the core (WP5).

### Corrections to the v0.2a plan, found while building
1. **`fine_level`** is 12 when the plan holds any z12 tile. The plan's rule ("the spawn's cell has
   z12") would skip the z12 tiles of a battlefield that reaches into a second cell, where the z10
   tile is not planned.
2. **The capsule's reference point** is the surface under the ECEF mean of its ends. The plan said
   "the geodetic of `normalize(ecef(A) + ecef(B))`", and a unit vector has no meaningful geodetic.
3. **BF2's "the stage returns to INSIDE 300 ticks after re-entry"** means that the countdown resets
   then (the gate's own words in ADR-51). After the reset the stage is ADVISORY while the aircraft
   is still within `W` of the edge.
4. **Open for the owner:** V1's extent rule, the largest vertex-to-vertex distance of at most
   1 000 km, rejects the Extreme preset's own 1 000 km square, whose diagonal is 1 414 km. Nothing in
   v0.2a builds an Extreme battlefield, so the rule stays as written. Reading "extent" as the longer
   side of the bounding box would admit it. This is to be decided before an Extreme battlefield is
   used (v0.4).

## Alternatives considered

- *Rasterising each shape's analytic distance:* there would be one formula per shape, and polygons
  would still need the general method. One method for all shapes keeps the anchor, the SDF and the
  validator on the same geometry.
- *An exact Euclidean distance transform over the whole grid:* it is more code for a field that
  only has to be exact near the edge. Beyond `D` the boundary logic needs only the sign.
- *Streaming the sim's terrain:* it is v0.2b's, with its own determinism suite. Loading the plan
  before the first tick keeps every world session deterministic now.

## Consequences

- A world session's memory grows with its plan: Medium is about 1 261 tiles x 37 KB ≈ 47 MB, and the
  8 192-tile limit is about 306 MB.
- The validator runs inside `fg_world_apply`; a reject leaves the core Idle.
- An Extreme square needs the V1 decision (correction 4) before v0.4 uses it.
