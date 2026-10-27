# The flight model

How an aircraft flies in the game, where the code is, and how to add or check an aircraft. The
decisions behind it are in [ADR-29](../adr/0029-flight-crates-and-integration.md) (crates and
integration), [ADR-30](../adr/0030-aircraft-data-format-and-cpack-v2.md) (data format),
[ADR-31](../adr/0031-performance-conformance-test-method.md) and
[ADR-32](../adr/0032-ground-roll-conformance-method.md) (conformance),
[ADR-35](../adr/0035-ground-model-and-test-airfield.md) (the ground) and
[ADR-38](../adr/0038-g-physiology-departure-and-structure.md) (the pilot's body, departure and
structure); the physics are GDD 11 and the Kestrel's numbers are GDD Appendix C. The control laws
that fly it are in [fcs.md](fcs.md).

## Where the code is

```text
libm ─▶ simmath ─▶ flight ─▶ fcs ─▶ flight-test ─▶ cli (fg conformance, circuit, hash, bench)
           │          │        │                          ▲
content ───┴──────────┴────────┴──▶ sim-core ─▶ sim-wasm  │
                                       └───────────────────┘
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
| `env` | `FlightEnv`, what the model asks of the world (heights, up, gravity, wind, and `surface()` for the wheels), and the analytic test worlds `SphereSea` and `FlatSea` |
| `frames` | Euler angles from the body quaternion and back |
| `controls` | `PilotControls` (the stick, throttle, levers and wheel brakes), the `ControlLaw` trait, `ManualLaw` and the pitch trim |
| `gear` | The ground model: massless struts, tanh-regularised friction, nose-wheel steering and brakes, the airframe's contact points, the static pose, and the touchdown and crash events (ADR-35) |
| `surface` | `SurfaceKind` and the GDD 10.4 friction table (`// DATA`, moves to world content at M2) |
| `physio` | The pilot's g-physiology: `G_f`, stress, fatigue, red-out, G-LOC, and the effects of the dial (GDD 11.14) |
| `structure` | Configuration load limits and the over-stress, over-speed, gear and flap speed clocks (nothing breaks before the damage chapter) |
| `departure` | The dynamic directional stability parameter and the lateral control departure parameter (informational) |
| `aircraft` | `AircraftModel` (built once from content), `AircraftState` (with its levers and ground state), `step_tick`, `Telemetry`, and `Sense` (what the control laws read) |
| `trim` | Damped Newton trim for level flight, fixed throttle, glide and coordinated turns |
| `linearise` | Longitudinal and lateral state matrices, eigenvalues and modes (informational) |
| `energy` | Specific energy, specific excess power, turn rate and radius, wind-axis load factor |

`crates/flight-test/src/` is the Performance Conformance Test: `pilot` (the test pilot),
`procedures/` (level acceleration, stall, sustained turn, fuel-flow bench, the two ground rolls),
`report`
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

`sim-core` runs the aircraft in the Dynamics stage of each 60 Hz tick (`pilot.rs` is the pilot's
side of it, ADR-36):

```text
OwnshipDynamics
  RESET press → respawn (the frame's spawn request, else the session's); spawn_seq += 1
  levers: gear (up refused on the wheels), flaps, parking brake, trim reset
  assist request ≠ level in force → bumpless switch
  input frame → PilotControls; the spawn throttle is held until the host acknowledges
  the pilot's body: a blackout delays stick and pedals, G-LOC zeroes them
  FcsModel::step → Commands (surfaces, engine lever, gear, flaps, wheel brakes, steering)  once per tick
  AircraftModel::step_tick
    mass properties from the tank contents                           once per tick
    saved = state                                                    rollback copy
    2 sub-steps of 1/120 s:
      actuators (the gear moves at its own rate), then engine spool, AB stage and failures
      RK4 over (p, v, q, ω); every stage evaluates h_MSL, up, gravity, wind, ISA, air data,
        α̇, aero force and moment, thrust, and the gear force and moment
      renormalise q; burn fuel in feed order
      ground events: touchdowns, hard landings, collapses, destruction, weight on wheels
      fault check: non-finite, |ω| > 50 rad/s, V > 3 000 m/s
    on a fault: restore, neutral surfaces, retry once; a second fault sets numerical_fault
  physiology and structure take the tick's load factor
TerrainContact
  ground events → counters, the destroyed cause and the 2 s respawn countdown; a numerical_fault
  respawns the same way
Publish
  snapshot v3: the ownship block with position, attitude, air data, thrust, fuel, mass, surfaces,
  levers and wheels, the pilot's state, the control system's mode and the warning flags
  (docs/architecture/runtime.md)
```

**The ground.** Each strut is a massless spring and damper along the surface normal, evaluated in
every RK4 stage as a pure function of the rigid state; nothing is decided inside a stage. State
changes (a strut collapsing, the aircraft being destroyed, a hard landing) are decided once per
sub-step on the accepted state. Friction is the GDD 11.13.2 model with a `tanh` regularisation on
every tangential term, so a parked aircraft stays parked and nothing chatters at rest. A contact is
judged by the speed into the ground of the first contact of a touchdown: up to 3 m/s normal, to
4.5 m/s a hard landing (counted, flagged for 3 s), to 7 m/s that strut collapses, above that
destroyed; an airframe contact above 30 m/s of ground speed is destroyed, below it a skid. A CG more
than 5 m under the ground is a numerical failure (`Tunnelling`), not a crash.

**Spawns** (`sim-core/src/spawn.rs`). *Runway*, the game's default: 150 m past the threshold of the
test airfield's runway (3 000 m by 45 m, heading 000, 40 m MSL, a pad in a basin of the procedural
terrain, `seams/geo.rs`), at rest on the struts with the static pose from `gear::static_pose`
(pitch about 3°), engine idle, parking brake on, gear down, take-off flaps, 60 % fuel. *Air*: 2 000 m
above the threshold at 200 m/s, trimmed level, gear and flaps up. If trim fails the aircraft starts
at α 2° and throttle 0.6, and `trim_failures` is counted. Respawning resets the same slot, so neither
a respawn nor a trim allocates.

Without aircraft content (tests, the benchmark's spectator mode, `fg hash` without `--content`)
the ownship is the v0.0b observer, flagged `spectator`.

**The pilot's body** (ADR-38). `G_f` follows the load factor at the CG with a 2 s lag; stress
grows above `G_tol` (6 g with no fatigue, falling as fatigue builds, and 1.5 g higher on the Reduced
dial) and recovers over 4 s.
At 0.7 the controls arrive 300 ms late; at 1.0 the pilot is out for 6 s and returns over 4 s more.
Red-out follows load factors below -1 g. The dial (Off, Reduced, Full) decides what is applied,
and the picture's strength (0 to 100 %) is the renderer's own setting.

## Adding an aircraft

An aircraft is data only (ADR-13, ADR-30); no code changes.

1. **Write the files** in a pack, e.g. `content/core/`:
   - `aircraft/<name>.json` (`aircraft/1`): mass and tanks, geometry (including the `gear` legs
     and the airframe `contacts`), engine installations, aero limits and configuration increments
     (with `n_max_by_config`), the reference to its FCS gains, actuators (the gear's transit time)
     and the performance envelope;
   - `fcs/<name>.json` (`fcs/1`): the control gains; see `content/core/fcs/README.md` and
     [fcs.md](fcs.md);
   - `engines/<name>.json` (`engine/1`), or refer to an existing engine;
   - `aero/<name>.json` (`aero/1`): the coefficient tables. A generator next to them, like
     `aero/kestrel_gen.rs`, is recommended; a test then keeps the JSON equal to its output.

   The schemas are in `content/schemas/`. Every number must be exactly representable as `f32`,
   and every key carries a unit suffix (`_m`, `_deg`, `_kg_per_ns`, …) or is listed as
   dimensionless.
2. **Bake** with `cargo xtask content`. The baker checks references, units, grid monotonicity
   and coverage (α −30° to 90°, Mach 0 to 3), the lift peak against `C_L,max`, the post-stall
   blend start, spikes, the sign of `C_mα` and the static margin, and the gear: the nose wheel's
   share of the load (5 to 20 % over the CG range), static compression (20 to 60 % of the
   stroke), strut bounce below 15 rad/s with a damping ratio from 0.15 to 1, tail clearance at the
   touchdown attitude, tip-back, every contact inside the bounding box, a braked leg, at most one
   steerable leg. Every error names the file, a JSON pointer and a fix.
3. **Fly the envelope**:
   `fg conformance --content content-out/core.cpack --aircraft-index N --traces target/conformance/traces`.
   Aircraft are indexed in handle order, which is id order. The two ground rolls need a braked main
   leg; a model without one defers them. Then fly the circuit under every assist
   (`cargo xtask circuit`, [fcs.md](fcs.md)).
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
| `cargo test -p flight` (gear) | The static pose, drops at 1, 3.5, 5 and 8 m/s, the parking brake, braking against μ(W − L), the nose-wheel schedule, friction through zero without chatter, the friction circle, anti-skid and lock-up, a skid at 25 m/s and destruction at 35 m/s, a bounce's energy, 600 s of taxiing |
| `cargo test -p flight` (pilot's body) | `AC-11.9`: blackout at 9.5 s and G-LOC at 11.9 s of a 9 g pull, a 3 s pull below 0.05; the dials; over-stress; the sign change of C_nβ,dyn near 34°; the control model against `evaluate` at 10⁴ states |
| `fg conformance` | Point performance against C.9 (`AC-3.2`), including the two ground rolls |
| `cargo xtask circuit` | The tester's take-off, pattern and landing under each assist ([fcs.md](fcs.md)) |
| `cargo xtask determinism` | flight60 (`AC-11.7`), the 10⁶-tick erratic run and the circuit: native = debug = WASM single = WASM threads = golden |
| `crates/sim-wasm/tests/{no_alloc,flight_no_alloc,fcs_no_alloc}.rs` | Any allocation in a tick with a live aircraft: a crash and a respawn, the runway spawn, a take-off roll, the gear, a touchdown, a hard landing, every assist switch |
| `clippy.toml` | `f64::sin` and friends outside `simmath::dmath` (ADR-10) |
| `cargo xtask check-layers` | A crate depending on something its layer forbids |
