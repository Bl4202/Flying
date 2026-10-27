# ADR-35: Ground model, touchdown criteria and the test airfield

- **Status:** Accepted
- **Owner:** Bl4202
- **Date:** 2026-10-22
- **Affects:** GDD 10.4, 10.7, 11.10, 11.13, 18.8.1, Appendix A.2, C.7 to C.9; `crates/flight` (`gear`, `surface`, `env`, `aircraft`, `actuators`), `crates/sim-core` (`seams/geo`, `services`, `spawn`, `systems`), `content/core/aircraft/`, `tools/content-bake` (gear lints)

## Context

Through v0.1a the aircraft had no ground. `CRASH_HEIGHT_M` ended the flight when the reference
point came within a few metres of the terrain, the gear actuator was `Actuator::fixed()` (so a
commanded `gear = 1` snapped back to 0 on the first sub-step) and the two ground-roll points of
the performance envelope were `deferred` (ADR-31, ADR-32). v0.1b needs take-off, landing, braking
and crash criteria (GDD 11.13), and a place to do them.

GDD 11.13 gives the shape of the model (struts, friction, anti-skid, steering, criteria) but not
its numerics. The risks are numerical: a stiff spring and a Coulomb friction inside RK4 at 120 Hz
chatter or blow up unless they are regularised, and a ground that is only a sphere with a terrain
height would not give a runway to land on.

## Decision

### The model: `flight::gear`

- **Per strut, per RK4 stage.** The ground force is a pure function of the rigid state, so it is
  evaluated in every `stage()` of the integrator and appears in `derivative()` and in the angle of
  attack rate closure. Load factor, trim and linearisation see it. Nothing about the ground is
  decided inside a stage: state changes are decided once per sub-step by `gear::events`.
- **Massless struts.** Each leg places its contact point (strut fully extended) in BF and samples
  the ground below it through `FlightEnv::surface`. Penetration along the surface normal is the
  strut compression `x` (positive when compressed).
- **Normal force.** `N = max(0, k x (1 + (x / x_max)^2) + c(xdot) xdot)`: a progressive spring and
  asymmetric damping (`c_comp` while compressing, `c_ext` while extending). Past `x_max` a stiff
  stop of ten times the spring rate takes over; 5 cm beyond the stroke the strut collapses.
  `// DATA:` the spring, damping and stroke are aircraft data in `aircraft/1`; the stop ratio and
  the collapse margin are `// ALGO:` constants.
- **Friction (GDD 11.13.2) with tanh regularisation on every tangential term.** Rolling
  resistance, braking and sideways force are each `-mu N tanh(v / v_s)` with `v_s` of 0.2 m/s
  (rolling), 0.05 m/s (braking) and 0.5 m/s (sideways, as the GDD). Rolling and braking **add**
  along the wheel, a **friction circle** of `mu_peak N` limits the tangential vector, anti-skid
  scales the brake by 0.9 and, without anti-skid, a brake above 0.8 above 5 m/s locks the wheel at
  a factor 0.6. Steering rotates the wheel axes; positive steering is the nose wheel turned right.
  `tanh` is `simmath::dmath::tanh` (ADR-10). A sharp `sign()` was rejected (see below).
- **Airframe contacts.** Tail, wingtips, nose tip and belly are checked only when the CG is within
  the gear's reach of the ground. They push back as a stiff spring (omega 15 rad/s, zeta 0.6) and
  drag with a skid friction of 0.5. A collapsed leg behaves the same way at the foot of its stroke.
- **Static pose.** `gear::static_pose` places the aircraft at rest on its gear: a linear-spring
  initial guess, then a damped two-degree-of-freedom Newton (height and pitch) on the real
  `forces()`, with a fixed iteration count and no allocation. `AircraftModel::static_state`
  returns the state and the pose (attitude, compressions, residual force). The linear guess is
  needed because Newton started level finds only the nose touching.
- **Gear actuator.** A real actuator over 0 to 1 at the data rate (`transit_s`, 6 s for the
  Kestrel), with no lag. The lever and the position are different things.

### Touchdown classification (GDD 11.13.3, read literally)

Sink rate is the velocity into the ground of the first contact of a touchdown:

| Sink rate at first gear contact | Result |
|---|---|
| up to 3.0 m/s | normal |
| 3.0 to 4.5 m/s | hard landing: counted, flag latched for 3 s |
| 4.5 to 7.0 m/s | that strut collapses |
| above 7.0 m/s | destroyed (`SinkRate`) |

- **Airframe contact** or a **collapsed strut** on the ground at ground speed above 30 m/s is
  destroyed (`AirframeImpact`, `GearCollapse`); below it the aircraft skids at mu 0.5 and
  survives. An airframe contact at a sink rate above 7 m/s is destroyed at any speed.
- **Only the leading contact** of a touchdown is judged for "hard": a leg that arrives later in
  the same landing (the nose coming down) is not a second hard landing. A touchdown re-arms after
  0.5 s in the air, so a bounce counts only if it was a real flight.
- A gear **in transit** (extension between 0.02 and 0.99) that meets the ground collapses.
- **Tunnelling.** A CG more than 5 m below the ground is a numerical failure, reported as
  `Tunnelling`, not as a crash. The old reference-point rule `CRASH_HEIGHT_M` is deleted.
- GDD 18.8.1 words this differently (normal-speed contact); the conflict is recorded here and
  GDD 11.13.3 governs until the damage chapter (18) is built. A GDD 0.2.2 clarification says so.

### Weight on wheels (GDD 11.13.4)

Any main leg with `N > 0` and a ground speed below 120 m/s. The debounce (0.1 s) belongs to the
FCS (ADR-34); the flight crate reports the raw state in `Telemetry::wow` (and `GroundState`).

### Surfaces: `flight::surface` and `FlightEnv::surface`

`FlightEnv::surface(p) -> SurfaceSample { height_msl_m, normal_bf, kind, wetness }` is a required
method; its height equals `ground_height_m` (a contract test). The GDD 10.4 friction table
(`// DATA: GDD 10.4 design targets; moves to world content at M2`) lives in `surface.rs`:

| Surface | Rolling | Braking, dry | Braking, wet |
|---|---|---|---|
| Paved | 0.02 | 0.70 | 0.40 |
| Grass | 0.05 | 0.40 | 0.30 |
| Dirt | 0.04 | 0.50 | 0.35 |
| Snow | 0.04 | 0.20 | 0.20 |
| Water | 0.10 | 0.20 | 0.20 |

The sideways peak equals the braking value (`// ALGO:`), so a wet runway corners worse too.
`SphereSea::default()` is paved and dry at 0 m, so a wheel test runs on asphalt;
`SphereSea::open()` has no ground (the airborne conformance points fly there so they measure the
aircraft and not the ground, ADR-31). `FlatSea` is water. In `sim-core`, `ServicesEnv` maps the
world's `SurfaceClass` to a kind: Paved to Paved, Land to Grass, Ice to Snow, Water to Water,
Urban to Paved. Wetness comes from the weather seam's `runway_wetness()` (0, dry, by default).

### The test airfield (`sim-core`, `seams/geo.rs`)

- `ProceduralGeo` gains a **basin** and a **runway pad**. The runway is 3 000 m by 45 m, heading
  000, centre at BF (0, -4 500) m, threshold at (0, -6 000), elevation 40 m MSL.
- Within 4 km of the runway rectangle the terrain is a 40 m plain with +-3 m of gentle noise,
  blending to the existing ridged noise over the next 4 km (smoothstep). On top of that the pad is
  flattened per GDD 10.7 (`w = smoothstep(1 - d / 300 m)`).
- `SurfaceClass` gains `Paved`: inside the runway polygon the surface is Paved, elsewhere Land.
- `TERRAIN_DATA_VERSION` becomes 2. The noise seed stays derived from version 1, so the landscape
  outside the basin does not change.
- The renderer reads the same heights through `fg_terrain_fill` and draws the markings from
  generated `AIRFIELD` constants, so the simulation and the picture have one source.

### The Kestrel's gear (starting data, measured)

| Leg | Contact (body) | Stroke | k | c comp / ext | Steer | Brake |
|---|---|---|---|---|---|---|
| nose | (4.58, 0, 2.29) m | 0.30 m | 130 kN/m | 9 / 18 kN s/m | 60 deg | yes |
| main L, R | (-0.62, ±1.45, 2.05) m | 0.30 m | 325 kN/m | 30 / 60 kN s/m | none | yes |

Measured by the tests at 60 % fuel: static pitch 3.19 deg; compressions 0.071 m (nose) and
0.120 m (mains), about 40 % of the stroke; the nose carries about a tenth of the weight. The
tail-scrape pitch is **16.45 deg**, not the 19.4 deg of the design review (the contact point was
placed after it); a touchdown at 1.15 V_s,LDG is at about 15 to 16 deg, so the margin is thin and
the baker lint below keeps it at least 1 deg. Strut bounce is below 15 rad/s (GDD 11.10).

### Baker lints (`tools/content-bake`)

The nose share stays within 5 to 20 % over the CG range; static compression is 20 to 60 % of the
stroke at MLW and MTOW; bounce omega below 15 rad/s with zeta in [0.15, 1.0]; tail clearance at
least the touchdown pitch plus 1 deg; the CG stays ahead of the main contacts (tip-back); every
contact lies inside the bounding box; at least one braked leg; none or exactly one steerable leg.

## Alternatives considered

- **A sharp `sign(v)` Coulomb friction.** Simple, but it chatters at rest (the sign flips every
  step) and makes a parked aircraft creep. The tanh keeps the force continuous and a parked
  aircraft still, with `lambda dt` near 1 at the integration step.
- **Struts with their own mass and state.** More physical, but it needs a stiff sub-integration
  and extra hashed state. A massless strut with a force from the compression is exact for the
  frequencies that matter (below 15 rad/s) and keeps the state in the rigid body.
- **Ground contact decided inside each RK4 stage.** Decisions (collapse, destroy) inside a stage
  are made on provisional states and would depend on the integrator. They are made once per
  sub-step on the accepted state.
- **Judging every leg's touchdown.** The nose wheel arriving 0.2 s after the mains would be a
  second, "hard" landing at a sink rate that is only the pitch-over. Only the leading contact
  counts; a strut driven hard enough still collapses whenever it arrives.
- **A runway as a separate collision mesh.** Rejected: the terrain is already a pure function of
  position (GDD 7.7), and a flattened pad in that function keeps the sim, the renderer and the
  wheel contact on one source.

## Consequences

- `Commands` carries `WheelCommands`, `PilotControls` carries `gear`, `brake_l` and `brake_r`,
  `AircraftState` carries `GroundState`, and `StepOutcome` carries `GroundEvents` and `nz_g`.
  Every caller had to be updated.
- The airborne conformance points do not move (verified: `cargo xtask conformance`, same digits as
  v0.1a). The content hash changes with the new data, so the determinism goldens that include
  content change once, in the dedicated bless of WP9.
- The sink-rate and speed thresholds are the GDD's literal values. If play shows them too harsh
  they are constants in one place (`gear.rs`), but changing one needs a GDD clarification first.
- The ground model costs nothing in the air: `forces()` returns zero when the CG is above the
  gear's reach, and `step_tick` still allocates nothing (`flight_no_alloc.rs` covers the pose
  solver, taxiing, all four touchdown classes, a belly skid and the tunnelling guard).
- The ground itself costs what it should not hide: on the wheels an aircraft costs about 33 µs per
  tick natively against about 6.3 µs in the air (`fg bench --aircraft 1 --ground`), because each of
  three wheels takes a terrain sample, a normal (four more height lookups) and a class in every one
  of eight RK4 stages. A cheaper normal on the flat pad would change bits and so the goldens; it is
  left for a milestone that re-blesses anyway. The height of the lowest wheel that the telemetry
  reports reads `ground_height_m`, which is what `surface().height_msl_m` is by contract.
- Wetness, snow and dirt are modelled but the procedural world is paved and grass only until M2.
- To be revisited at M2 (surface table into world content) and in the damage chapter (18).
