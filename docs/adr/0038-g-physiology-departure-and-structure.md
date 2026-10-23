# ADR-38: G-physiology, departure diagnostics, structural limits and the onboard control model

- **Status:** Proposed
- **Owner:** Bl4202
- **Date:** 2026-10-22
- **Affects:** GDD 11.14, 11.15, 57, 59, 11.21 (AC-11.9); `crates/flight` (`physio`, `structure`, `departure`, `aero`, `linearise`), `content/core/aero/` (lateral tables)

## Context

v0.1b adds the pilot's body and the airframe's limits to the flight model: sustained load greys the
screen out, then blacks it out, then knocks the pilot out; departure behaviour has to exist for
Reduced and Manual assist to be worth having; and the flight control system (ADR-34) needs the
aircraft's own control power to invert. None of this changes the airborne performance points.

## Decision

### G-physiology (`flight::physio`)

- The equations are GDD 11.14 unchanged (`G_f` lag 2 s, `G_tol` 4.5 + 1.5 - 0.4 F, `S` growth 0.25 /s
  above tolerance and 4 s recovery below, fatigue +0.01 /s above 5 g and -0.005 /s otherwise,
  red-out `(-1 - n_z) / 3 s` below -1 g). The constants are `PhysioParams::GDD` and marked
  `// DATA: GDD 11.14 design target`.
- It is stepped **once per tick** with `n_z` held. `G_f` uses the exact first-order update; `S`
  integrates the **mean** `G_f` of the tick, which is exact for a held `n_z`. The result does not
  depend on the step beyond the one tick at which a threshold is crossed.
- `n_z` is the accelerometer value **at the CG**. The pilot station's rotational terms (a pilot 5 m
  ahead of the CG feels `q-dot` times the arm) are a recorded simplification.
- **AC-11.9, checked by hand and by test:** a 9 g pull from 1 g reaches `S` = 0.7 at 9.50 s and
  1.0 at 11.93 s with fatigue frozen at 0 (bands 9.6 +- 0.5 and 12.0 +- 0.5), and at 9.42 s and
  11.80 s with fatigue building; a 3 s pull peaks at `S` = 0.040 (< 0.05) and causes no blackout.
- **The dial** (GDD 57, 59), `GEffects`:

| Dial | Tolerance | Visual | Control |
|---|---|---|---|
| Full (default) | `G_tol` | grey-out, black, red | `S` >= 0.7: controls delayed 300 ms (18 ticks); `S` >= 1.0: G-LOC, controls zeroed 6 s, then a linear 4 s ramp back |
| Reduced | `G_tol` + 1.5 g | grey-out, black, red | none |
| Off | `G_tol` | none | none |

  The state advances in every mode, so turning the dial on mid-flight does not start from rest.
  Strength of the visual effect (0 to 100 %) is a local setting applied by the renderer, not a
  simulation input.
- G-LOC cannot re-trigger while it is running, and does re-trigger if the pilot is still above
  tolerance when it ends (the aircraft is still pulling). Leaving Full clears it. In Full assist
  the FCS holds wings level and `gamma >= 0` through G-LOC (ADR-34); that is the FCS's job, not
  the physiology's.
- Effects are `PhysioEffects`: `blackout`, `gloc`, `gloc_started`, `control_scale`,
  `delay_ticks` (0 to `MAX_DELAY_TICKS` = 18) and the visual strengths `grey`, `black`, `red`. The
  caller owns the delay ring (`sim-core`, hashed state).

### Structural limits (`flight::structure`)

- Per configuration: `n_max` (9 g clean, 4 g with the gear or the flaps out, from
  `n_max_by_config`), `n_min` (-3 g), `vmo`/`mmo`, the gear-extended and flap-extended speeds.
  `Limits::n_max_for(gear, flaps)` is the one place the configuration limit is chosen; the FCS g
  limiter reads it too.
- `|n| > 1.1 n_limit` raises OVERSTRESS and accumulates `overstress_s` (never healed). Beyond
  `vmo` or `mmo` raises OVERSPEED and accumulates `overspeed_s`. The gear out above its limit raises
  GEAR_OVERSPEED, the flaps out above theirs FLAP_OVERSPEED. These are the snapshot's warning flags.
- **Nothing breaks in v0.1b.** GDD 11.15 puts break-up beyond 1.5 `n_max` or 1.2 `V_ne` in the
  damage model (GDD 18.7, v0.3c). The clocks exist so that chapter has the history.

### Stall and departure

- The stall warning stays at `alpha > alpha_peak - 3 deg` (GDD 11.15); the stall itself emerges from
  `C_L,max` and the post-stall blend (ADR-30).
- The twelve lateral and directional derivatives are **tables against |alpha|** (done in the data
  format work of this milestone). Up to 15 deg they equal the C.5 scalars, so no airborne point
  moves; above it they are shaped for the departure behaviour. The shapes are in
  `content/core/aero/README.md`.
- `flight::departure` computes the **dynamic directional stability parameter**
  `C_n,beta,dyn = C_n,beta cos a - (I_z / I_x) C_l,beta sin a` and the **lateral control departure
  parameter** `LCDP = C_n,beta - C_l,beta C_n,da / C_l,da` from those tables, for any mass state.
  They decide nothing at run time. Measured on the Kestrel (`I_z / I_x` about 6.6 at 1 600 kg of
  fuel):

| alpha | C_n,beta,dyn | LCDP |
|---|---|---|
| 0 | +0.140 | +0.134 |
| 10 | +0.253 | +0.134 |
| 20 | +0.405 | +0.073 |
| 26 (the limiter) | +0.500 | -0.011 |
| 30 | +0.278 | -0.038 |
| **34.05** | **0 (turns negative)** | -0.036 |
| 40 | -0.146 | -0.068 |
| 60 | -0.093 | -0.041 |
| 90 | 0 | 0 |

  So the airframe diverges directionally from 34 deg, 8 deg above the 26 deg limiter Standard and
  Full hold, and the aileron loses and then reverses its authority from about 25 deg (LCDP < 0),
  which is why the FCS reduces the roll command with `alpha` (`p_max_alpha`, ADR-34). Reduced and
  Manual have no such limiter and can depart; RECOVER (ADR-34) is the way out.
- `linearise` prints both at each C.5 trim point (informational).

### The onboard control model (`TableAero::control_model`)

The FCS inverts the aircraft's own tables (ADR-34). `control_model(&AeroInput) -> ControlModel`
returns the **free moment** (the model's own `evaluate` with the elevator, aileron and rudder
centred) and the **moment per radian** of each surface, about the CG, including the
control-effectiveness fall-off `eta(|d|)` at the surface's current deflection (GDD 11.7), plus the
elevator's direct lift `dz_de` and `q S`. The full 3 x 3 matrix is in `moment_per_rad_nm`; the
scalars (`dm_de`, `dl_da`, `dl_dr`, `dn_da`, `dn_dr`) are its main entries.

- The moment coefficients are exactly linear in the effective deflection, and the force terms at
  the CG lever arm are linear in the lift slope and the induced-drag slope at zero deflection. A
  test compares the model with `evaluate` at **10 000 random states** (the whole envelope, the
  post-stall blend, ground effect, every configuration, a CG off the reference point) with small
  deflections: the worst relative error is 2e-11 (the limit is 1e-9).
- The elevator's effect on `alpha_dot` (through the lift) is not included; the input's `alpha_dot`
  is held. It is second order and the FCS loop closes over it.

## Alternatives considered

- **Euler integration of `S` with the instantaneous `G_f`.** It was off by up to 0.06 s at the
  blackout time at 60 Hz and changed with the step. The mean-`G_f` form costs one `exp` and is exact.
- **Treating Reduced as Off for control but with Full tolerance.** The GDD's "never lose control"
  accessibility toggle is visual only, so Reduced keeps the visuals and adds tolerance (+1.5 g) so a
  player who chose it is not blacked out at the same load.
- **A numeric linearisation of `evaluate` for the control model.** Central differences are exact
  for these tables (the coefficients are linear or quadratic in the deflection), but seven
  evaluations per tick for what is a few multiplications; the analytic form is faster and the test
  holds it to `evaluate`.
- **Break-up at 1.5 `n_max` now.** It needs the damage model's rules (what breaks, what the player
  sees). Without it a break-up would be a bare crash.

## Consequences

- The physiology, structure and departure pieces are pure functions of `Copy` state. The state
  joins the `ownship` hash when `sim-core` adopts it (`PhysioState`, `StructureState`, the delay
  ring), and a re-bless of the determinism goldens covers it (WP9).
- `Limits::n_max_for` and `AircraftModel::structure_input` give `sim-core` and the FCS one reading
  of the configuration.
- The departure table is a *target*; ADR-34's departure suite (Standard and Full protected; Reduced
  and Manual depart and recover within 15 s) is what judges it, and the data may move in WP5. Every
  change is logged in `content/core/aero/README.md`.
- To be revisited in the damage chapter (break-up) and if pilot-station accelerations are wanted.
