# ADR-48: Combat in the core: the duel scenario, ABI v5, snapshot v5, events and the combat hash

- **Status:** Proposed
- **Owner:** Bl4202
- **Date:** 2026-11-10
- **Affects:** GDD 15, 18.8, 19.12, 65.2, 65.3; ADR-10, ADR-16, ADR-23 (amendment 1), ADR-43,
  ADR-45, ADR-46, ADR-47; `crates/sim-core/src/combat/`, `crates/sim-core/src/layout.rs`,
  `crates/sim-core/src/session.rs`, `crates/sim-wasm/src/abi.rs`, `crates/cli/src/sortie.rs`,
  `crates/xtask/src/{sortie,determinism,golden_diff}.rs`

## Context

v0.1d puts a fight in the simulation: the player's gun and IR missiles against one bandit flown by
the AI (ADR-46 has the weapons, ADR-47 the brains). The core must run it deterministically,
without allocation after start, and expose it to the host through the snapshot without the host
ever reading simulation memory directly (ADR-16, ADR-43). A free-flight session must run exactly
as before, so that the v0.1c goldens keep their meaning apart from the pack's hash (ADR-45 D2).

## Decision

**A duel is a scenario of the session.** `fg_configure(spawn, assist, g_effects, arena, scenario,
skill) -> u32` (ABI v5). `scenario`: 0 free flight, 1 `DUEL` (the bandit patrols a CAP orbit 15 km
north of the anchor, radius 5 km, 4 500 m MSL, and is told of the player 10 s after the player is
airborne), 2 `DUEL_MERGE` (the first bandit 3 km ahead of an air start, already active; a runway
spawn is `INVALID`; tests and debugging only). `skill` is the bandit's rank, 1 to 3, found by
`rank`, never by content id. The session hash word gains `scenario << 32 | skill << 40`; free
flight writes 0 and 0, so every existing session hashes as before.

**One `Combat` per duel.** `SimCore::start` allocates one boxed `Combat` (fixed arrays: 192
rounds, 4 missiles, 8 explosions, a 32-entry perception ring at 10 Hz, a 32-entry event ring) and
nothing after. A reset drops it. The models (gun, missile, rails, IR signature, damage, the three
skills) are built once in `Services::combat` at content load and are read from there, not copied
into `Combat` (the plan said "copied at start"; reading them saves the copy and they never change
in a session). `CombatModels::from_registry` needs the gun, the rails, the IR signature and the
damage model; without them `Services::combat` is `None` and a duel session starts with no
`Combat`, flying as free flight.

**Five systems, each returning at once without combat** (no state change, no random draw), in
the existing stages: Ai (`CombatAi`: where both aircraft start the tick, activation, perception,
the brain's 5 Hz decision and every-tick manoeuvre into the bandit's own `InputState`), Dynamics
(`BanditDynamics`: `pilot::fly` for the bandit's slot after the ownship), Weapons
(`CombatWeapons`: arming, triggers, rounds, the seeker, launches and their refusals, missile
guidance and integration, the gunsights, the fly-out predictor every 30 ticks), Collision
(`CombatCollision`: swept round hits, fuzes, damage, kills and deaths, aircraft colliding, the
bandit meeting the ground) and Systems (`CombatLifecycle`: the wreck, the respawn after 20 s,
explosions ageing). The bandit flies the player's Kestrel with Standard assist and g-effects off,
from the same inputs a player has (GDD 19.12).

**Randomness** comes from five named Xoshiro streams of the master seed (gun dispersion, the
bandit's gun, the bandit's brain, missiles, spawns); the `weapons` and `ai-pilot` crates take
their draws as inputs and never own a generator. `rng::gaussian_pair` is the one new primitive.

**Perception** (a stub of GDD 16.6, the full audit is M3): GCI reports every 4 s with 300 m of
noise, dead-reckoned to now between reports; the eye within `R_vis` with terrain line of sight,
delayed by the skill's perception delay; missiles noticed with the skill's probability per
decision. When the bandit sees the player by eye, its gunsight solves against the player's true
state (the eye is what makes the sight's inputs good enough).

**Cues the bot reads.** The player's instruments are computed in the sim and published: the
director pipper (`C_PIPPER_BF`), `SHOOT_GUN` (the bandit within 6 mrad of the pipper and 1 000 m),
`IN_RANGE` (the predictor), `SHOOT_MISSILE` (locked, in range, no inhibit) and the seeker's mode.
The SORTIE bot (ADR-47) reads these and exact state; it is an instrument, not a player.

**Scripts.** `input_script::SORTIE = 4` (the bot's whole sortie), `HOLD = 5` (holds the heading,
height and CAS of its first tick), `HOLD_FIRE = 6` (the same, and presses FIRE_MISSILE once, at
the first tick the seeker is locked **with the weapons armed**: the seeker locks in 0.5 s and the
weapons arm 1 s off the wheels, so the plan's "first lock" was always refused). They are refused
unless the session is a duel, and hash under script tags 4 and 5.

**ABI v5.** `abi::VERSION`, `SNAPSHOT_LAYOUT_VERSION` and `snapshot::VERSION` are 5.
`button::FIRE_GUN = 11` (held), `button::FIRE_MISSILE = 12` (press), `crash_cause::SHOT_DOWN = 6`,
`crash_cause::COLLISION = 7`, `hash_kind::COMBAT = 7`. No new export. `SimCore::combat_setup`
(air starts, the brain kind, activation, GCI) is a tool's knob for the gate runners and tests, not
part of the ABI, like `set_spawn_fuel`.

**Snapshot v5** grows from 10 528 to 13 592 bytes: a combat block after the probe bodies (flags,
the seeker, the inhibit, the pipper, counters, one bandit, four missiles, 48 tracers, 8 explosions
and the event ring). Every field is derived at publication; none is hashed. All zero without
combat.

**Events.** A ring of the last 32 (`MISSILE_LAUNCH`, `MISSILE_HIT`, `MISSILE_MISS`, `KILL`,
`DEATH`, `BANDIT_SPAWN`, `LAUNCH_REFUSED`, `COLLISION`, `BANDIT_CRASH`), each with a sequence
number that starts at 1 and never repeats in a session, so the host reads every event once even
when it skips frames (up to 32 between reads). Kills follow GDD 65.2: a bandit that crashes within
30 s of the player's last hit is credited to the player.

**The combat hash.** `SubsystemHashes` has a seventh column, `combat`, inside `combined`. Without
combat it is the hash of the tag `NOCOMBAT` (a constant). With combat it covers every number of
the state in a fixed order (the bandit's slot, input state and brain, both health bars, the
weapons, rounds, missiles, explosions, perception, the random streams, the cues, the timers) and
nothing observational (the event ring, the counters, the stamps). ADR-23 amendment 1 has the
golden policy.

**The gate runners** live in `sim_core::combat::report` and run under `fg sortie` (ADR-45):
BP1/BP2 (`bot_sortie`, judged from the snapshot like the circuit judge, with the crash cause and
the bot's phase on every failure), MS5 (`sweep_shot`), AI1, AI2 and AI8 (`pursue_run`,
`defend_run`), and AI4 and AI5 over every bandit flown. `cargo xtask sortie` bakes, builds and
gates (CI `conformance`). `cargo xtask determinism` has a fifth suite, `combat`: the bot's 10-minute
sortie against a Regular, checked every 600 ticks.

## Alternatives considered

- *Combat as a separate world or crate stepping alongside the core:* two schedules to keep in
  step and two hashes to reconcile. One world with systems that are no-ops without combat keeps
  free flight bit-identical with no second code path.
- *Copying the models into `Combat` at start (the plan):* no benefit, since the models never
  change within a session and `Services` outlives `Combat`.
- *Hashing the events:* they are a view of state changes that are already hashed; hashing them
  would make the hash depend on the ring's capacity.
- *A new export for events:* the snapshot ring with sequence numbers needs no extra call and no
  allocation on either side.

## Consequences

- A free-flight session is unchanged except through the pack's SHA-256 (`content`, `combined`);
  `cargo xtask golden-diff --against v0.1c` proves it at the one re-bless (ADR-23 amendment 1).
- The bot's code is in the `combat` golden: any change to `ai-pilot` or the combat systems moves it
  and needs a bless with the reason.
- The host (WP5) reads combat only through the snapshot block and the event ring.
- Damage has no effect on flight until the aircraft is destroyed; fuel and bingo logic, more than
  one bandit, and bandit missiles are not built (plan section 1, ADR-47's known limits).
