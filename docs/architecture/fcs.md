# The flight control system

What stands between the pilot's stick and the surfaces, how to tune it, and the virtual tester that
flies the exit circuit. The decisions behind it are [ADR-34](../adr/0034-flight-control-system.md)
(the laws), [ADR-33](../adr/0033-v0-1b-gates.md) (the gates it is judged by) and
[ADR-38](../adr/0038-g-physiology-departure-and-structure.md) (the onboard control model and the
pilot's body); the requirements are GDD 11.11, 11.13 and 58.4. The airframe it flies is described in
[flight.md](flight.md), the controls that drive it in [controls.md](controls.md).

## Where the code is

```text
flight ─▶ fcs ─▶ flight-test, sim-core, cli
```

`crates/fcs/src/` (it depends on `flight`, `simmath` and `content`, and names no content entry):

| Module | What it does |
|---|---|
| `lib.rs` | `PilotInput`, `FcsModel::step` (once per tick), `switch_assist` |
| `model.rs` | `FcsModel`: the gains of the `fcs/1` entry the aircraft refers to, and the limits it reads from the airframe |
| `state.rs` | `FcsState` (`Copy`, hashed, fixed word layout), `Assist`, `Mode`, `Limiting`, `RecoverPhase` |
| `sense.rs` | `Sensed`: what the laws read, derived from the airframe's `Sense` (bank about the velocity vector, the turn load, the stability-axis yaw rate) |
| `inverse.rs` | The incremental inversion: pitch by the elevator, roll and yaw by the 2 x 2 aileron-rudder solve, with the rate-limit lead and the anti-windup |
| `standard.rs`, `full.rs`, `reduced.rs` | The three assisted laws and their limiters |
| `ground.rs` | The ground law: weight-on-wheels debounce, elevator about the trim, nose-wheel steering, brakes and the blends |
| `recover.rs` | The recovery button |
| `mouse_aim.rs` | The aim direction turned into stick (Full and Standard) |
| `tester/` | The virtual tester: `geom` (the circuit's geometry), `sticks` (a demand turned into the stick each assist reads), `mod` (the phases) |

Auto-GCAS is not built (the flag and the `gcas` data block exist; the feature is off). The GDD
11.18 autotuner is not built either; the handling suite is the gate.

## One tick

`sim-core/src/pilot.rs` runs it after the buttons and the assist request:

```text
levers (gear, flaps, parking brake) ─▶ PilotControls ─▶ pilot's body (blackout delay, G-LOC) ─▶
PilotInput { controls, aim, recover, gloc } ─▶ FcsModel::step ─▶ Commands ─▶
AircraftModel::step_tick (2 x RK4 at 120 Hz, the commands held) ─▶ physiology, structure
```

The control system runs once per 60 Hz tick and its output is held over both sub-steps. It reads
the airframe through `sense_light` (angular acceleration, forces without the wheels' share, the
control power from `TableAero::control_model`) and allocates nothing
(`crates/sim-wasm/tests/fcs_no_alloc.rs`). Its `Mode` is in the snapshot: Flight, Ground, Blend,
Recover, Gcas (unused), Gloc.

## The laws

| Level | Pitch stick | Roll stick | Limiters |
|---|---|---|---|
| **Full** | flight-path rate (centred holds the path) | bank angle, up to 60° (centred levels the wings) | the Standard limiters, alpha 2° tighter; wings level and no descent in G-LOC |
| **Standard** (default) | load factor, 1 g at the centre | roll rate, shrunk by dynamic pressure, alpha, load factor and inertia coupling | alpha, load factor (4 g with gear or flaps out), overspeed soft wall |
| **Reduced** | pitch rate | roll rate | none; warnings only; a yaw damper |
| **Manual** | the elevator, about the trim | the aileron | none (`ManualLaw`) |

Standard and Full coordinate the turn (the pedals command sideslip). Every law but Manual inverts
the angular acceleration incrementally: the surfaces move by `B^-1 (wanted - measured)`, so there
is no gain schedule over dynamic pressure; ADR-34 has the detail and why. On the wheels every law
but Manual becomes the ground law, blended over 0.5 s at touchdown and 2 s after lift-off; from the
first contact of a wheel the flight law is frozen so the jolt is not a disturbance for it to cancel.
Switching the level in flight is bumpless (the new law starts from the present state; the surface
command blends over 0.5 s). The recovery button (`U`) levels the wings and climbs in Full and
Standard, and in Reduced and Manual does anti-spin only if the aircraft has departed (a stability-axis
yaw rate above 30°/s, or alpha above 35°).

## Mouse-aim

`mouse_aim.rs` takes the world-frame aim (azimuth clockwise from north, elevation up) that the page
sends in the cursor axes and turns it into stick: the roll stick points the lift at the target, the
pitch stick makes the pitch rate follow the lift needed. It exists in Full and Standard only. It
runs in the sim so that AC-58.4 can be tested headless: `crates/fcs/tests/mouse_aim.rs`.

## Tuning

The gains are data: `content/core/fcs/kestrel.json` (`fcs/1`, schema in `content/schemas`). The
file's README (`content/core/fcs/README.md`) lists each key and keeps the **tuning log**; every
change made to a gain goes there with the test that moved and by how much. Constants that are the
laws' structure (`BRAKE_SHARE`, `STOP_DELAY_S`, `ALPHA_LEAD_S`, `COUPLING_SHARE`, the mouse-aim
blends) are `// ALGO:` in the code and change only with an amendment to ADR-34.

To tune:

1. Change one block of `kestrel.json` and run `cargo xtask content` (it bakes and checks).
2. Run the suite that judges it, in release: `cargo test -p fcs --release`.

   | Test file | Judges | Gate |
   |---|---|---|
   | `handling.rs` | H1 to H12 over the grid of Mach, height and mass: load-factor and roll-rate steps, no limit cycle, sideslip in a roll, the three limiters, Full's path and bank holds, Reduced's pitch rate | ADR-33 section 2 |
   | `transitions.rs` | ground-flight blends and assist switches (H11, H12) | surface step under 2° and 1°, load-factor transient under 0.3 g |
   | `departure.rs` | D1 to D3: Standard and Full are protected; Reduced and Manual depart and recover | ADR-33 section 3 |
   | `fuzz.rs` | AC-11.6: 10 minutes of random input per assist and seed, and 2 minutes from the runway | no NaN, no fault |
   | `mouse_aim.rs` | AC-58.4 (scaled): 500 runs | at least 95 %, ADR-33 amendment 1 |

3. Run the circuit (below) and `cargo xtask conformance`; a gain change moves no conformance point
   because the physics is in the aero tables.
4. A change that alters the flight changes the hashes: bless the determinism goldens again, in a
   dedicated commit with a stated reason (ADR-23).

## The virtual tester and the circuit gate

`fcs::tester::Tester` is a deterministic state machine that flies the whole sortie through the same
path the game uses: it writes `PilotControls` and button presses (never the surfaces), and
`sim-core::circuit` turns them into input frames for `InputScript::Circuit`. Its phases:

```text
Hold ─ Roll ─ Rotate ─ Climb ─ Crosswind ─ Downwind ─ Base ─ Final ─ Flare ─ Rollout ─ Stopped
```

It reads the exact state and thinks in flight-path rate and bank angle, and `sticks` converts that
to the stick each assist reads (the load factor for Standard, the path rate and bank for Full, the
pitch rate for Reduced, the inverse of the manual law for Manual). The numbers are in the module's
constants, marked `// DATA: ADR-33` or `// ALGO`.

```text
cargo xtask circuit                          # bake, build, fly all four assists, gate
fg circuit --content content-out/core.cpack [--assist all|full|standard|reduced|manual]
           [--json PATH] [--traces DIR] [--robustness] [--seed HEX]
```

`sim_core::circuit::report` judges nine criteria (C1 to C9: lift-off distance and speed, no stall
warning, gear up in time, the downwind leg, the centreline at 50 m, the touchdown, the rollout, no
crash or G-LOC or fault, and the time) and writes `circuit_report/1`. The gate load is 10 260 kg.
`--robustness` also flies the other loads of ADR-33 amendment 2 and reports them as informational.
The CI `conformance` job runs `cargo xtask conformance && cargo xtask circuit` and uploads both
reports. The same script flies in the browser with `?script=circuit` (`tests/e2e/circuit.spec.ts`),
and its hash is the third determinism suite.

## Checks that guard it

| Check | Catches |
|---|---|
| `cargo test -p fcs --release` | The suites above |
| `cargo xtask circuit` | A law that cannot take off, fly the pattern and land |
| `cargo xtask determinism` | `FcsState` left out of the hash: the circuit golden diverges |
| `crates/sim-wasm/tests/fcs_no_alloc.rs`, `no_alloc.rs` | An allocation in a step, a switch, the ground blends, mouse-aim, recover, G-LOC, or the tester's sortie |
| `cargo xtask check-layers`, `check-content-ids` | `fcs` depending on `sim-core`; a content id spelt in `fcs` code |
