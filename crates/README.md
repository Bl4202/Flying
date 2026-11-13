# crates

Rust workspace (GDD §61.2–§61.3).

| Crate | What |
|---|---|
| `simmath` | deterministic `f64` maths: `DVec3`, `DQuat`, `DMat3` and the software-`libm` wrappers in `dmath`. Depends only on `libm` |
| `geo` | WGS84 geodesy, frames, tiles, height codec, sampler, `GeoPack` layout (GDD 7, 44.3, ADR-52, ADR-54): geodetic ↔ ECEF, the curved battlefield frame on the ellipsoid and its up vector, haversine, bearings and the Vincenty inverse, the geodetic quadtree's tile ids, bounds and cells, the 16-bit height codec, the Catmull–Rom sampler, CRC-32C, and the `.gpk` header and directory layout (no decompression). A sim crate: depends only on `simmath`, and `geographiclib-rs` only as a dev-dependency (the reference of gate GE7) |
| `world` | the battlefield system (GDD 8, ADR-53): the authored shapes and their polygons, the anchor (the ECEF mean of the densified boundary), the signed distance field, the five size presets and corridor sizing, the validator, the four boundary stages, the sim terrain plan and the world configuration block. A sim crate: depends only on `geo` and `simmath` |
| `geo-pack` | `GeoPack` v1 (ADR-54): the archive writer (deterministic, tile-id order) and the reader with inflate, which checks every rule of the format. Not a sim crate; depends on `geo` and `miniz_oxide` |
| `flight` | the flight model (GDD 11): ISA atmosphere, air data, tables, table aerodynamics, propulsion, mass and fuel, actuators, the 6-DOF rigid body with RK4 at 120 Hz, the landing gear and ground contact (ADR-35), the pilot's g-physiology, structural limits and departure diagnostics (ADR-38), trim and linearisation. Depends only on `simmath` and `content`; knows nothing of the tick schedule or the browser |
| `fcs` | the flight control system (GDD 11.11, ADR-34): the four assist levels (Full, Standard, Reduced, Manual), the control laws (incremental dynamic inversion against the aircraft's own tables), limiters, ground mode, mouse-aim and recovery, and the virtual tester that flies the exit circuit (ADR-33). Depends on `flight`, `simmath`, `content` |
| `weapons` | weapon physics as pure functions (GDD 15, ADR-46): gun ballistics (RK2), the director gunsight, swept hit tests against capsules, the IR signature and the rear-aspect seeker, the 3-DOF PN missile (RK4 at 120 Hz), the proximity fuze and the fly-out predictor. Depends on `content`, `flight`, `simmath`; `sim-core` keeps the pools, timers and random streams |
| `ai-pilot` | the pilot AI (GDD 19, ADR-47): engagement geometry, the manoeuvre controller (goals to a player's sticks through `fcs::tester::sticks::air_n`), skills, the bandit's state machine, the straight-and-level test pilot and the bot playtester. Depends on `content`, `fcs`, `flight`, `simmath`; `sim-core` builds its percepts and turns its output into input frames |
| `flight-test` | the Performance Conformance Test (GDD 11.19, ADR-31): a test pilot, the flight-test procedures (including the two ground rolls, ADR-32) and the `conformance_report/1` report. Depends on `flight`, `simmath`, `content`, `serde_json` |
| `sim-core` | the deterministic core: tick schedule, the session config, the ownship (the aircraft, or the observer as a spectator), runway and air spawns, the pilot's side of the tick, crash and respawn, input, snapshot, hashes, the circuit report, the 16 seams and the test airfield in the procedural world, and the duel (`combat/`, ADR-48: the bandit, the weapons' state, rounds, missiles, hits, perception, events, the combat hash, the snapshot's combat block and the gate runners of `fg sortie`). Depends only on `content`, `simmath`, `flight`, `fcs`, `weapons` and `ai-pilot` (it re-exports `math` and `dmath`, and `flight::aero` as the Aero seam) |
| `sim-wasm` | the hand-written C ABI (v5) the core is built into for the browser; the only crate with `unsafe` (see `UNSAFE.md`), so it also hosts the no-allocation tests |
| `content` | the dependency-free `.cpack` reader and writer and the registry |
| `protocol` | wire types and the `Transport` seam with a loopback implementation |
| `cli` | `fg`: `fg hash` (state hashes), `fg bench` (ns per tick; `--aircraft N [--assist] [--ground]` per aircraft), `fg conformance` (ADR-31, ADR-32), `fg circuit` (ADR-33), `fg sortie` (the duel's gates BP1, BP2, MS5 and the AI gates, ADR-45) |
| `server` | the dedicated-server stub (`--selftest` sends a hello through the loopback transport) |
| `xtask` | `cargo xtask`: build, check and orchestration (`wasm`, `content`, `determinism`, `golden-diff`, `conformance`, `circuit`, `sortie`, `repro`, `bench`, `gen-abi`, `check-layers`) |

The offline content pipeline is `tools/content-bake`.

Reserved names (not created yet): `damage`, `sensors`,
`ew`, `ai-squadron`, `ai-mission`, `ai-strategic`, `mission-core`, `mission-gen`,
`director`, `campaign`, `net-proto`, `net-sim`, `savegame`, `replay`.
