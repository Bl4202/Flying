# crates

Rust workspace (GDD §61.2–§61.3).

| Crate | What |
|---|---|
| `simmath` | deterministic `f64` maths: `DVec3`, `DQuat`, `DMat3` and the software-`libm` wrappers in `dmath`. Depends only on `libm` |
| `flight` | the flight model (GDD 11): ISA atmosphere, air data, tables, table aerodynamics, propulsion, mass and fuel, actuators, the 6-DOF rigid body with RK4 at 120 Hz, the landing gear and ground contact (ADR-35), the pilot's g-physiology, structural limits and departure diagnostics (ADR-38), trim and linearisation. Depends only on `simmath` and `content`; knows nothing of the tick schedule or the browser |
| `fcs` | the flight control system (GDD 11.11, ADR-34): the four assist levels (Full, Standard, Reduced, Manual), the control laws (incremental dynamic inversion against the aircraft's own tables), limiters, ground mode, mouse-aim and recovery, and the virtual tester that flies the exit circuit (ADR-33). Depends on `flight`, `simmath`, `content` |
| `flight-test` | the Performance Conformance Test (GDD 11.19, ADR-31): a test pilot, the flight-test procedures (including the two ground rolls, ADR-32) and the `conformance_report/1` report. Depends on `flight`, `simmath`, `content`, `serde_json` |
| `sim-core` | the deterministic core: tick schedule, the session config, the ownship (the aircraft, or the observer as a spectator), runway and air spawns, the pilot's side of the tick, crash and respawn, input, snapshot, hashes, the circuit report, the 16 seams and the test airfield in the procedural world. Depends only on `content`, `simmath`, `flight` and `fcs` (it re-exports `math` and `dmath`, and `flight::aero` as the Aero seam) |
| `sim-wasm` | the hand-written C ABI (v3) the core is built into for the browser; the only crate with `unsafe` (see `UNSAFE.md`), so it also hosts the no-allocation tests |
| `content` | the dependency-free `.cpack` reader and writer and the registry |
| `protocol` | wire types and the `Transport` seam with a loopback implementation |
| `cli` | `fg`: `fg hash` (state hashes), `fg bench` (ns per tick; `--aircraft N [--assist] [--ground]` per aircraft), `fg conformance` (ADR-31, ADR-32), `fg circuit` (ADR-33) |
| `server` | the dedicated-server stub (`--selftest` sends a hello through the loopback transport) |
| `xtask` | `cargo xtask`: build, check and orchestration (`wasm`, `content`, `determinism`, `conformance`, `circuit`, `repro`, `bench`, `gen-abi`, `check-layers`) |

The offline content pipeline is `tools/content-bake`.

Reserved names (not created yet): `geo`, `world`, `damage`, `weapons`, `sensors`,
`ew`, `ai-pilot`, `ai-squadron`, `ai-mission`, `ai-strategic`, `mission-core`, `mission-gen`,
`director`, `campaign`, `net-proto`, `net-sim`, `savegame`, `replay`.
