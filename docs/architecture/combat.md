# Combat, as built in v0.1d

One gun, one rear-aspect IR missile and one AI bandit, in the deterministic core, with the HUD,
the sound and the debrief around them. The decisions are in ADR-46 (weapons), ADR-47 (the bandit
and the bot playtester), ADR-48 (combat in the core) and ADR-49 (the debrief, statistics and the
feel questionnaire). The gates are ADR-45. This page is the map.

## Where the code is

| Crate or package | What it holds |
|---|---|
| `crates/weapons` | Pure functions over plain structs, no allocation, no generator (draws come in as arguments): capsules and the swept segment test (`capsule.rs`), the round's ballistics (`ballistics.rs`), the director gunsight (`sight.rs`), IR intensity (`ir.rs`), the seeker (`seeker.rs`), the missile's guidance and RK4 flight (`missile.rs`), the fuze and the warhead (`fuze.rs`), the fly-out predictor (`predict.rs`) |
| `crates/ai-pilot` | The bandit's brain (`bandit.rs`), the bot playtester (`sortie.rs`), the straight-and-level test pilot (`hold.rs`), geometry (`geometry.rs`: ATA, AA, lead point), the manoeuvre controller (`maneuver.rs`: a goal direction and a load cap to sticks), the skill parameters (`skill.rs`) and the percept the core fills in (`percept.rs`) |
| `crates/sim-core/src/combat/` | The duel's state (`mod.rs`: `Combat`), the models from the registry (`models.rs`), the player's weapons (`player.rs`), the bandit (`bandit.rs`), the round pool (`rounds.rs`), missiles (`missiles.rs`), hits and damage (`hits.rs`), perception (`perception.rs`), events (`events.rs`), the hash (`hash.rs`), the snapshot block (`view.rs`), the five systems (`systems.rs`), the SORTIE script's frames (`sortie.rs`) and the gate runners (`report.rs`) |
| `crates/cli` | `fg sortie`: BP1, BP2, MS5 and the AI gates (`sortie.rs`); `cargo xtask sortie` bakes, builds and runs it |
| `packages/sim-bridge` | `combat-status.ts`: the snapshot's combat block as `CombatStatus`; the event cursor |
| `packages/render` | The bandit's model, the missile, tracer and explosion markers, and the HUD's weapon symbols |
| `packages/audio` | The combat voices in `FlightDsp` (gun, enemy gun, seeker growl, lock tone, launch, hits, explosions) |
| `packages/ui` | The Dogfight mode in the main menu, the debrief, the statistics store and screen, the feel questionnaire |
| `packages/app-shell` | `debrief-flow.ts`: opening the debrief at a sortie's end and the questionnaire over it |

The layering rules (`cargo xtask check-layers`): `weapons` depends on `simmath`, `flight` and
`content` only; `ai-pilot` on those and `fcs`; `sim-core` on both. Neither new crate knows the
core.

## One tick with combat

```text
Ingest     player input (host frames, or the circuit or SORTIE script's frame)
Ai         CombatAi: where both aircraft start the tick; activation; perception (the 10 Hz ring,
           GCI, the eye, missiles noticed); the brain's decision at 5 Hz and its manoeuvre every
           tick, into the bandit's own input state
Dynamics   OwnshipDynamics, ProbeDynamics, BanditDynamics (the bandit's slot through pilot::fly)
Weapons    CombatWeapons: arming; triggers; rounds spawned and flown; the seeker; launches and
           refusals; missile guidance and flight; the gunsights; the fly-out predictor (every 30 ticks)
Collision  TerrainContact (the player), CombatCollision: swept round hits, fuzes, damage, kills
           and deaths, the two aircraft colliding, the bandit meeting the ground
Systems    CombatLifecycle: the wreck, the respawn 20 s after, explosions ageing
Publish    PublishSnapshot (writes the v5 combat block)
```

Every combat system returns at once when the world has no `Combat`: no state change, no random
draw. So free flight hashes as it did in v0.1c, and the four old goldens changed only in their
`content` column (the pack grew), proven by `cargo xtask golden-diff --against v0.1c`.

## A duel in the session

`fg_configure(spawn, assist, g_effects, arena, scenario, skill)` (ABI v5). `scenario` 0 is free
flight, 1 `DUEL` (the bandit patrols a CAP orbit 15 km north of the anchor and is told of the
player 10 s after the player is airborne), 2 `DUEL_MERGE` (the first bandit 3 km ahead of an air
start, already active; for tests and debugging). `skill` is the bandit's rank, 1 to 3, looked up
by `rank`. The browser's `?scenario=duel|merge&skill=N` and the main menu's Mode and Bandit set
them.

`SimCore::start` allocates one boxed `Combat` with fixed arrays (192 rounds, 4 missiles, 8
explosions, a 32-entry perception ring, a 32-entry event ring) and nothing after it. A reset drops
it. The models (gun, missile, rails, IR signature, damage, the three skills) are built once at
content load in `Services::combat` and read from there.

## The formulas

The worked examples are the unit tests of each module; the derivations are ADR-46.

- **Rounds:** drag `k |v − w| (v − w)` with `k = ρ C_d(M) A / 2m`, gravity, wind; two RK2 sub-steps
  per tick. A round dies after 3 s, below 300 m/s or under the ground. Dispersion is a Gaussian of
  σ = 0.5 mrad on two axes about the harmonised gun axis. Rate: 1 700 rpm through an accumulator
  that fires the first round on the trigger's rising edge (GN5).
- **Hits:** each tick's round segment, in the target's body axes, against its capsules
  (`damage.capsules`). A hit deals `½ m v_rel²` joules; `joules_per_hp` turns them into hit points
  (GN6).
- **The director gunsight:** a virtual round flown at 120 Hz along the gun line; the aim point is
  where the target must be *now* for the rounds to meet it (`q(t*) = b(t*) − v_t t*` at the
  target's range). It moves with the shooter's nose: a shooter puts it on the target, never its nose
  on it (ADR-47 amendment 2).
- **IR and the seeker:** intensity from the engine setting, the aspect and the skin's heating; signal
  `(I / I_ref)(R_ref / R)²`. It acquires at a signal of 1 within the 2° caged half-field and 35° of
  the target's tail, locks after 0.5 s of tone, and keeps the lock within the 30° gimbal and 40°.
- **The missile:** proportional navigation (N' = 4) plus gravity compensation, limited by g and by
  `cn_max`, through a first-order autopilot lag; RK4 over two sub-steps per tick. A proximity fuze
  swept against the capsules: it arms after 300 m and 0.5 s, triggers within 8 m, and the warhead's
  energy falls as `(1 − d / r_lethal)²`.
- **The predictor:** a copy of the next missile flown at 60 Hz against the target at constant
  velocity with a perfect lock: `IN RNG` when it would fuze inside the lethal radius.

## The bandit

States: PATROL, INTERCEPT, PURSUE, GUN, DEFEND_GUN, DEFEND_MISSILE, EXTEND (the Veteran),
RECOVER, RETURN, WRECK. It decides at 5 Hz, holding a choice for the skill's commitment time, and
flies its goal every tick through the manoeuvre controller and Standard assist, with g-effects
off, on the player's own aircraft. The rules, in order: the wreck is final; recover from the
ground; return to the arena; defend a noticed missile; defend a gun threat after the reaction
delay; extend (Veteran); GUN when the pipper is near the seen player inside 1 200 m; pursue;
intercept; patrol. The skills (`content/core/skills/*.json`, `ai_skill/1`) set its perception
delay, notice probability, g, aim error, reaction times and tactics. The Recruit never fires.

It sees the player through GCI (every 4 s, 300 m of noise, dead-reckoned) and by eye within the
visual range with terrain line of sight, delayed by the skill. Its gunsight solves against the
true player when it sees the player by eye.

The bot playtester (`SortieTester`) flies the player's side for the gates: the FCS tester's
take-off, a climb, the attack (the seeker's pursuit, then the pipper), the return and the landing.
It is an instrument: it reads exact state and the cues the HUD shows.

## The snapshot's combat block

Snapshot v5 is 13 592 bytes; the combat block follows the probe bodies. It has:
- the flags (`ACTIVE`, `ARMED`, `TARGET`, `PIPPER`, `IN_RANGE`, `SHOOT_GUN`, `SHOOT_MISSILE`, …), the seeker, the signal and
  the last launch refusal with its age;
- the pipper, the target's range and closure;
- the counters (`COMBAT_COUNTER`: rounds, hits, missiles, kills, deaths, hits taken, …);
- one bandit (pose, surfaces, health, life, AI state, skill, spawn sequence);
- four missile slots, 48 tracers, 8 explosions;
- the event ring.

Every field is derived at publication; none is hashed. All zero without combat. The layout is
`crates/sim-core/src/layout.rs` (generated into `packages/sim-bridge/src/layout.gen.ts`), and
`tests/vectors/snapshot_v5.json` pins it.

## Events

A ring of the last 32: `MISSILE_LAUNCH`, `MISSILE_HIT`, `MISSILE_MISS`, `KILL`, `DEATH`,
`BANDIT_SPAWN`, `LAUNCH_REFUSED`, `COLLISION`, `BANDIT_CRASH`. Each has a sequence number that
starts at 1 and never repeats in a session, so the host's `EventCursor` reads each once even when
it skips frames. A bandit that crashes within 30 s of the player's last hit is the player's kill
(GDD 65.2). The debrief's recorder counts from the counters' rises, not from events.

## The hash

`combat` is the seventh column of `SubsystemHashes`, inside `combined`. Without combat it is the
hash of the tag `NOCOMBAT`, a constant. With combat it covers every number of the state in a fixed
order (the bandit's slot, input and brain, both health bars, the weapons, rounds, missiles,
explosions, perception, the five random streams, the cues, the timers) and nothing observational
(events, counters). `every_hashed_number_moves_the_combat_hash_and_no_observation_does` guards it.
The `combat` determinism suite (`tests/golden/combat.json`) is the bot's 10-minute sortie against a
Regular, checked every 600 ticks, native, debug and both WASM builds alike.

## Adding a weapon by data

1. A `weapon/1` file in `content/core/weapons/` with an id of type `wpn` (`core:wpn/<name>`), a
   `class` of `gun` or `srm` and exactly the block of its class. Every number carries its unit in
   its key; see `content/core/weapons/README.md` and `content/schemas/weapon.schema.json`.
2. Point the aircraft at it: `internal_weapons.gun.ref` for a gun (with `rounds`, `muzzle_m` and
   the harmonisation), `loadout.rails[].store` for a missile on a station.
3. `cargo xtask content` bakes it. The baker checks the schema, the class of each reference, and
   the cross-checks (gimbal ≥ half the field of view, the fuze arming before the shortest launch,
   the trigger inside the lethal radius, the propellant lighter than the missile).
4. The weapons are in the sim pack: a new or changed weapon changes the pack's SHA-256, and with
   it every determinism golden's `content` column. Bless them in their own commit with the reason.
5. The HUD's weapon symbols (`hud/1` types `gun_reticle`, `target_box`, `seeker`, `kill_cue`,
   `weapon_status`) and the audio profile's `weapons` block are presentation data;
   `rate_hz` of the gun voice must equal the gun's `rpm / 60`.

The v0.1d core handles one gun and one missile type per aircraft; a second missile type or a
radar missile is later work.

## Checks that guard it

- `crates/weapons` unit tests: GN1 to GN4, MS1 to MS4 (worked examples, the swept test, PN).
- `crates/sim-core/src/combat/tests.rs`: GN5, GN6, kill credit (ST1's sim cases), DT3 (reset),
  the hash's coverage, a Regular hitting a player flying straight.
- `fg sortie` (CI `conformance`): BP1, BP2, MS5, AI1, AI2, AI4, AI5, AI8.
- `cargo xtask determinism`: the `combat` suite; `crates/sim-wasm/tests/no_alloc.rs`: a duel under
  each assist with no allocation after `fg_start`.
- `crates/fcs/tests/nn2.rs`: G1-c, the control system never moves the aircraft.
- e2e: `combat.spec.ts`, `dogfight.spec.ts`, `debrief.spec.ts`, `stats.spec.ts`; the soak
  (`tests/e2e/soak/`).
