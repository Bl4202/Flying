# ADR-46: Weapons: the gun, the director gunsight, the IR seeker, the PN missile and the fuze

- **Status:** Proposed
- **Owner:** Bl4202
- **Date:** 2026-11-09
- **Affects:** ADR-10, ADR-29, ADR-30, ADR-45; GDD 15.4, 15.5, 15.6, 15.7, 15.12, 18.2, 18.8.2, C.1,
  C.10, C.11; `crates/weapons`, `crates/content`, `tools/content-bake`, `content/core/weapons`,
  `content/core/aircraft`

## Context

v0.1d adds one gun and one infrared missile with simple proportional navigation (the roadmap's
v0.1d section). ADR-45 fixed their gates before any weapon code existed: GN1 to GN6 for the gun and
MS1 to MS5 for the missile. The GDD's weapons chapter (15) describes a game-fidelity model for every
weapon class; v0.1d needs two of them, built so the later classes (radar missiles, countermeasures,
the DLZ baker, component damage) fit beside them without rework.

The simulation's rules apply (ADR-10): `f64` state, transcendental functions through `dmath`, integer
random streams owned by `sim-core`, no allocation on the tick path, bit-identical results on every
target.

## Decision

### The crate

`crates/weapons` (a name GDD 61.2 reserved) holds the weapon physics as pure functions over plain
structs. It depends on `content`, `flight` and `simmath` only (`check-layers`), never on the world:
`sim-core` keeps the round pool, the missile slots, the timers and the random streams, and hands the
crate positions, velocities, attitudes, a `FlightEnv` and the atmosphere table. Random numbers come
in as arguments (standard normals drawn by `sim-core`), so every function is a pure function of its
inputs. Nothing in it allocates once the parameters are built (`sim-wasm/tests/weapons_no_alloc.rs`:
the gunsight, rounds, hit tests, seeker, guidance, missile, fuze and predictor over 600 ticks, zero
allocations; the counting allocator lives in `sim-wasm` because only that crate may use `unsafe`).

| Module | What |
|---|---|
| `capsule` | Capsules in body axes; segment-segment closest points; the swept segment-against-capsules test (the first capsule entered and where); the relative segment in the target's axes; capsule-set clearance |
| `ballistics` | `GunBallistics::from_def`; round acceleration; two RK2 sub-steps per tick; expiry; the gun axis, dispersion, spawning |
| `sight` | The director solution |
| `ir` | `IrSignature::from_def`; engine term, aspect pattern, intensity, aspect angle |
| `seeker` | `SeekerParams::from_def`; the signal; the SEARCH, TONE, LOCKED state machine |
| `missile` | `MissileParams::from_def`; launch limits; PN; guidance; RK4 integration; expiry |
| `fuze` | Arming, the swept proximity check, warhead energy |
| `predict` | The fly-out predictor behind `IN RNG` |

### Content: `weapon/1` (cpack v4)

A weapon entry is cpack kind 5 with a class and exactly the block of its class (`gun` or `srm`); AI
skills are kind 6 (ADR-47). The aircraft record gains, encoded after every earlier field: the
internal gun (weapon handle, rounds, muzzle position, harmonisation), up to 8 launch rails (station,
position, store handle), the IR signature, the visual size and the damage data (hit points, joules
per hit point, up to 16 capsules). The reader checks that the gun and each rail store are weapon
entries. The fields are those of the plan's section 9.1; the baker lints their ranges and the
cross-checks listed in `content/core/weapons/README.md`. The weapons are in the simulation pack, so
the pack's SHA-256 (in the state hash's `content` column) moves; ADR-45 D2 re-blesses the goldens
once, after the last data change.

### The gun (GDD 15.4.2)

- A round is a point mass: `a = −k |v − w| (v − w) + g(p)`, `k = ρ(h) C_d(M) A / (2 m)`,
  `A = π (calibre / 2)²`, `M = |v − w| / a(h)`. Two midpoint (RK2) sub-steps of `dt / 2` per 60 Hz
  tick. It expires after `max_age_s` (3 s), below `min_speed_mps` (300) or below the ground.
- It leaves the muzzle at `ref + att(muzzle)` with velocity `v_ref + att(ω × muzzle) + v_muzzle ·
  att(dir)`. `dir` is the gun axis `(cos h, 0, −sin h)` turned by a **Gaussian** offset
  `normalize(axis + σ (n1 e1 + n2 e2))`, σ = `dispersion_mrad` (GDD 15.4.1), `e1, e2` perpendicular
  to the axis.
- The GS-27's `dispersion_mrad` is **0.5** (owner decision, 2026-11-09). With 1.5 mrad, GN3
  (`AC-54.2`: ≥ 90 of 100 rounds fired from the pipper hit at 300 to 1 000 m) cannot pass: at
  1 000 m, over 10 runs of 100 rounds per aspect, σ 1.5 gave 63 to 73 % on average and a uniform
  1.5 mrad cone 86 to 97 % (worst run 81); σ 0.5 gives 96 to 100 % (worst run 94). WP1 had described
  the number as a uniform cone's half-angle; the Gaussian reading is the GDD's and the plan's.
- A hit is a **swept** test: the round's path over the tick, relative to the target and in the
  target's body axes at the end of the tick, against the target's capsules (GN4: 1 000 of 1 000
  rounds at 1 000 m/s through a 1 m capsule caught, 0 of 1 000 at 1.2 m). The first capsule entered
  wins. Aircraft against aircraft use the capsule-set clearance (≤ 0 is a collision).

### The gunsight (GDD 15.4.3)

A director (predictor) sight computed in the simulation, not the GDD's Newton iteration on an
analytic trajectory: a virtual round is fired now along the gun axis (no dispersion) and flown with
the real ballistics in steps of 1/120 s. For a target at range `R` moving at `v_t`, the point
`q(t) = b(t) − v_t t` is where the target must be now for a round to meet it at `t`; the solution is
the first `t*` with `|q(t*) − p_s| = R` (linear interpolation between steps) and the aim point is
`q(t*)`. It is exact for a non-manoeuvring target and costs one short integration a tick. GN2:
against a 10× finer solution the angle is under 5e-5 rad (gate 1 mrad). A target placed on the aim
point is passed within 0.3 m.

### The infrared signature (GDD 15.7, C.11)

`I = engine · f(AA) + skin · M²`. The engine term runs from idle to military with the core thrust and
from military to full afterburner with the afterburner level. **Reconciliation:** GDD 15.7 gives
generic relative intensities (idle 0.2, military 1.0, afterburner 3.5); C.11 gives the Kestrel's own
(dry 0.5, afterburner 1.0). The Kestrel's data use C.11's scale (military 0.5, afterburner 1.0) with
15.7's idle-to-military ratio (idle 0.1); the aspect pattern is 15.7's (rear 1.0, side 0.45, front
0.15, linear between 0°, 90° and 180°); skin heating is 0.05 × M².

### The seeker (GDD 15.7)

- Signal `s = (I / I_ref)(R_ref / R)²`, so the lock range is `R_ref √(I / I_ref)` (6 000 m at
  intensity 1).
- SEARCH: caged at its axis (the launcher's nose on the rail, the missile's velocity in flight) with
  a ±2° field of view. It sees the target when the terrain line of sight is clear, the target is in
  the field of view, the aspect angle is ≤ 35° (**rear-aspect cut-off**) and `s ≥ 1`. Each tick it
  sees, it is in TONE; after `round(lock_time_s / dt)` consecutive ticks (30) it LOCKS.
- LOCKED: it tracks within the 30° gimbal and keeps the lock while `s ≥ 0.8`, the aspect is ≤ 40°
  and the line of sight is clear (hysteresis against flicker at the edges); otherwise back to
  SEARCH. Non-finite inputs give signal 0 and SEARCH.

### The missile (GDD 15.5.2, 15.5.3)

- A 3-DOF point mass, state `(p, v, m)`: `dv/dt = (T − D)/m · v̂_air + a_lat + g`, a **constant-thrust**
  motor (the GDD's mass-flow curve is not built) that lights at launch and burns for `burn_s` with
  `ṁ = −propellant / burn_s`; `D = q̄ S (cd0(M) + k C_N²)` with separate boost and coast tables and
  `C_N = m |a_lat| / (q̄ S)`.
- **RK4 over two sub-steps per 60 Hz tick (120 Hz), always** (the GDD drops to 60 Hz far from the
  target; one rate is simpler and the cost is small with four missiles). A sub-step that contains
  burnout is split there.
- The autopilot is a first-order lag `τ`. Over a sub-step with the command held, `a_lat` follows
  `a_cmd + (a_lat − a_cmd) e^{−t/τ}`; the sub-step's RK4 holds `a_lat` at that curve's **mean** over
  the sub-step, and the sub-step ends at the curve's end value. Each RK4 stage removes the held
  value's component along the stage's velocity, and `|a_lat|` is held to the airframe's limit
  `min(g_max G0, q̄ S C_N,max / m)` (zero below `g_min_speed_mps`). MS1 (60 s against a 480 Hz
  reference with the same commands, gate 0.2 m) measures 0.11 m; holding the sub-step's end value
  instead, as the plan first wrote it, measures 3.6 m, and holding a fixed vector through the
  sub-step without the per-stage projection 3.2 m.
- Guidance: true PN, `a = N′ Vc (Ω × R̂)`, `Ω = (R × Vr)/(R·R)`, `Vc = −(R·Vr)/|R|`, N′ = 4, with
  the line of sight displaced by the seeker noise `|R| σ_los (n1 e1 + n2 e2)`, plus the cancellation
  of gravity's lateral part; the component along the velocity is removed and the command limited to
  the airframe's limit. No command unless the missile's seeker is LOCKED.
- Launch refusals, checked in every assist (no misfire model until v0.3a): too close
  (`min_range_m`), load factor, angle of attack, speed; `sim-core` checks safe, empty and no lock
  first.
- It ends on: ground impact; 25 s of flight; below 150 m/s after burnout; 2 s after its target is
  destroyed.

### The fuze and the warhead (GDD 15.6)

The fuze arms after 300 m flown and 0.5 s (`safe_sep_s`; the motor does not wait for it). Each tick
it takes the missile's path relative to the target in the target's axes and finds the closest
approach to the capsules' surfaces (0 inside one). It triggers at the **first local minimum** inside
`trigger_radius_m` (8 m): when the closest point of the tick's path is not the path's end (at the end,
the missile is still closing and the next tick comes closer). The warhead delivers
`damage_j (1 − d / R_lethal)²` (2 548 000 J at 0 m, 400 hit points of the Kestrel's 6 370 J each).

### The fly-out predictor (`IN RNG`)

A stand-in for the DLZ baker (GDD 15.5.5, v0.3a): a copy of the next missile as it would launch now,
flown at 60 Hz (one RK4 sub-step a tick) with a perfect lock and no noise against the target held at
its velocity and attitude, for the missile's flight time. True if the fuze would trigger inside the
lethal radius; false below the minimum launch range. `sim-core` runs it every 30 ticks.

### Damage (GDD 15.4.4, 18)

**Aggregate, linear in energy**: a round delivers `½ m |v_round − v_target|²` and a warhead its
energy above, both into the existing `AggregateHealth` seam (`joules_per_hp` from the aircraft's
data). The GDD's per-component model with the `(½ m v²)^0.7` law, armour classes and ricochet is
v0.3b.

### Deviations from the GDD, in one list

Constant thrust; aggregate linear-energy damage; 120 Hz missile integration always; the rear-aspect
cut-off at 35° with a 40° keep; the C.11 and 15.7 intensity scales reconciled as above; launch
refusal in every assist; a fixed Gaussian dispersion (no growth with barrel heat, g or slip); no gun
recoil, no store mass or drag on the flight model; a director sight instead of the iterated
analytic solution; the fuze's first-local-minimum rule over swept capsules.

## Alternatives considered

- **Dispersion as a uniform cone** (WP1's wording). Bounded, but GN3 needs the cone at 1.0 mrad or
  less, and the GDD defines the number as a Gaussian σ. Rejected by the owner for σ 0.5.
- **Holding the autopilot's end value through the sub-step** (the plan's first text). Simple, but
  first-order in the step: 3.6 m against MS1's 0.2 m. The mean is exact for a held command.
- **The analytic LCOS solution of GDD 15.4.3** (Newton on a drag approximation). Cheaper, but its
  error grows with range and drag; the director is exact for a non-manoeuvring target and the GDD's
  acceptance (`AC-15.2`) is stated against that.
- **Point-sampled hit tests.** A 27 mm round moves 17 m a tick; a sampled test misses most hits
  (GN4 checks that its point samples catch fewer than 300 of the 1 000).
- **The missile at 60 Hz with RK2.** Fails MS1 by far (deliberate fault 4 of the plan).

## Consequences

- `sim-core` (WP4) owns the pools, the slots, the random streams and the order of the steps; the
  functions here stay testable alone, and the gates GN1, GN2, GN4, MS1 to MS4 run in `cargo test -p
  weapons`, GN3 in `crates/weapons/tests/gun_hits.rs`.
- The Dart-1's *target* values may still be tuned in WP4 against MS5 within the plan's ranges; the
  GS-27 changed once (dispersion, above). Both are logged in `content/core/weapons/README.md`. After
  the one re-bless (end of WP4) the simulation pack does not change before the tag.
- The DLZ baker (v0.3a) replaces the predictor; component damage (v0.3b) replaces the aggregate
  energy model; radar seekers and countermeasures plug into the seeker's state machine.
