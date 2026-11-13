# ADR-51: v0.2a gates: geodesy, the battlefield, the GeoPack pipeline, licences, world sessions, streaming and the globe

- **Status:** Proposed
- **Owner:** Bl4202
- **Date:** 2026-11-12
- **Affects:** GDD 7, 8, 9.9, 24, 44, 45, 61.5, 63.7, Appendix H; `crates/geo`, `crates/world`,
  `crates/geo-pack`, `tools/geo-pipeline`, `crates/sim-core`, `crates/sim-wasm`, `crates/cli`,
  `crates/xtask`, `packages/geo-stream`, `packages/render`, `packages/sim-host`, `packages/sim-bridge`,
  `packages/ui`, `packages/app-shell`, `packages/platform-web`, `packages/tools`, `content/world`,
  `tests/`, `.github/workflows/*`

## Context

The v0.2a section of docs/roadmap.md (7.1) reads:

> **Goal:** the Earth exists as data, streams without holes and renders from orbit to the runway.
> Geodesy and frames (§7.2–§7.7) · Battlefield system (§8.1–§8.8) · Geodata pipeline (§45.5–§45.7,
> §45.11) · Licensing (§45.9, Appendix H) · Hosting (ADR-50) · Streaming (§44.2–§44.10) ·
> Rendering: globe rendering and atmosphere (§9.9, §24 basic); the `GeoService` seam's real
> implementation (§61.5). **Content:** the base globe pack and the six pilot regions at full detail.
> **Tools and tests:** coverage and QA reports per pack build; world debugging views (§63.7).
> **Exit:** 1. Seamless zoom from orbit to the ground; no holes in a scripted descent (`AC-44.1`
> precursor). 2. Geodesy round-trip tests (`AC-7.*`) and battlefield validator tests (`AC-8.*`)
> pass on native and WASM. 3. The licence descriptor audit passes for the pilot pack (`AC-45.4`).

v0.2a is the first sub-milestone of M2. This ADR fixes every pass/fail threshold **before** any
v0.2a code exists, so that none can be adjusted to fit what the code happens to do (AC-66.4).

The owner decided four points on 2026-11-12:

- **D1, world depth.** The whole Earth to z10 (about 150 m posts) from Copernicus DEM GLO-90; the
  six pilot regions to z12 (about 38 m posts) from GLO-30. Worldwide z11–z12 is v0.2b's (for
  `AC-3.1`).
- **D2, where packs are built.** A manual GitHub Actions workflow (`geo-publish.yml`) builds the
  datapack in parallel jobs and uploads it with a scoped R2 token kept as repository secrets (as
  ADR-50 says). Local builds are for development and the synthetic fixture.
- **D3, the public address.** The bucket's `r2.dev` public development URL for v0.2; a custom domain
  before the public beta (v0.5e).
- **D4, staging.** The six regions in the main menu's Arena list (air start, a Medium battlefield,
  the boundary) and a Globe button. The two test arenas and the dogfight stay as they are.

Three points carry over from ADR-50:

- **C1, data licences.** Only data whose descriptor says `commercial_ok: true`; the audit fails a
  pack otherwise. Copernicus DEM is the only source in v0.2a's packs.
- **C2, panels.** Automated gates decide (OQ-12); the owner's own look is recorded and does not
  block. v0.2a has no panel gate: G4's look-development gate is v0.2c's.
- **C3, merges.** The v0.2a plan's approval authorises two merges to `main` and no others.

**Hard constraint:** the five existing determinism goldens (`tests/golden/{determinism,flight60,
circuit,plain-circuit,combat}.json`) must stay byte-identical to `v0.1d`. Every new piece of state is
hashed only in world sessions, and the two test arenas keep their sphere (`ProceduralGeo` is not
touched).

Where a table below says "section N", it means section N of the v0.2a implementation plan. The parts
of those sections the gates depend on are copied under "Definitions the gates use", with the plan's
numbers.

## Decision

Every gate has an id. **Gate** = blocks the tag. **Recorded** = written in the exit doc, does not
block. **Informational** = measured and reported only. "Native and WASM" means: the test runs as a
Rust unit/integration test natively (debug, as CI runs `cargo test --workspace`), **and** its result
(a hash of every output value's bit pattern) is produced by the same code inside `sim.single.wasm` and
`sim.threads.wasm` through `fg_selftest` and compared by `cargo xtask determinism` (section 7.5). No
human panel decides any gate (C2).

### 1. Geodesy (roadmap exit 2)
| # | Test | Pass |
|---|---|---|
| GE1 | AC-7.1 vector: `geodetic_to_ecef(36.2°, −121.8°, 1 500 m)` | (−2 716 000.612, −4 380 460.552, 3 747 008.625) m within 1 mm each (gate) |
| GE2 | AC-7.1 round trip: 10⁶ points from `geo::testkit::Points` (seed 1), φ uniform in [−89.9°, 89.9°], λ in (−180°, 180°], h in [−500, 30 000] m; geodetic → ECEF → geodetic | max height error < 1 µm; max φ and λ error < 1e-12° (λ compared after normalising the difference to (−180°, 180°]) (gate) |
| GE3 | AC-7.2: 100 anchors (seed 2, φ in [−85°, 85°]) x 1 000 BF points each (x, y uniform in a disc of 1 000 km, z in [−20 000, 30 000] m) — 10⁵ in all; BF → geodetic → BF | max position error < 1 µm (gate) |
| GE4 | AC-7.3: anchor 46.55° N, 8.0° E; azimuths 0°, 90°, 180°, 270°; BF horizontal distances 10, 50, 100, 300, 500 km; find by bisection (80 steps on z) the BF point at that (x, y) whose geodetic height is 0; its drop `−z` | within 0.5 % of `d² / (2 x 6 371 000)` at every point (the GDD table prints this formula rounded; 10 km is 7.848 m, not 7.8). Measured for this plan: worst 0.33 % (gate) |
| GE5 | AC-7.4: selftests `GEO_ROUNDTRIP` (GE2's 10⁶ geodetic → ECEF → geodetic results) and `BF_ROUNDTRIP` (GE3's 10⁵ `geo_to_bf` → `bf_to_geo` results), hashed over every output's bit pattern | native = `sim.single.wasm` = `sim.threads.wasm` (gate) |
| GE6 | AC-7.5: the antimeridian flight (WS6) | max terrain-height step between consecutive 120 Hz samples < 1 m (gate) |
| GE7 | Geodesics: 10 000 random pairs (seed 3) with angular separation < 179°; `geo::inverse` (Vincenty) against `geographiclib-rs 0.2.7` (dev-dependency only) | distance within 1 mm, initial bearing within 1e-6°; every pair with separation ≥ 179.5° returns `accuracy: Spherical` and a distance within 0.6 % of GeographicLib's (gate) |
| GE8 | Haversine and bearing: 1 000 pairs against hand formulas in the test; the GDD 7.3 metres-per-degree table | haversine equals its formula within 1e-9 relative; the table's 12 values reproduced to the metre (gate) |
| GE9 | Tile maths vectors (`tests/vectors/tiles.json`, written by `cargo xtask gen-abi`): `tile_id`, bounds, `tile_for`, parent, children, cell for 64 cases including the antimeridian and both poles | Rust and TypeScript (`packages/geo-stream`) give identical results (gate) |

### 2. Battlefield (roadmap exit 2)
| # | Test | Pass |
|---|---|---|
| BF1 | AC-8.1: SDF of a rect (120 x 80 km, rotated 30°), a circle (r 60 km) and a capsule (A–B 300 km, r 90 km), each at three anchors (46.55° N 8° E; 0° 0°; 64° N 179.5° E); every grid cell centre with `|analytic distance| ≤ D` | `|sdf − analytic| ≤ cell size` (gate) |
| BF2 | AC-8.2: the scripted boundary flight (section 8.4 constants) for four parameter sets: Easy (`assist_turn`, 30 s), Normal (`mission_fail`, 30 s), Hard (`mission_fail`, 15 s), Simulator (`deserter`, 10 s) | each stage change happens within ±6 ticks (0.1 s) of its predicted tick (rules in section 8.4); for Easy the aircraft is back inside within 120 s and the countdown resets 5 s after re-entry (gate) |
| BF3 | AC-8.3: corridor sizing, L = 300 km | r = 90 km, area 79 446.9 km² (±0.1), class `huge` (gate) |
| BF4 | AC-8.4: building the SDF of a 64-vertex star polygon (alternate radii 400 and 250 km) | ≤ 50 ms in `sim.single.wasm` under Node on the Mac M4 (gate, measured by `node packages/tools/src/geo-bench.ts`); natively in CI: informational |
| BF5 | Validator: one passing and one failing case per check of section 8.5 (V1 to V8) | each returns the specified status and reason code (gate) |
| BF6 | Anchor: a 400 km circle centred on 0° N 180° E, and a polygon straddling the antimeridian | anchor within 1 m of the expected point; native = WASM bit-for-bit (gate) |
| BF7 | Presets: the five classes of GDD 0.6.4 and the "smallest preset whose area ≥ shape area" rule | the GDD's edges and areas; boundaries at 2 500, 14 400, 62 500, 250 000, 1 000 000 km² (gate) |
| BF8 | Boundary state machine unit tests (hold, 5 s reset, re-exit before 5 s continues the countdown, ceiling counts as outside) | as section 8.4 (gate) |

### 3. GeoPack and the pipeline
| # | Test | Pass |
|---|---|---|
| GP1 | AC-45.1 (reproducibility): `geo-pipeline synth` run twice into two directories; and in `geo-publish.yml` every job rebuilds its first cell a second time | byte-identical files (SHA-256 equal) (gate) |
| GP2 | AC-45.1 (voids): the QA report of the published datapack | `unfilled_voids = 0` for every cell (gate) |
| GP3 | AC-45.2: 10⁵ random tile requests (seed 4) against the fixture cell archive, (a) full-file read in Rust, (b) range requests through the TS chunk cache against the fixture server, both returning the decoded tile | identical bytes; (c) OPFS-cached read is v0.2d (recorded as deferred) (gate for a, b) |
| GP4 | AC-45.3: the QA report's counters for each cell | tile counts per level = `4^(z−5)` minus implicit-ocean tiles, listed; raw size = count x 33 282 B; the totals table printed (gate: present and consistent) |
| GP5 | Format vectors (`tests/vectors/gpk.json` + `tests/vectors/gpk_fixture.gpk`, 3 tiles, written by `cargo xtask gen-abi`) | Rust `geo-pack` and TS `geo-stream` decode the same heights, landcover, h_min, h_max, ε (gate) |
| GP6 | TIFF reader: (a) round trip of TIFFs written by the test (float32, predictor 3, zlib, tiled and padded); (b) `#[ignore]` network test on GLO-90 N46 E007 (section 1.2 values) | (a) bit-exact; (b) min 398.0, max 4 462.87255859375, Lake Thun 557.5, Bern 553.6300048828125 (gate for a; b runs in `geo-publish.yml` before the build) |
| GP7 | CRC32C | `crc32c(b"123456789") = 0xE306_9283` in Rust and TS (gate) |

### 4. Licensing (roadmap exit 3)
| # | Test | Pass |
|---|---|---|
| LA1 | AC-45.4 audit of the published datapack (`geo-pipeline audit`, run by `geo-publish.yml` before `datapack.json` is written) | every layer of every archive maps to a descriptor; `terms_checked` ≤ 365 days before the build date; `commercial_ok`, `cache_allowed`, `redistribution_allowed` true; no ODbL source without a dump (gate) |
| LA2 | The audit catches faults: the fixture with (a) a descriptor removed, (b) `terms_checked` 400 days old, (c) `commercial_ok: false` | each fails with its message (gate, in CI) |
| LA3 | AC-45.4 credits: e2e opens Settings → Data → Credits & data | every `required_notices` string of every descriptor in the world bundle is on screen; the region attribution line shows under the Arena list (gate) |

### 5. World sessions in the sim (roadmap exit 2, and the vertical slice)
| # | Test | Pass |
|---|---|---|
| WS1 | `SimTerrainPlan` unit tests: Small/Medium/Large/Huge/Extreme at the Alps point and at the fixture's antimeridian point | the plan's level rules (section 8.2); sorted, no duplicates; counts recorded in the test |
| WS2 | Start gating: configure + apply a world session, commit all plan tiles but one | `fg_start` returns 0; after the last commit it returns 1 (gate) |
| WS3 | AC-44.3 (precursor): 10⁶ random points (seed 5) inside the fixture plan; `terrain_height` | the hash of the f32 bit patterns: native = single WASM = threads WASM (gate) |
| WS4 | `EarthGeo` passes `seams::geo::contract` (the existing contract) with the fixture tiles | (gate) |
| WS5 | The `world` determinism suite (section 7.5): erratic script, 36 000 ticks, checkpoints every 600 | native release = native debug = single = threads = `tests/golden/world.json` (gate) |
| WS6 | Antimeridian flight: fixture region `fixture-antimeridian` (spawn 3 000 m MSL, at least 500 m above the island's highest point, 16.6 km west of 180°), HOLD script heading 090, 5 minutes (it crosses ±180° and stays inside the 120 km battlefield), sampling `ground_height_msl_m` under the aircraft at every sub-step (120 Hz) | GE6 (gate) |
| WS7 | No allocation: `crates/sim-wasm/tests/no_alloc.rs` gains a world session (fixture, erratic, a boundary loss and respawn) | 0 allocations after `fg_start` (gate) |
| WS8 | Arena sessions untouched: `cargo xtask determinism` on the five old suites; `git diff --exit-code v0.1d -- tests/golden/{determinism,flight60,circuit,plain-circuit,combat}.json` | both pass (gate) |
| WS9 | The boundary in the browser (`world-boundary.spec.ts`, section 9.7's `bfprobe`) | the warning strip shows BOUNDARY, RETURN TO BATTLEFIELD, the countdown and OUT OF AREA in that order, the aircraft is destroyed with cause `OUT_OF_AREA` and is back at the spawn after the normal crash flow (gate) |

### 6. Streaming (roadmap exit 1)
| # | Test | Pass |
|---|---|---|
| ST1 | AC-44.2: the selector's switch distance for z5 to z13 with W = 1 920, fov_h = 90°, τ = 2, ε = 0.35 x post spacing | within 5 % of GDD 44.4's table (gate) |
| ST2 | Residency invariant, unit (`world-terrain.test.ts`): 10 000 random camera paths through a mock tile source with random latencies and failures | every drawn tile's ancestors to z5 are resident; the drawn ∪ culled leaves partition the globe exactly (integer area check, section 8.7) (gate) |
| ST3 | **No holes in the scripted descent** (`tests/e2e/world-descent.spec.ts`, the fixture server, WebGL2 on the CI rasteriser and WebGPU on the M4): the descent of section 8.7 in the classify view, a frame read every 0.5 s | 0 pixels with the backstop-globe key in every frame read; the per-frame invariant counter `holeFrames` is 0 (gate) |
| ST4 | The same descent on staging with the real datapack, on the M4 (WebGPU, mains) — `pnpm perf:local --scenario descent` | as ST3 (gate) |
| ST5 | Budgets during ST3/ST4 | resident layers ≤ the profile's budget; in-flight requests ≤ the cap; queued ≤ 64; the atlas never full with a wanted tile refused (gate) |
| ST6 | Streaming cost on the Render Worker (selection + culling + upload + bookkeeping) during ST4 on the M4 | avg ≤ 0.7 ms, p99 ≤ 2 ms per frame (gate) |
| ST7 | Failures (fixture server faults, section 9.6): 10 % of range requests 404; 5 % of tiles corrupted once; one tile always corrupted; 2 s added latency on 20 % | no hole (ST3 checks), no uncaught error, retries and quarantines counted in the world panel; the always-corrupt tile is quarantined after one re-fetch and its parent stays drawn (gate) |
| ST8 | Stream Worker crash (`?debug=1&fault=stream-crash` kills it 5 s into the descent) | restarted within 2 s; no hole (gate) |
| ST9 | Throughput sanity: a 10-minute flight at M 0.9, 500 m AGL across the Alps cell on staging (M4) | informational: MB/s, requests/s, retries |

### 7. Rendering
| # | Test | Pass |
|---|---|---|
| RN1 | Horizon dip over the fixture's ocean (classify view, `?selftest=screens&case=world-horizon`): eye heights 100 m, 1 km, 5 km, 10 km, 15 km | measured dip within 0.05° of `acos(Rg / (Rg + h))`, `Rg = √(M N)` at the eye's latitude (gate, CI rasteriser and M4) |
| RN2 | Atmosphere: (a) at 400 km above the fixture looking at the limb, the mean luma of the 3-pixel band just above the limb > 0.25 and 40 px above it < 0.05; (b) at 10 km in daylight, zenith luma < horizon luma; (c) `fault=no-atmosphere` fails (a) | (gate) |
| RN3 | Night side: a terrain pixel where the sun is 15° below the horizon | luma < 0.05 (gate) |
| RN4 | The six regions on staging (M4): one cockpit and one chase screenshot each, 3 000 m AGL, noon local | recorded in `docs/exit/v0.2a/regions/` (the owner looks; does not block) |
| RN5 | The arena's screenshot tests S1–S5 and HUD tests | unchanged and green (gate) |

### 8. Performance (reference machine: the Mac M4, WebGPU, mains, 1 920 x 1 080, High; ADR-28)
| # | Test | Pass |
|---|---|---|
| PF1 | WORLD scenario (new): the Alps region on staging, cockpit, scripted HOLD flight at 600 m AGL heading 090, 120 s after 10 s warm-up | frame avg ≤ 16.7 ms, p99 ≤ 22.2 ms (the arena gates) (gate) |
| PF2 | Sim step in the WORLD scenario | avg ≤ 4.0 ms, p99 ≤ 7.0 ms (PS-0 limits) (gate) |
| PF3 | ARENA, COMBAT and PS-0 scenarios re-run | the v0.1d gates (gate) |
| PF4 | Control latency during PF1 | p95 ≤ 25 ms (gate) |
| PF5 | `cargo xtask bench` (arena) | ≤ 5 % slower than `tests/perf/baseline.json` (gate) |
| PF6 | PS-9 baselines: tile decode (TS: CRC + inflate + undelta per tile), control-grid build per tile, SDF 64-gon, `terrain_height` per call, `bf_to_geo` per call | recorded as the first baseline in `tests/perf/baseline.json` (`geo` block); the 5 % rule applies from v0.2b (informational) |

### 9. The rest
| # | Pass |
|---|---|
| NA | No allocation after start in world sessions (WS7); the render worker's per-frame world code makes < 320 B per frame at rest (garbage harness, as `scene.garbage.test.ts`) (gate) |
| LY | `check-layers` (rules for `geo`, `world`, `geo-pack`, `geo-pipeline`), `check-content-ids` (`geo`, `world` added to `SIM_CRATES`), `depcruise` (`geo-stream` at tier 1) (gate) |
| DOM | AC-53.4: ≤ 150 DOM nodes in a world session with the overlay and the world panel open (gate) |
| CSP | The CSP differs from v0.1d's only by the `connect-src` origins of section 3 (`headers.test.ts`) (gate) |
| PUB | `geo-publish.yml` run on `main` green; `datapack.json` live; staging smoke reads `datapack.json` and one tile from R2 through CORS with a 206 response (gate) |
| SG | `deploy-staging` and `staging-smoke` green on the final commit (gate) |
| CI | Every required job green on the final commit (gate) |
| OW | The owner's look at the six regions and the globe (section 13.3) (recorded) |

### 10. The exit checklist
| # | Item | Gates |
|---|---|---|
| 1 | Seamless zoom from orbit to the ground; no holes in a scripted descent (roadmap exit 1, `AC-44.1` precursor) | ST1–ST8 (ST9 informational) |
| 2 | Geodesy round-trip tests pass on native and WASM (roadmap exit 2, `AC-7.*`) | GE1–GE9 |
| 3 | Battlefield and validator tests pass on native and WASM (roadmap exit 2, `AC-8.*`) | BF1–BF8 |
| 4 | The licence audit passes for the pilot datapack (roadmap exit 3, `AC-45.4`) | LA1–LA3 |
| 5 | GeoPack format, pipeline reproducibility, QA (`AC-45.1`–`AC-45.3`) | GP1–GP7 |
| 6 | World sessions: the `GeoService` seam's real implementation, deterministic | WS1–WS7, WS9 |
| 7 | The five existing goldens unchanged | WS8 |
| 8 | Globe and atmosphere; arena screenshot tests unchanged | RN1–RN3, RN5 |
| 9 | The six regions on staging | RN4 (recorded) |
| 10 | Performance on the M4 | PF1–PF5 (PF6 informational) |
| 11 | The datapack published on R2 and reachable from staging | PUB |
| 12 | No allocation; layering; DOM; CSP | NA, LY, DOM, CSP |
| 13 | Deliberate faults (section 13.2) all caught | — |
| 14 | CI, staging deploy and smoke green on the final commit | CI, SG |
| 15 | The owner's look at the world | OW (recorded) |

### 11. Deliberate faults (exit item 13)
Each fault must make the named check fail (plan section 13.2). A fault that does not is a finding:
the check is strengthened and the fault run again.

| # | Fault | Check that must fail |
|---|---|---|
| 1 | `geodetic_to_ecef` uses `N` without `(1 − E2)` in Z | GE1 |
| 2 | ECEF→geodetic stops after one iteration | GE2 |
| 3 | `up_bf` uses the geocentric direction instead of the ellipsoid normal | GE3 or GE4 (record which) |
| 4 | `tile_for` without the longitude wrap | GE9 and GE6 |
| 5 | Morton with x and y swapped | GE9, GP5 |
| 6 | SDF without the sign (always positive) | BF1 |
| 7 | Countdown resets on the first tick inside | BF2, BF8 |
| 8 | Anchor from the mean of lat/lon degrees instead of ECEF | BF6 |
| 9 | Pipeline writes tiles in thread completion order | GP1 |
| 10 | Height payload not delta-decoded in the TS decoder | GP5, GP3 |
| 11 | CRC computed over the uncompressed bytes in the writer only | GP5 (reader rejects) |
| 12 | Descriptor `commercial_ok: false` in the fixture | LA2 |
| 13 | A plan tile never committed (the loader drops one) | WS2 (start refused) and the loading never completes (e2e times out with a clear message) |
| 14 | The world configuration left out of the `ownship` column | `world_hash_covers_the_config` (WP5 unit test: two sessions that differ only in `countdown_s` must have different `ownship` hashes at tick 1) |
| 15 | An arena session hashes the (empty) world words | WS8 |
| 16 | `fault=drop-children` | ST3 |
| 17 | Eviction ignores resident children | ST2 |
| 18 | Horizon culling with `Rs = A` (larger than the polar radius) | ST2 or ST3 (a visible tile culled near the poles; record which) |
| 19 | No retry on 404 | ST7 |
| 20 | `fault=no-atmosphere` | RN2 |
| 21 | Flat-earth fault (`fault=flat-earth`) in a world session | RN1 |
| 22 | The CSP without the R2 origin | PUB (and the browser console shows the CSP block) |
| 23 | A 1 KB allocation per tick in `BoundarySystem` | WS7 |

## Definitions the gates use

### Section 3: the CSP origins (gate CSP)
- `connect-src 'self' <R2 origin> http://localhost:4175 http://127.0.0.1:4175` (4175 = the e2e fixture server). One constant, `GEO_CONNECT_ORIGINS`, in `packages/platform-web/src/geo-origin.ts`. ADR-22 amendment 3

### Section 7.5 How "native and WASM" is proved (GE5, WS3, BF6, and every gate marked so)
- `fg_selftest(kind: u32, count: u32) -> u32` (sim-wasm export, ABI v6) runs a fixed computation in
  `crates/sim-core/src/world/selftest.rs` and returns 0 (ok) or an error; the 64-bit FNV-1a hash of
  every output's bit pattern is then read with `fg_subsystem_hash_lo/hi(hash_kind::SELFTEST = 8)`.
  Kinds (`selftest` group, Appendix A): `GEO_ROUNDTRIP = 1` (GE2's points through `bf_to_geo`),
  `BF_ROUNDTRIP = 2` (GE3), `TERRAIN = 3` (WS3; needs a world session with the fixture applied and
  committed), `SDF = 4` (BF1's three shapes, every cell), `ANCHOR = 5` (BF6), `GEODESIC = 6` (GE7's
  pairs), `SDF64 = 7` (builds BF4's polygon once; used for timing, its hash is compared too).
- `fg selftest --kind <name> --count <n> [--world <region.json> --pack <dir>]` (new `cli`
  subcommand) prints the same hash natively.
- `cargo xtask determinism` gains a `selftests` step that runs each kind natively (release and debug)
  and through `packages/tools/src/wasm-hash.ts --selftest <kind>` for both WASM variants, and fails on
  any difference. The expected native hashes are **not** pinned in a golden (only equality is gated).

### Section 7.6 The world determinism suite
- Fixture: `cargo xtask geo-fixture` runs `geo-pipeline synth --out target/geo-fixture` (section
  9.5). Region file: `tests/geo-fixture/fixture-small.json` (`region/1`, Small battlefield 50 km
  square centred 46.55° N 8.0° E over the fixture's island I1 (section 9.5), spawn at the centre,
  4 000 m MSL, heading 090, 200 m/s, UTC 2026-06-21T10:00:00Z, policy `mission_fail`, countdown 30 s,
  `spawn_level` 12).
- `fg hash --world tests/geo-fixture/fixture-small.json --pack target/geo-fixture --script erratic
  --spawn air --assist standard --g full --ticks 36000 --every 600` (the `--world` flag sets arena
  WORLD, builds the world block, applies it, commits the plan's tiles from the pack, then runs).
- `fg world-tiles --world <region.json> --pack <dir> --out <file>` writes the plan's tile records
  exactly as `fg_tile_ptr`'s layout, in plan order; `wasm-hash.ts --world <region.json> --tiles
  <file>` replays them into the WASM core. Both sides therefore commit identical bytes.
- The suite is named `world`, golden `tests/golden/world.json`, same columns as the others.

### Section 8.2 The sim terrain plan (`world::plan`)
Inputs: the BF polygon of the playable area, the frame, the size class, the spawn (φ, λ), the coverage
list (cell x, cell y, max level; a cell not in the list is implicit ocean, max level "complete").
1. `margin_m = 10 000`. The **fine zone** is the disc of 40 km around the spawn.
2. `level_for_cell(c) = 12` if the cell's `max_level ≥ 12`, else `10` (implicit-ocean cells: 10).
3. For Small, Medium and Large: for each z10 tile T whose bounds intersect the playable area expanded
   by `margin_m`: if `level_for_cell(cell(T)) = 12`, add T's sixteen z12 descendants that intersect
   the expanded area; else add T.
4. For Huge and Extreme: as step 3, but z12 descendants only where they also intersect the fine zone;
   z10 elsewhere.
5. Add every z5 tile intersecting the playable area expanded by 200 km (the fallback level).
6. Sort ascending by tile id; remove duplicates. If the count exceeds `SIM_PLAN_MAX_TILES = 8 192`,
   the validator rejects with `TOO_LARGE`.
"Intersects" uses the tile's 9 sample points (4 corners, 4 edge midpoints, centre) at h = 0 projected
to BF (x, y): a tile intersects a region if any sample is inside it, or any region polygon vertex is
inside the tile's lat/lon bounds, or the tile's centre is within `margin` of the region (SDF). This is
conservative; a few extra tiles are harmless.
`fine_level` = 12 if the spawn's cell has z12, else 10; `rest_level` = 10.
**Height lookup** (`SimTerrain::height(lat, lon)`): for z in `[fine_level, rest_level, 5]` (skip
duplicates): `id = tile_id(tile_for(lat, lon, z))`; binary-search `ids`; if found → ocean tiles
return 0.0 with quality `Source`; otherwise the bicubic of its posts with quality `Source`. If no level
is found: 0.0 with quality `Missing`.

### Section 8.4 SDF and boundary (`world::sdf`, `world::boundary`)
- `W = warn_width_m` if > 0, else `max(2 000, 0.03 x shorter side of the polygon's BF bounding box)`.
  `D = max(2 W, 10 000)`. The grid covers the bounding box expanded by `D + 2 cells`, with
  `cells = min(1024, ceil(extent_m / 100))` along the longer side (`extent_m` = longer side of the
  expanded box) and the same cell size `cell_m = extent_m / cells` on both axes.
- Build: (1) for each row, the inside flag of every cell centre by the non-zero winding number
  against all edges (O(rows x edges + cells)); (2) every cell starts at `+∞` distance; for each edge,
  for every cell whose centre lies in the edge's bounding box expanded by D, `d = min(d, distance from
  the cell centre to the segment)`; (3) cells still at `+∞` get `D`; (4) `sdf = −d` inside, `+d`
  outside, clamped to `[−D, D]`. Values are f32 in a `Box<[f32]>` allocated once.
- `sample(x, y)`: bilinear between the four nearest cell centres; outside the grid `+D`. `gradient`
  by central differences of `sample` at ±½ cell; `normal = normalize(gradient)`.
- **Stage machine** (`BoundaryState`, updated once per tick with `dt = 1/60`):
  - `outside = sdf > 0 || alt_msl_m > ceiling_m`; `advisory = !outside && sdf ≥ −W`.
  - if `outside`: `outside_s += dt`; `inside_s = 0`. Else: `inside_s += dt`.
  - Stage: `ENFORCED` latches when the countdown reaches 0 (until the respawn clears the state, or for
    `assist_turn` until the reset); else `COUNTDOWN` if a countdown is running; else `RESTRICTED` if
    `outside`; else `ADVISORY` if `advisory`; else `INSIDE`.
  - A countdown **starts** on the tick when `outside_s` first exceeds 3.0 s; its `remaining_s` starts at
    `countdown_s` and decreases by `dt` on every tick **while outside**; while inside it holds; when
    `inside_s ≥ 5.0` it is **reset** (`remaining` cleared, `outside_s = 0`). Re-exiting before 5 s
    resumes the same countdown.
  - When `remaining_s ≤ 0`: `mission_fail`/`deserter` → event `Lost` (the core destroys the aircraft
    with `crash_cause::OUT_OF_AREA` through the same path a crash uses; whatever the existing crash
    flow does next — an automatic respawn or waiting for the player's respawn action — applies
    unchanged; every respawn in a world session uses the world spawn; `losses += 1`; the boundary
    state resets); `assist_turn` → `assist_active = true` until the reset.
- **Assist-turn pilot**: `Goal { dir_bf: climb_dir(horizontal(to_anchor_bf, up, fallback), up,
  height_gamma(alt0, alt, 800.0, 0.15)), n_cap: 3.0, throttle: 0.8 }` with `to_anchor_bf = −pos_bf`
  (the anchor is the BF origin), `alt0` = the altitude when it engaged, flown by
  `ai_pilot::maneuver::fly_goal` — the same way the `HOLD` script turns `HoldCourse`'s controls into
  input (read `crates/sim-core/src/input.rs` and copy that path).
- **BF2's scripted flight** (constants in `world/boundary.rs` tests): a fixture world of implicit ocean
  only (no data), Small battlefield = rect 50 km x 50 km at 46.55° N 8.0° E; spawn 5 000 m west of the
  east edge (`x = 20 000`, `y = 0`), 3 000 m MSL, heading 090, 200 m/s; script `HOLD`. Predicted
  ticks are taken from the run's own SDF trace: advisory = first tick with `sdf ≥ −W`; restricted =
  first tick with `sdf > 0`; countdown = the first tick with `outside_s > 3.0`; enforcement = countdown
  tick + `countdown_s x 60`. Each recorded stage change must equal its prediction ±6 ticks. For Easy,
  after enforcement the aircraft's `sdf` must become ≤ 0 within 120 s and the stage return to INSIDE
  exactly 300 ticks (±6) after re-entry.

### Section 8.5 The validator (`world::validate`)
| # | Check | Result |
|---|---|---|
| V1 | Polygon simple (no two non-adjacent edges intersect), area ≥ 1 km², extent (largest vertex-to-vertex distance) ≤ 1 000 km, ≤ 64 authored vertices | `Reject::{SelfIntersecting, TooSmall, TooLarge, TooManyVertices}` |
| V2 | Anchor latitude within ±85° | `Reject::PolarAnchor` |
| V3 | Every cell meeting the playable area + 10 km has `max_level ≥ 10` or is implicit ocean | `Reject::NoCoverage` |
| V4 | The fine zone's cells have `max_level ≥ spawn_level` | `WarnFlags::LOW_DETAIL` (proceed with `fine_level` 10) |
| V5 | Spawn inside the playable area (`sdf < 0`) | `Reject::SpawnOutside` |
| V6 | Air spawn ≥ 150 m above the planned terrain under it (checked in `start()`, once tiles are committed) | if lower: raised to 300 m above it, `WarnFlags::SPAWN_RAISED` |
| V7 | Entity budget (GDD 0.6.2) | always passes in v0.2a (one aircraft) |
| V8 | A base or an air spawn exists | passes (air spawn) |
| — | Plan size ≤ 8 192 tiles | `Reject::TooLarge` |
`fg_world_apply` returns `reject_code | (warn_flags << 16)`; reject codes are Appendix A's
`world_status`. A rejected apply leaves the core Idle with no world session.

### Section 8.7 (extract): residency, the coverage check and the descent
- **Residency invariant**: the base tiles (z3–z5) load first and stay pinned; a child is requested
  only when its parent is resident; a tile is drawn refined only if **all four** children are
  resident (else the tile itself is drawn and the missing children are requested); eviction (LRU by
  last frame used) only removes a resident tile with no resident child, never a pinned one, never a
  tile drawn this frame. Implicit-ocean tiles are resident without a fetch and share atlas layer 0
  (all zeros).
- **Coverage check** (every frame in debug and test builds): each leaf of the traversal (drawn or
  culled) at level z adds `4^(13 − z)` to a plain `number` counter (exact: the total is far below
  2^53); the total must equal `2 x 4^13 = 134 217 728` (the 128 z3 roots each weigh `4^10`). A
  mismatch, or a drawn tile with a non-resident ancestor, increments `holeFrames`.
- **The descent** (ST3, ST4; `globe-camera.ts` `descentPose(t)`): target (φ, λ) = the region's
  centre (fixture: 46.55° N, 8.0° E); duration 60 s; eye height above the ellipsoid
  `h(t) = H0 (H1 / H0)^(t / 60)` with H0 = 20 000 km and H1 = 300 m + the terrain height under the
  target (from the base tile); heading 0; pitch −90° until `h = 50 km`, then linearly to −15° at the
  end; then hold 10 s. Frames are read at 0.5 s steps (140 reads).

### Section 9.5 The synthetic fixture (`geo-pipeline synth`)
- **Rule for every archive, real or synthetic: a cell archive is complete to its `max_level`.** Every
  tile of the cell at every level from 6 to `max_level` is either stored or ocean (all posts exactly
  0.0). A tile is left out of the directory only when it is ocean.
- Two synthetic islands (everything else is sea at 0 m): **I1** centred at 46.55° N, 8.0° E and **I2**
  centred at 53.4° N, 180.0° E (it straddles the antimeridian), each of angular radius 0.006 rad
  (≈ 38 km).
- Cells: (33, 7) with `max_level` 12 (I1); (63, 6) and (0, 6) with `max_level` 10 (I2's two halves);
  `base.gpk` with the z3–z5 tiles over them. Every other cell is absent (implicit ocean).
- Height (deterministic, continuous across ±180°, from the unit vector `u = (cos φ cos λ, cos φ sin λ,
  sin φ)` and each island's centre vector `c`): `h(u) = 2 500 m x max(0, 1 − |u − c| / 0.006)² x
  (0.6 + 0.4 n(u))`, where `n(u)` is a 3-D value noise in [0, 1] on an integer-hash lattice of spacing
  0.002 (in unit-vector space), 4 octaves (each half the spacing, half the amplitude, normalised by
  the amplitude sum), trilinear with the smoothstep fade `t²(3 − 2t)`, hashing with integer operations
  only. Write the function once in `synth.rs` with a doc comment; tests pin `h` at five points (the two
  centres, a point 20 km east of each, a sea point).
- Fixture regions (`tests/geo-fixture/`, `region/1`): `fixture-small.json` (section 7.6) and
  `fixture-antimeridian.json` (Medium 120 km square centred 53.4° N 180.0° E; spawn 53.4° N,
  179.75° E, 3 000 m MSL, heading 090, 200 m/s; `mission_fail`, 30 s; `spawn_level` 10).
- Output: `target/geo-fixture/datapack.json`, `base.gpk`, `cells/*.gpk`, `qa.json`, `licences.json`
  (descriptor `synthetic` with `commercial_ok: true`, `terms_checked` = the build date).
- Size target ≤ 6 MB in all (the e2e fixture server serves it; nothing of it is deployed).

### Section 9.6 The fixture server (`packages/tools/src/geo-server.ts`)
Node `http` server on `127.0.0.1:4175` serving a directory with: `GET`, `HEAD`, single-range `Range:
bytes=a-b` (206 with `Content-Range`), CORS (`Access-Control-Allow-Origin` echoing
`http://localhost:4173` or `http://localhost:4174`, `Access-Control-Allow-Headers: Range`,
`Access-Control-Expose-Headers: Content-Range, Content-Length, ETag`), `OPTIONS` preflight, and
`Cache-Control: no-store`. Faults by query-free control endpoints (`POST /__fault` with JSON
`{ "p404": 0.1, "pCorrupt": 0.05, "corruptTile": <id>, "delayMs": 2000, "pDelay": 0.2, "seed": 1 }`,
`POST /__reset`), deterministic per seed. Playwright starts it as a third `webServer`. "Corrupt"
flips one byte inside the requested range at a deterministic position.

### Section 9.7 What the e2e and perf specs use
The six real regions, served by the fixture datapack: `?world=alps&geo=http://localhost:4175/` works
because the fixture's cell (33, 7) is complete to z12 (its island lies inside the Alps battlefield;
the rest of the plan is ocean). Debug flags (only with `?debug=1`): `bfprobe=1` replaces the region's
battlefield by BF2's Small square and spawn (section 8.4); `fault=drop-children` (draw nothing where a
refinement lacks children: ST3 must then fail), `fault=stream-crash`, `fault=no-atmosphere`.

### Section 13.3 The owner's look (non-blocking; appended to `docs/runbooks/reference-machines.md` as Part 5)
About 20 minutes on staging, Chrome, mains power: (1) Globe: rotate, zoom from orbit to the Alps,
Norway and Tokyo Bay; is the day/night line where it should be now? (2) Each of the six regions:
Fly, look around in the cockpit and chase views; does the place look like itself from 3 km up? (3)
Fly out of a battlefield: are the warnings, tone, countdown and loss clear? (4) Settings → Data →
Credits & data is readable. Notes go into the exit doc, item 15.


## Alternatives considered

- *World depth:*
  - worldwide z12 from GLO-30 now. About ten times the data and the build time of z10, and `AC-3.1`
    needs it only at v0.2b;
  - the six regions only, with no world. That fails the roadmap's exit 1, "seamless zoom from orbit
    to the ground", anywhere but six places.
- *Where packs are built:* on the owner's Mac. It ties every rebuild to one machine, and the full
  build takes many hours there. GitHub Actions runs the cells in parallel and keeps the R2 keys out
  of the agent's hands.
- *The public address:* a custom domain now. It needs a domain on Cloudflare first. The `r2.dev`
  address is rate-limited but enough for one developer, and the change is one constant
  (`GEO_DEFAULT_BASE_URL`) and the CSP.
- *Staging:* a separate world page instead of the regions in the menu. It would make the regions
  harder to reach and test the app flow less.
- *Gates on a human look:* not allowed for a solo project (OQ-12, ADR-50). The owner's look (OW) is
  recorded only.

## Consequences

- No v0.2a code is written before this ADR. A gate that cannot pass as written is a finding to
  report and decide with the owner, never a reason to change the threshold (AC-66.4).
- The five existing goldens must not move (WS8). If one would, the work stops and the owner is
  asked.
- The new `world` golden is blessed in its own commit. After merge 1 it is re-blessed only with the
  owner's OK.
- ADR-52 to ADR-56 record the designs these gates test: geodesy and world sessions, the battlefield,
  GeoPack and the pipeline, hosting and delivery, and world rendering. Each is Proposed in the work
  package that builds it, and they are accepted together with this ADR at the exit.
- A cut from the plan's cut list (the curtain, the GeoInspector's picking, the look-ahead want
  lists, sub-LOD 32, the Stream Worker's restart) needs a note here and the owner's OK, because the
  gates are fixed.
