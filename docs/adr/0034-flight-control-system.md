# ADR-34: The flight control system (`crates/fcs`)

- **Status:** Accepted
- **Owner:** Bl4202
- **Date:** 2026-10-24
- **Affects:** GDD 11.11, 11.13.4, 11.15, 58.4, 11.18, 11.21 (AC-11.6), 58.11 (AC-58.4); `crates/fcs`, `crates/flight` (`aircraft::Sense`, `aero::ControlModel`), `content/core/fcs/`, `tools/content-bake`, `crates/xtask` (layers, content ids)

## Context

Through v0.1a the stick moved the surfaces directly (`flight::controls::ManualLaw`). v0.1b adds
the four assist levels of GDD 11.11 (Full, Standard, Reduced, Manual), the angle-of-attack, load
and overspeed limiters, the ground law, the recovery button and mouse-aim. GDD 11.11.2 describes
classical loops (a PI on the load-factor error with a steady-pull feed-forward, pitch-rate
damping, an angle-of-attack feedback, a trim feed-forward) and 11.18 an autotuner. The Kestrel
flies from q-bar 2 kPa (a landing) to 110 kPa (M 1.2 at sea level), where the control power of
every surface changes by a factor of 50; a gain schedule for each loop would have to be built and
tuned over all of it. The gates are fixed in ADR-33 (before this existed, AC-66.4).

## Decision

### Crate, tick and state

- A new crate `crates/fcs` between `flight` and the integrators. It depends on `flight`,
  `simmath` and `content` only (`crates/xtask/src/layers.rs`); `sim-core`, `flight-test` and `cli`
  depend on it. `flight` stays airframe physics. The crate names no content entry: the gains come
  through the aircraft's own `fcs.gains_ref` (`check-content-ids` covers it).
- `FcsModel::step` runs **once per 60 Hz tick**, before the airframe's tick; its output is held
  over both RK4 sub-steps. Its state, `FcsState`, is `Copy`, has `is_finite` and a fixed word
  layout (`WORDS`), and is hashed by the caller. A step allocates nothing
  (`crates/sim-wasm/tests/fcs_no_alloc.rs`: every law, the ground law and its blends, mouse-aim,
  recover, G-LOC and assist switching).
- `PilotInput` is what a pilot (the stick, the virtual tester, later the AI) gives it: the
  controls, an optional aim, and the recover and G-LOC conditions.
- `flight::aircraft::Sense` gives the laws what the airframe is doing without the ground: the
  angular acceleration and the normal and side forces **exclude the wheels** (the push of the
  gear is not lift; the flight law cancelling it fought every touchdown).

### Incremental dynamic inversion

Every law except Manual asks for an **angular acceleration** and gets it from the surfaces by
inverting the airframe's own control power. The airframe reports its angular acceleration now
(`omega_dot`) and how much one radian of each surface changes it (`ControlModel`, from the same
tables the airframe flies on). The surfaces move by `B^-1 (nu - omega_dot)` from where they are,
at most `LEAD_S` = 0.1 s ahead of what the rate limit allows (without it the inversion at low
dynamic pressure asks for steps that the rate limit turns into phase lag). Pitch uses the
elevator alone; roll and yaw solve the 2 x 2 system of aileron and rudder, falling back to one
surface per axis where it is near singular. There is no gain schedule over dynamic pressure
because the inversion divides by the control power. The gains (`fcs/1`, ADR-30 style data) are
scalar except for the roll-authority schedules.

### Standard (the default)

- **Pitch** is a load-factor loop. With `alpha_dot = q - q_hold` and `q_hold = (g/V)(n -
  cos(gamma) cos(bank))`, asking for `n_dot = k_n (n_cmd - n)` gives `q_des = q_hold + (k_n /
  n_alpha) e_n`; `k_n` is scaled with the square root of q-bar (0.6 to 1.8 of its value at 20
  kPa). The stick is `n_cmd = 1 + s (n_max - 1)` aft and `1 + s (1 - n_min)` forward (1 g at the
  centre, read literally from GDD 11.11.2), through a prefilter `tau_cmd_s`. The integral
  collects only errors under 0.25 g (conditional integration: a manoeuvre is not an offset).
- **Limiters.** *Load:* `n_cmd` is clamped to the limit of the configuration
  (`n_max_by_config`, gear or flaps out 4 g). *Overspeed:* from 0.92 of `vmo` or `mmo` the command
  gains `k_pull` g per unit of ratio. *Angle of attack:* the pull is held to the rate from which
  the elevator could still stop at the limit (`BRAKE_SHARE` 0.4 of its power, `STOP_DELAY_S`
  0.3), and in a band of 3 deg below the limit the allowed pitch rate falls to the one that holds
  alpha and then brings it back, looked at `ALPHA_LEAD_S` 0.4 ahead along the **true** alpha rate,
  the roll-sideslip coupling included (`Sensed::q_hold`): rolling with sideslip changes the angle
  of attack as much as pulling.
- **Roll** commands the stability-axis rate `p_s = s p_max`, where `p_max` is the airframe's
  maximum shrunk by q-bar, alpha and load factor (data schedules), and further by the **inertia
  coupling** of rolling while pitching: the roll rate at which `(I_x - I_y)/I_z p q` uses half of
  the yaw acceleration the rudder has. Without it a rolling pull at the limit throws the nose
  sideways faster than the rudder can hold it (D1). The gravity-turn pitch rate of a 1 g bank is
  not counted, so a plain roll is as fast inverted as upright.
- **Yaw** coordinates the turn (`r_s = (g/V) sin(phi) cos(theta) + Y/(m V) + k_beta (beta -
  beta_cmd)`); the pedals command `beta_cmd` up to a q-bar schedule.
- **Continuous attitude.** Euler bank flips past 90 deg of pitch, so the loops use the bank of
  the lift vector about the velocity vector (`bank_w`) and gravity along the stability normal
  axis (`turn_load`), both continuous over the top of a loop. Over-the-vertical pulls (H5b) hold
  the angle-of-attack limit because of this.

### Full, Reduced, Manual

- **Full.** Stick back commands a flight-path rate (at most `gamma_rate_max`); centred, the path
  is held. Stick sideways commands a bank angle (at most 60 deg); centred, the wings level. Both
  ride on the Standard loops with the angle-of-attack limit `alpha_margin` (2 deg) tighter. In
  G-LOC the stick is zero and the law holds wings level and a climb or level path, never a
  descent. This is GDD 11.11.3 read literally; the clarification for GDD 0.2.2 is that the stick
  is a bank, not a roll rate.
- **Reduced.** Pitch-rate command (a q-bar schedule), roll-rate command without the alpha and
  load reductions, a yaw damper with washout and direct pedals. No angle-of-attack hard limit;
  the load limit is a warning. It can stall and depart.
- **Manual.** `ManualLaw` and nothing else (the Kestrel's `sas_required` is false). Switching to
  Manual sets the trim so the stick where it is gives the elevator the last command had.

### Ground mode (GDD 11.13)

Weight on wheels (any main leg loaded, debounced 0.1 s) switches every law but Manual to the
ground law: the stick moves the elevator about the held trim with pitch-rate damping, the
aileron is direct, the pedals move the rudder and steer the nose wheel (full range below 15 m/s,
the small range above 30 m/s), and the toe brakes brake. The weight is blended: 0.5 s on touchdown
and 2.0 s after lift-off. **From the first contact of a wheel the flight law is frozen** where it
was (`holding`) until the ground law owns the aircraft, so the jolt of the impact is not a
disturbance for it to cancel; its memory restarts when the weight comes off.

### Switching and recovering

- **Switching the assist level** in flight is bumpless: the new law's filters and integrators start
  from the present state at the next tick and the surface command blends from the last one over
  `assist_blend_s` (0.5 s). H12: surface step under 1 deg, load-factor transient under 0.3 g.
- **Recover** (the button held). *Full and Standard:* roll the wings level the short way, unloading
  while the bank is over 90 deg and holding 1 g to 20 deg, then pull to a 5 deg climb at 4 g, the
  limiters still on. *Reduced and Manual,* only if the aircraft has departed: ailerons neutral,
  rudder against the yaw rate, stick towards the unstalled side, **eased off** linearly between
  15 and 30 deg of alpha. Holding full anti-spin controls against a rotation that is no longer a
  stall starts an autorotation the other way (D3).
- **Departure** is a yaw rate about the stability axis above 30 deg/s, or alpha above 35 deg.
  ADR-33's D1 text says |r| < 30 deg/s; a commanded roll at a high alpha yaws the **body**
  without departing (`p sin alpha`), so the stability-axis rate is the one that means it.
  `Sensed::yaw_rate_stability` is that definition.

### Mouse-aim (GDD 58.4)

In the sim (`fcs::mouse_aim`), so AC-58.4 runs headless and deterministically; TypeScript sends a
world-frame aim. In Standard the nose is asked to close on the target at `k e` (capped at
`rate_cap_dps`), the rate it already closes at looked ahead by `lead_s`. The lift that does it,
in g, is the turn `V w / g` toward the target plus the lift that holds the path against gravity,
a vector in the body y-z plane. Its **direction is the roll stick** (it falls out as "wings level"
for a small error and "roll to the target" for a large one, with nothing singular at zero) and
its **part along the lift axis the pitch stick**, once the lift axis points that way. A target
under the nose is pushed down to while it is near and rolled over for when it is not. The pitch
stick is the load factor that makes the **pitch rate** follow the lift: the load-factor loop turns
a load-factor error into an alpha rate through `n_alpha` and its bandwidth, so the stick asks for
the fraction of the error that gives the rate. In Full, which points by a bank and a path rate, it
is a heading error to a bank and an elevation error to a rate. Mouse-aim is unavailable in
Reduced and Manual, which have only mouse-as-joystick. The GDD's 1.6 /rad, 3 /rad and 3 /s do not
survive: the structure does, the numbers are data (`content/core/fcs/README.md`). AC-58.4's
wording and result are in ADR-33, Amendment 1.

### Not in v0.1b

Auto-GCAS (the opt-in flag and the data block exist; the feature is off) and the GDD 11.18
autotuner (the handling suite is the gate). Both are on the plan's cut list.

## Alternatives considered

- **Classical gain-scheduled loops per GDD 11.11.2.** Each loop needs a schedule over a factor of
  50 in control power, tuned for every configuration; the inversion removes the schedule and
  uses the tables the airframe flies on. The classical structure (PI on `n`, pitch-rate damping,
  alpha feedback, trim feed-forward) is still visible in the loops above.
- **Full nonlinear dynamic inversion.** Needs the aerodynamics as a function of state in the
  law; incremental inversion needs only the control power and the measured acceleration.
- **Mouse-aim in TypeScript.** Not testable headless, so AC-58.4 could not gate commits.
- **Bank/pull mouse-aim exactly as GDD 58.4.** Built first. With the gravity left out, the pull
  gated by the bank and a 3 /s ramp it hunted in roll and passed none of 500 runs; the lift-vector
  form passes 98.6 %.

## Consequences

- Gains are data and each change is logged (`content/core/fcs/README.md`); the algorithm
  constants (`BRAKE_SHARE`, `STOP_DELAY_S`, `ALPHA_LEAD_S`, `COUPLING_SHARE` and the mouse-aim
  blends) are `// ALGO` in the code and change only with this ADR.
- The handling suite (H1 to H12), the departure suite (D1 to D3), the AC-11.6 fuzz and AC-58.4
  are tests in `crates/fcs/tests`. Full assist meets none of AC-58.4's time (its path rate is
  10 deg/s), by design.
- Departure is defined by the stability-axis yaw rate, which differs from ADR-33's wording; this
  ADR is the record of that reading.
- Kestrel-specific coupling (`I_x`, `I_y`, `I_z`, rudder power) enters the roll limit through the
  mass and control models, not through constants, so another airframe needs only its own data.
- The laws read the airframe through `AircraftModel::sense_light`: the same evaluation as `sense`,
  with `agl_m` the height of the CG and not of the lowest wheel. No law reads it, and finding the
  lowest of eight points costs a terrain lookup for each of them (v0.1b performance work, which
  moved no hash). The virtual tester, which does read the height of the wheels, keeps `sense`.
- Cost, native, on the M4, per aircraft-tick through `fg bench --aircraft`: about 6.6 µs in the air
  under every assist (6.5 to 6.65 µs for all four: the control system adds under 0.1 µs to the
  manual law, nearly all of the cost being the airframe's own evaluation) and about 34 µs on the runway, where the gear forces
  take a terrain sample and a surface normal per wheel per RK4 stage. In WASM the air figure is
  about 16 to 20 µs, at the informational budget of 20 µs (GDD 60.2); in the browser the whole
  sim step averages 22 µs a tick with one aircraft (PS-0, reference run A).
