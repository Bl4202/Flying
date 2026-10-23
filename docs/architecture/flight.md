# The flight model

How an aircraft flies in the game, where the code is, and how to add or check an aircraft. The
decisions behind it are in [ADR-29](../adr/0029-flight-crates-and-integration.md) (crates and
integration), [ADR-30](../adr/0030-aircraft-data-format-and-cpack-v2.md) (data format) and
[ADR-31](../adr/0031-performance-conformance-test-method.md) (conformance); the physics are GDD 11
and the Kestrel's numbers are GDD Appendix C.

## Where the code is

```text
libm ─▶ simmath ─▶ flight ─▶ flight-test ─▶ cli (fg conformance, fg hash, fg bench)
           │          │                        ▲
content ───┴──────────┴──▶ sim-core ─▶ sim-wasm│
                              └────────────────┘
```

`crates/flight/src/`:

| Module | What it does |
|---|---|
| `physical` | Physical constants, each marked `// PHYS:` with its source |
| `atmosphere` | ISA (GDD 11.3): the analytic three-layer model as the oracle, and a 1 025-node table over geopotential 0–32 km for the tick; non-standard days; density altitude |
| `airdata` | TAS, EAS, CAS (subsonic and Rayleigh formulas), Mach, dynamic pressure, α, β, and non-dimensional rates (GDD 11.5) |
| `table` | `Table1D` and `Table2D`: monotone grids, clamped multilinear lookup, no allocation |
| `aero` | The `AeroModel` seam and `TableAero`, the GDD 11.6.1 build-up: base lift and moment tables, drag polar with wave drag, controls with power fall-off, damping, post-stall blend, ground effect, configuration increments, wind-to-body transform, moment transfer to the CG |
| `actuators` | Exact first-order lag with rate and range limits, per surface (GDD 11.7) |
| `engine` | The `EngineModel` trait and `Turbofan`: throttle map with the military detent and AB stages, spool, AB light-off and cut-off, thrust lapse with density, Mach and inlet α, fuel flow, flameout and compressor stall (GDD 11.8) |
| `mass` | Tanks, feed order, and mass, CG and inertia by the parallel-axis rule (GDD 11.9) |
| `body` | The rigid-body equations (GDD 11.2.2) and one RK4 step |
| `env` | `FlightEnv`, what the model asks of the world, and the analytic test worlds `SphereSea` and `FlatSea` |
| `frames` | Euler angles from the body quaternion and back |
| `controls` | `PilotControls`, the `ControlLaw` trait, `ManualLaw` and the pitch trim |
| `aircraft` | `AircraftModel` (built once from content), `AircraftState`, `step_tick`, telemetry |
| `trim` | Damped Newton trim for level flight, fixed throttle, glide and coordinated turns |
| `linearise` | Longitudinal and lateral state matrices, eigenvalues and modes (informational) |
| `energy` | Specific energy, specific excess power, turn rate and radius, wind-axis load factor |

`crates/flight-test/src/` is the Performance Conformance Test: `pilot` (the test pilot),
`procedures/` (level acceleration, stall, sustained turn, fuel-flow bench), `report`
(`conformance_report/1`) and `lib.rs` (`run`). `crates/sim-core` holds what is about the game
rather than the physics: `ownship` (aircraft or spectator), `spawn`, `systems` (the
`OwnshipDynamics` and `TerrainContact` stages) and `services::ServicesEnv`, which is the
`FlightEnv` over the geo and weather seams.

## Frames, units and signs

SI units throughout and radians internally; degrees appear only in data files and on screen.
Every vector is named with its frame: `pos_bf`, `vel_body`, `omega_body`, `force_body_n`.

- **BF** (battlefield) is East-North-Up and curved: `up` depends on position (ADR-04). Height is
  always h_MSL from the geo service and vertical speed is `v·up(p)`, never BF `z`.
- **BODY** is forward-right-down, with its origin at the **reference point** (the empty CG, 27 %
  MAC on the Kestrel). Data positions (tanks, engines, the CG) are body axes from that point.
- The state integrates the CG. The snapshot reports the reference point, `p_cg − R(q)·r_cg`, so
  the camera does not drift as fuel burns.

| Quantity | Positive means | Test |
|---|---|---|
| α | nose above the airflow | `airdata::flow_angles_follow_the_frd_conventions` |
| β | wind from the right | same |
| Elevator δe | trailing edge down, so a nose-down moment (`C_mδe < 0`) and more lift | `aero::controls_and_rates_have_the_documented_signs` |
| Aileron δa | right wing down (`C_lδa > 0`) | same |
| Rudder δr | trailing edge left, so nose left (`C_nδr < 0`) and side force right (`C_Yδr > 0`) | same |
| Pitch input | stick back (S, or the pad stick pulled) is nose up: `δe = trim − s·δe_max` | `controls::manual_law_signs` |
| Roll and yaw input | stick right is right wing down; right pedal is nose right | same |
| Pitch, roll, heading | nose up; right wing down; clockwise from north | `frames::angles_have_the_aerospace_signs` |

Lateral linear terms use `sin β`, so they saturate rather than grow without limit at large β.

## One tick

`sim-core` runs the aircraft in the Dynamics stage of each 60 Hz tick:

```text
OwnshipDynamics
  input frame → PilotControls (stick, throttle lever, trim buttons)   once per tick
  ManualLaw → surface commands around the held pitch trim (2 °/s)    once per tick
  AircraftModel::step_tick
    mass properties from the tank contents                           once per tick
    saved = state                                                    rollback copy
    2 sub-steps of 1/120 s:
      actuators, then engine spool, AB stage and failures
      RK4 over (p, v, q, ω); every stage evaluates h_MSL, up, gravity, wind, ISA, air data,
        α̇, aero force and moment, thrust
      renormalise q; burn fuel in feed order
      fault check: non-finite, |ω| > 50 rad/s, V > 3 000 m/s
    on a fault: restore, neutral surfaces, retry once; a second fault sets numerical_fault
TerrainContact
  reference point below terrain + 1.5 m → crashed; a crash or numerical_fault respawns after
  120 ticks (2 s), and R respawns at once
Publish
  snapshot v2: the ownship block with position, attitude, air data, thrust, fuel, mass,
  surfaces, AB stage and flags (docs/architecture/runtime.md)
```

The spawn is 6 km south of the terrain patch centre, 2 000 m above the ground there, heading
north at 200 m/s with 60 % fuel. The trim solver sets throttle and pitch trim for level flight
(`sim-core/src/spawn.rs`, `SpawnTuning`). If trim fails, the aircraft starts at α 2° and throttle
0.6, and `trim_failures` is counted. Respawning resets the same slot, so neither respawn nor trim
allocates.

Without aircraft content (tests, the benchmark's spectator mode, `fg hash` without `--content`)
the ownship is the v0.0b observer, flagged `spectator`.

## Adding an aircraft

An aircraft is data only (ADR-13, ADR-30); no code changes.

1. **Write the files** in a pack, e.g. `content/core/`:
   - `aircraft/<name>.json` (`aircraft/1`): mass and tanks, geometry, engine installations,
     aero limits and configuration increments, FCS data, actuators and the performance envelope;
   - `engines/<name>.json` (`engine/1`), or refer to an existing engine;
   - `aero/<name>.json` (`aero/1`): the coefficient tables. A generator next to them, like
     `aero/kestrel_gen.rs`, is recommended; a test then keeps the JSON equal to its output.

   The schemas are in `content/schemas/`. Every number must be exactly representable as `f32`,
   and every key carries a unit suffix (`_m`, `_deg`, `_kg_per_ns`, …) or is listed as
   dimensionless.
2. **Bake** with `cargo xtask content`. The baker checks references, units, grid monotonicity
   and coverage (α −30° to 90°, Mach 0 to 3), the lift peak against `C_L,max`, the post-stall
   blend start, spikes, the sign of `C_mα` and the static margin. Every error names the file, a
   JSON pointer and a fix.
3. **Fly the envelope**:
   `fg conformance --content content-out/core.cpack --aircraft-index N --traces target/conformance/traces`.
   Aircraft are indexed in handle order, which is id order.
4. **Tune data, not targets** (ADR-31). Log every change, the point it moved and by how much in
   the aero README (see `content/core/aero/README.md`).

The live game flies the **lowest-handle** aircraft. A new aircraft whose id sorts before
`core:ac/kestrel` therefore becomes the player's aircraft, and the determinism goldens must then
be re-blessed with a reason (ADR-23).

## Reading the conformance report

`cargo xtask conformance` bakes, builds `fg` in release, flies every point and applies the gate.
`pnpm conformance` does the same, and so does the `conformance` CI job, which uploads the JSON
and the traces. The Markdown table on stdout:

| Column | Meaning |
|---|---|
| Point, Metric | The envelope entry (C.9) and what it measures (`max_mach`, `roc_mps`, `stall_mps`, `n_sust`, `turn_dps`, `ff_kgps`, `roll_m`) |
| Target, Measured | The authored value and what the procedure measured, continuous (not floored to C.8's 0.01 Mach grid) |
| Error, Tol | Signed error and the point's tolerance, percent |
| Used | `|error| / tol`. Above 60 % prints a warning: the point passes, but there is little room for later changes |
| Cross-check | A static solution of the same point (fixed-throttle trim, minimum-speed trim, turn trim or analytic TSFC) and its difference from the measurement. `OUT` means the procedure and the static answer disagree beyond the band, which points to a method fault rather than a data fault |
| Status | `pass`, `fail`, `deferred` (needs a model that does not exist yet), `no_convergence` or `timeout` |

The run fails on any failing point, any cross-check outside its band, or a deferred set other
than the expected one (empty since v0.1b: `to_roll` and `ldg_roll` are flown on the gear model, ADR-32; a model without a braked main leg still defers them). The JSON
(`--json PATH`) adds the procedure, the simulated seconds and `settle_stats` for each point: how
well altitude and load factor were held, the number of valid samples, and whether the starting
trim converged. The 10 Hz traces (`--traces DIR`, one CSV per procedure) hold time, height,
airspeed, Mach, α, β, load factor, bank, throttle, thrust, fuel flow, P_s, surface positions and
body rates. Plot one when a point moves unexpectedly.

## Checks that guard the model

| Check | Catches |
|---|---|
| `cargo test -p flight` | ISA rows (`AC-11.5`), quaternion norm over 10⁶ steps (`AC-11.6`), energy in a drag-free zoom (`AC-11.4`), RK4 order, torque-free spin, trim at 50 points (`AC-11.1`), the elevator envelope, sign conventions, NaN fuzz |
| `fg conformance` | Point performance against C.9 (`AC-3.2`) |
| `cargo xtask determinism` | flight60 (`AC-11.7`) and the 10⁶-tick erratic run: native = debug = WASM single = WASM threads = golden |
| `crates/sim-wasm/tests/no_alloc.rs` | Any allocation in a tick with a live aircraft, a crash and a respawn |
| `clippy.toml` | `f64::sin` and friends outside `simmath::dmath` (ADR-10) |
| `cargo xtask check-layers` | A crate depending on something its layer forbids |
