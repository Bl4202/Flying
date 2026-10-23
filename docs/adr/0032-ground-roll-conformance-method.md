# ADR-32: Ground-roll conformance method and the landing-roll finding

- **Status:** Accepted
- **Owner:** Bl4202
- **Date:** 2026-10-22
- **Affects:** GDD 11.13, 11.19, 11.21 (AC-3.2, AC-11.3), Appendix C.7 to C.9, AC-66.4; `crates/flight-test`, `crates/flight`, `crates/cli`, `crates/xtask`, `content/core/aircraft/`, `content/core/aero/`, `content/core/engines/`

## Context

ADR-31 deferred the two ground-roll points of the Kestrel envelope (`to_roll` 195 m, `ldg_roll`
322 m) until the gear model exists, and required the deferred set to be exactly
`{to_roll, ldg_roll}` so a point could not be dropped silently. v0.1b builds the gear model, so
both points are flown and AC-3.2 closes.

The targets come from the point-mass reference calculator (C.8): "the take-off and landing
integrations use the same polar with ground C_L of 0.55 (take-off) / 0.45 (landing), rolling
resistance mu_r = 0.02, thrust T_SL (rho/rho0)^0.75, and the configuration drag of C.4,
integrated with a 0.01 s Euler step to V_LOF (take-off) or to 2 m/s (landing)."

Like ADR-31, this ADR fixes the method before any gear code exists or is tuned (AC-66.4).

### The landing-roll finding

C.8's text was re-run for this ADR with an independent script (0.01 s Euler, the C.4 numbers,
V_s from C_Lmax 1.90, rolling resistance mu_r = 0.02 as C.8 states, brakes on the load
W - L). The two targets do not behave alike:

| Point | C.8 as written | Target | Difference |
|---|---|---|---|
| `to_roll`, 10 500 kg, AB, take-off flaps (gear + flap drag 0.035, ground C_L 0.55) | 195.4 m | 195 m | +0.2 % |
| `ldg_roll`, 7 500 kg | 293 m | 281 m | +4.2 % |
| `ldg_roll`, **8 500 kg**, mu_b 0.55 added to mu_r 0.02, airbrake, gear + landing flap drag (0.065), ground C_L 0.45, touchdown at 1.15 V_s, zero thrust | **332 m** | 322 m | **+3.1 %** |
| `ldg_roll`, 9 500 kg | 371 m | 362 m | +2.5 % |

So the take-off target reproduces and the landing targets sit 2.5 % to 4.2 % **below** what the
calculator's own text produces. Nothing was tried that gives 322 m exactly. The variants:

- Without mu_r while braking, the 8 500 kg result is 343 m (+6.5 %), 303 m and 383 m at the other
  masses (+7.7 %, +5.9 %).
- Without the lift relief on the wheel load, it is 283 m (-12 %).
- The wet-runway figure (mu_b 0.40) is 438 m against C.7's 420 m (+4.3 %); the wet/dry ratio is
  1.32 against 1.30.

The 6-DOF model differs from the calculator in a way that matters here: **the F129 idles at 6 %
of 79 kN (4.7 kN), which C.8 does not model.** With the calculator's friction and that thrust the
8 500 kg roll is about 373 m (+16 %). The model must therefore brake harder than C.8 assumes to
meet the target. That is a data question (the braked nose wheel, the GDD 10.4 dry-asphalt
mu_b 0.70 with the 11.13.2 anti-skid factor 0.9, the idle fraction), not a reason to move the target.

The owner decided (2026-10-22) to keep 322 m and retune data, following C.9: "differences larger
than the tolerance are resolved by retuning the data, not by changing the targets, unless the
target itself is shown to be wrong." The target is not shown to be wrong: it is within 3.1 % of
C.8's literal result. This ADR records the arithmetic so nobody has to rediscover it.

*Correction recorded 2026-10-22:* the owner's decision was made on a first figure of 343 m
(+6.4 %) that left out the rolling resistance C.8 states. With it, the gap is +3.1 %, inside the
5 % tolerance. The decision does not change (the target stays and the data is tuned), but the gap
is smaller than first reported.

## Decision

### Scope and gate

- The gate is **twelve points**: the ten airborne points of ADR-31, plus `to_roll` and
  `ldg_roll`. The deferred set must be **empty**. `requires: "gear"` is met when the model has at
  least one braked main leg; a model without gear still reports the points `deferred`.
- Tolerances are those of GDD 11.19 and C.9 (5 % for both ground rolls). They are not changed.
- Each ground-roll point must also pass its **cross-checks** (below). A disagreement outside the
  stated band fails the run even if the headline value is inside tolerance.
- Report a warning when a point uses more than 60 % of its tolerance. The tuning goal is at most
  60 %.

### Common rules

- All ADR-31 common rules hold: mass and CG frozen at the point's `mass_kg` with fuel in feed
  order, ISA sea level, still air, continuous measured values, every run capped in sim time, a
  timeout is a failure (`timeout`), a run that does not converge is `no_convergence`.
- **World:** `SphereSea::default()`: the sphere of R = 6 371 km with the ground at h_MSL 0, paved
  and dry. Not `SphereSea::open()`, which has no surface. The runway is the whole ground.
- **Anti-skid is on.** Brake commands are 0 or 1 per braked leg.
- **The test pilot is an instrument, not the FCS.** It commands surfaces, nose-wheel steering and
  brakes directly. Lateral control is a PD loop from lateral offset and heading to steering and
  rudder; the stick is neutral (elevator at the ground trim).
- **Lift-off and flare are not part of conformance.** C.8 integrates the ground phase only, so the
  procedures do too. The circuit gate of ADR-33 covers rotation, flare and touchdown handling.
- `config_surfaces(LdgBrakes)` opens the airbrake (v0.1a omitted it). C.7: "brakes mu_b = 0.55,
  airbrake".

### Procedures and cross-checks

| Point | Procedure | Valid while | Cross-checks (both gate) |
|---|---|---|---|
| `to_roll` (10 500 kg, `to_flaps_ab`) | Place the aircraft on its gear at the **static pose** (the attitude and compression that carry its weight at rest). Engine initialised **steady at maximum afterburner**: C.8 applies full thrust from t = 0. At t = 0 release the brakes. Measure the along-runway displacement of the CG until ground speed reaches V_LOF = 1.2 V_s,TO, with V_s,TO = sqrt(2 m g0 / (rho0 S C_Lmax,TO)) and C_Lmax,TO = `cl_max(0)` plus the take-off flap increment. Stick neutral, **no rotation** | All wheels in contact; lateral offset below 2 m; pitch within 0.5 deg of the static pose | (a) The C.8 point-mass integral with the **model's own forces** at the static attitude (thrust T(V) with the Mach factor, C_L and C_D with ground effect, mu_r from the surface table): within 2 %. (b) Energy closure, 1/2 m V^2 against the integral of (thrust_x - drag - wheel friction) dx taken from the trace: within 1 % |
| `ldg_roll` (8 500 kg, `ldg_brakes`) | Static pose for the weight less the lift at V_TD. Ground speed starts at V_TD = 1.15 V_s,LDG, with V_s,LDG from C_Lmax,LDG = `cl_max(0)` plus the landing flap increment. Engine steady at idle. Airbrake open, landing flaps, gear down. Brakes 1.0 on every braked leg with anti-skid, from t = 0. Measure displacement until ground speed is below 2 m/s | As above | (a) The C.8 point-mass integral with the model's forces, idle thrust T(V), and mu_eff = mu_r + 0.9 mu_b(surface) on the load N = W - L: within 3 %. (b) Energy closure within 1 % |

Both cross-checks use the same time step as the dynamic run and are computed from the run's own
trace (speed, thrust, drag, normal load), so they separate a **method** fault (the gear model
disagrees with the point-mass physics it should reduce to) from a **data** fault (the target
needs different gear, brake or engine data).

### Informational runs (reported, not gated)

- A rotation run at 10 500 kg: full aft stick at V_R = 1.1 V_s,TO. Report the lift-off speed
  (target at most 1.3 V_s,TO) and the distance. C.7 puts the 15 m obstacle distance at about 440 m
  (an estimate).
- The wet-runway landing at 8 500 kg (C.7 gives 420 m).
- `vmax_11k` on `ProceduralGeo`, as in v0.1a.

### Expected deviations from C.8 (recorded before tuning)

| Point | Expected deviation |
|---|---|
| `to_roll` | The 6-DOF model rolls at the static attitude (about 3 deg), so ground C_L is about 0.49 against C.8's 0.55 (+0.3 %). The Mach factor f(M) is at most 0.2 over the roll (+0.4 %). Ground effect is negligible (-0.2 %). Total about **+0.7 %** (196 to 197 m) |
| `ldg_roll` | Against C.8's literal 332 m: the braked effective friction is 0.65 (mu_b 0.70 x 0.9 anti-skid + mu_r 0.02) where C.8 uses 0.57, about -12 %; the idle thrust of 4.7 kN, which C.8 does not model, adds about +13 %; attitude and ground effect add about +1 %. Total about **+2.8 % against the 322 m target** (331 m), which is about 0 % against C.8's literal 332 m. With an idle fraction of 0.05 the estimate is 325 m, with 0.04 it is 319 m |
| Nose wheel | If only the main wheels brake (71 % of the load at the empty CG), the roll is about 463 m (+44 %). **A braked nose wheel is required data** (GDD A.2 allows `brake` on any gear leg) |

The ground-roll estimates come from point-mass scripts, not from the 6-DOF model, and are the
basis of the starting data. The dynamic run decides.

### Allowed tuning knobs (Kestrel data only; log each change)

- Gear geometry (static attitude and the load share of the nose leg), within the baker's gear
  lints (ADR-35).
- The `brake` flags on gear legs.
- The engine `idle_fraction`, between 0.04 and 0.06.
- `flap_to/ld` and airbrake and gear increments of drag or lift in `aero/1` `config`. None of these
  touch an airborne point.

Every change goes in the tuning log of `content/core/aero/README.md` (and
`content/core/engines/README.md` for the engine): what changed, which point moved, by how much.

### Not allowed without a new ADR

- The procedure constants: V_LOF = 1.2 V_s, V_TD = 1.15 V_s, the 2 m/s end of the landing run,
  anti-skid at 0.9, the static-pose start, the ground C_L (it follows from the pose).
- The surface friction table (GDD 10.4) and the tanh regularisation of friction.
- The targets and tolerances in C.9.
- **If either point cannot pass with data inside these knobs, stop and return to the owner** with
  the evidence (the owner's decision of 2026-10-22).

### Gate and tooling

- `EXPECT_DEFERRED` in `crates/xtask/src/conformance.rs` becomes empty; `fg conformance`
  defaults to an empty expected set (the flag stays, for models without gear).
- The deferred point list, the `RollM` branch of the runner and the tests that assert exactly
  ten passes and the `{to_roll, ldg_roll}` set are updated in the work package that makes the
  rolls pass: `tests/conformance.rs` expects 12 passes and an empty deferred set,
  `tools/content-bake/tests/bake.rs` follows.
- The report schema `conformance_report/1` is unchanged; the new points use the same fields, with
  `method` naming the cross-check.

## Alternatives considered

- **Change `ldg_roll` to C.8's literal 332 m (or to about 373 m, the untuned model with idle
  thrust).** Rejected by the owner (2026-10-22): C.9 says retune data, and the model has a free,
  plausible degree of freedom (the nose brake and the braking friction) that closes the gap. If the data route fails, this alternative returns as a new ADR with the
  evidence, not as a silent edit.
- **Gate `ldg_roll` later.** Would leave AC-3.2 open at the end of M1 for a point that has a
  documented route to pass.
- **Run the full take-off including rotation and lift-off.** The 6-DOF model cannot rotate below
  about 64 m/s with the mains 0.62 m behind the CG, so a 10 500 kg run that must lift off needs
  about 245 m (+25 %). C.8 integrates the ground phase to V_LOF; matching its definition is what
  the target means.
- **Compare against C.8 literally at every point.** The literal calculator has no static attitude,
  no ground effect and no idle thrust; the cross-checks use the model's own forces so that the
  comparison tests the gear model, not the differences in inputs.
- **Hold throttle detents and brakes with real input frames in conformance.** The pilot is an
  instrument. The real path is exercised by the circuit gate (ADR-33).

## Consequences

- AC-3.2 closes at v0.1b: twelve points, empty deferred set, cross-checks in band.
- The gear model, the friction table and the Kestrel gear data are validated against two numbers
  per point, not one.
- The C.7 and C.8 landing finding is public. The GDD gets a clarification in 0.2.2 saying the
  target is kept and what assumptions the model meets it with.
- Any later change to the gear, brake or engine data requires re-running `conformance` and a
  reasoned golden re-bless.
- If a different aircraft is added later, its ground-roll points use the same procedures; only its
  data differs.
