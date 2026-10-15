# ADR-29: Flight crates, the flight environment and the integration scheme

- **Status:** Accepted
- **Owner:** Bl4202
- **Date:** 2026-10-15
- **Affects:** GDD 0.6.1, 11.2, 11.6, 11.10, 61.4.2, 61.5; ADR-10, ADR-23, ADR-27; `crates/simmath`, `crates/flight`, `crates/flight-test`, `crates/sim-core`, `crates/cli`

## Context

v0.0b's player was a kinematic observer inside `sim-core`: the stick commanded body rates and
the throttle set the speed. v0.1a replaces it with the GDD 11 flight model, a 6-DOF rigid body
flown from table data. Three users need that model:

- the live tick (`sim-core`, compiled to WASM);
- the Performance Conformance Test (GDD 11.19, ADR-31), which flies it headlessly for minutes of
  simulated time on an analytic world;
- trim, linearisation and future tools.

The tick schedule, the ABI and the seams policy (ADR-27) already exist. The flight model must
fit them without the conformance test depending on any of them, and it must stay bit-for-bit
deterministic across native and WASM (ADR-10).

## Decision

**Crates.**

| Crate | Holds | May depend on |
|---|---|---|
| `simmath` | `DVec3`, `DQuat`, `DMat3`, `dmath` (moved unchanged from `sim-core`, see ADR-27's amendment) | `libm` |
| `flight` | atmosphere, air data, tables, aero, engine, mass, actuators, controls, rigid body, aircraft tick, trim, linearisation, energy | `simmath`, `content` |
| `flight-test` | the conformance pilot, procedures and report | `flight`, `simmath`, `content`, `serde_json` |
| `sim-core` | the tick, ownship, spawn, crash and respawn; adapts its services to the flight model | `content`, `simmath`, `flight` |
| `cli` | `fg conformance`, `fg hash`, `fg bench` | the above |

`cargo xtask check-layers` enforces the table. `flight` knows nothing about the tick schedule,
the ABI or the browser.

**The flight environment.** `flight::env::FlightEnv` is everything the model asks of the world:
`height_msl_m(p)`, `up_bf(p)`, `ground_height_m(p)`, and defaults for `earth_radius_m`,
`gravity_bf(p)` (`g0·(R/(R+h))²` along `-up`), `wind_bf(p, t)` (still air) and `day()` (ISA).
`sim-core`'s `ServicesEnv` implements it over the `GeoService` and `WeatherProvider` seams, which
gained `height_msl_m`, `gravity_bf`, `ground_height_msl_m` and `day` (contract-checked against
`bf_to_geo`). The conformance test flies in `SphereSea`, an analytic sphere of radius 6 371 km
(`open()` has no ground, so no ground effect). Altitude everywhere is h_MSL from the sphere and
vertical speed is `v·up(p)`, never BF `z`: on the 11 km run the aircraft covers about 120 km and
the ground falls about 1.1 km below the tangent plane.

**Integration.** Classical RK4 at `dt = tick/2`, two sub-steps per 60 Hz tick (120 Hz, GDD
0.6.1 and 11.10). The integrated state is 13 `f64`s: `pos_bf`, `vel_bf`, `q_body_to_bf`,
`omega_body`, for the centre of gravity.

- *Held for the tick* (zero-order hold): the pilot's commands and the mass properties, recomputed
  once per tick from the tank contents. There is no cache: everything that affects the next tick
  is in the hashed state.
- *Held for a sub-step:* actuator positions, engine spool and afterburner stage. They advance
  once per sub-step, before the RK4 step, with exact first-order updates.
- *Re-evaluated in every RK4 stage:* height, `up`, gravity, wind, the atmosphere, air data, the
  aerodynamic force and moment, and the thrust magnitude (which depends on density, Mach and
  angle of attack).
- *After each sub-step:* `q` is renormalised and fuel is burned in feed order.

**Angle-of-attack rate.** `α̇` is computed exactly inside each stage: forces first (lift has no
`α̇` term), then `v̇_body = F/m + g_body − ω × v_body`, then `α̇ = (u·ẇ − w·u̇)/(u² + w²)`, and
finally the moment, which has `C_mα̇`. `TableAero::evaluate_with` does this in one pass. There is
no lagged state.

**Faults.** Each tick starts from a copy of the state. A non-finite state, `|ω| > 50 rad/s` or
`V > 3 000 m/s` restores the copy, neutralises the surface commands and retries the tick once. A
second fault restores again and raises `numerical_fault`. The step reports its faults;
`sim-core` counts them in `fdm_faults` and respawns an aircraft that has `numerical_fault`.

**The AeroModel seam.** It moves to `flight::aero` and is re-exported from
`sim_core::seams::aero`. As ADR-27 allows for a provisional seam's first real user, its
signature changed: the input is the air data, `α̇`, the actual surface positions in **radians**
(elevator, aileron, rudder, flaps, airbrake, gear), the height above ground for ground effect and
the CG position, and the output moment is about the CG. `TableAero` is the real implementation.
The aircraft tick calls it concretely, never through `dyn` (GDD 61.4.2); `NoAero` and
`LinearAero` remain as contract-passing stand-ins, with no aircraft-specific constants in code.

**The throttle detent.** The lever is `f32`. Military power is the detent at `0.8f32`, and a
stage beyond it is selected only when the lever is more than `1e-4` past the edge, because
`0.8f32 as f64` is slightly above `0.8` and would otherwise light afterburner stage 1 at
military. The conformance harness sets engine modes as an enum, not as lever positions.

**The ownship.** `sim-core`'s ownship is an enum: `Aircraft(Box<AircraftSlot>)`, built from the
lowest-handle aircraft entry when content loads (no ids in code, `AC-12.4`) and allocated once at
`start()`, or `Spectator(Observer)`, the v0.0b observer, when no aircraft content is loaded.
Respawning resets the slot in place, so the tick never allocates. The snapshot reports the
**reference point** (`p_cg − R(q)·r_cg`), not the CG, so the camera does not move as fuel burns.

## Alternatives considered

- **The flight model inside `sim-core`.** One crate fewer, but the conformance test would then
  build the whole tick, ABI and services to fly one aircraft on an analytic world, and the
  layering could not stop flight code from reaching into the schedule.
- **Semi-implicit Euler, or RK4 at 60 Hz.** GDD 11.10 fixes RK4 at 120 Hz for T0. The short-period
  mode at high dynamic pressure is fast enough that 60 Hz leaves little margin.
- **A lagged `α̇` (last stage's value).** Simpler, but it is hidden state outside the hash and
  adds a phase error to the pitch damping.
- **Caching mass properties between ticks.** Saves a few hundred flops, but the cache would be
  state that the hash does not cover.
- **`dyn AeroModel` in the tick.** One indirection per RK4 stage on the hottest path; GDD 61.4.2
  keeps hot paths concrete. The trait remains the seam for tools and future models.

## Consequences

- The flight model is tested on its own (`cargo test -p flight`) and flown headlessly by
  `fg conformance` in about a second.
- One aircraft costs four derivative evaluations per sub-step, eight per tick. The T0 budget is
  measured by `fg bench --aircraft` (v0.1a exit item 12).
- `sim-core` gained a dependency, `flight`, and the seam table's Aero row is now wired.
- The change of player made the determinism golden stale. It was re-blessed once, with the
  `tick_h`, `rng`, `bodies` and `input` columns proven unchanged (ADR-23), and `flight60.json`
  was added for `AC-11.7`.
