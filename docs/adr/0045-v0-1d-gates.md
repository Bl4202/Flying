# ADR-45: v0.1d gates: weapons, the bandit, the bot playtester, the soak, the debrief and G1 for a solo project

- **Status:** Proposed
- **Owner:** Bl4202
- **Date:** 2026-11-09
- **Affects:** GDD 4.5, 15, 16.6.2, 18, 19, 54.4, 54.5, 56, 58.5, 60.9, 65, 66.4; `crates/weapons`,
  `crates/ai-pilot`, `crates/sim-core`, `crates/fcs`, `packages/*`, `tests/`, `.github/workflows/*`

## Context

The v0.1d section of docs/roadmap.md reads: "One gun and one IR missile (simple proportional
navigation); one AI bandit (state machine) that pursues and defends. Debrief and statistics stub
(§65 basic). Gate G1 (flight feel) with ≥ 12 blind testers (§66.4); 30-minute soak with no leaks."
Its exit is: "G1 passes; ≥ 80 % of external playtesters take off, fly, shoot a bandit and land
within 10 minutes (`AC-4.5` precursor)."

v0.1d closes milestone M1 (GDD 66.3). M1's exit list is G1, `AC-3.2`, 60 FPS on Recommended in the
arena, input-to-control latency ≤ 25 ms p95, a 30-minute soak with no leaks, and the AC-4.5
precursor. This ADR fixes every pass/fail threshold **before** any weapon, AI or combat code exists,
so that none can be adjusted to fit what the code happens to do (AC-66.4).

The project is solo (OQ-02), so neither a blind panel of 12 testers nor a group of external
playtesters exists. Three points needed the owner (decisions of 2026-11-08):

- **D1, who decides the tag.** The automated gates below decide it: the conformance suite, an audit
  that the flight-control system cannot fake the aircraft's response (NN-2), and a bot playtester
  that takes off, kills a bandit and lands under every assist level. The owner fills in the feel
  questionnaire and does the 10-minute task once per assist level; the results are recorded beside
  the bots' in the exit doc and do not block. The 12-tester blind study, G1(a), is dropped from
  v0.1d and kept open as OQ-11. v0.2 is not blocked by G1(a).
- **D2, the goldens.** The weapon and AI data go into the simulation pack. The pack's SHA-256 is part
  of the state hash, so the `content` and `combined` columns of every golden move. The four existing
  goldens are re-blessed **once**, after every simulation datum is final, and a checker
  (`cargo xtask golden-diff --against v0.1c`) proves that every other column is byte-identical and
  that the new `combat` column equals the no-combat constant (DT1).
- **D3, the bandit's gun.** The bandit fires the gun by skill: Recruit, Regular and Veteran (GDD
  19.6). **The Recruit never fires** (a deviation from the GDD, recorded in ADR-47). Regular and
  Veteran fire with an aim error that depends on the skill. The bandit has no missiles.

## Decision

Every gate below has an id. "Gate" = blocks the tag. "Recorded" = written in the exit doc, does
not block. "Informational" = measured and reported only.

### 1. G1, as decided for a solo project (exit item 1)
| # | Test | Pass |
|---|---|---|
| G1-b | `cargo xtask conformance` (unchanged) | AC-3.2: 12 of 12 points within tolerance (gate) |
| G1-c1 | **NN-2 structural audit** (`crates/fcs/tests/nn2.rs`, new): 2 000 random aircraft states × 4 assists × random pilot inputs; call `FcsModel::step` | It never changes `ac.body` (position, velocity, attitude, rates), `ac.engines`, `ac.fuel`, `ac.ground` or `ac.surfaces`; only `ac.trim_pitch_rad` may change (gate) |
| G1-c2 | **NN-2 behavioural test** (same file): Full assist, roll stick held full right, pitch stick neutral (Full holds the flight path and banks to its limit), throttle military, trimmed level at 3 000 m MSL, starting once at M 0.5 and once at M 0.85; 24 s each | The mean heading rate over seconds 4 to 8 differs between the two speeds by ≥ 10 %, and at M 0.85 the mean heading rate over seconds 20 to 24 differs from that over seconds 4 to 8 by ≥ 5 % as the turn bleeds speed (a fixed-rate turn would give neither) (gate) |
| G1-owner | The owner's feel questionnaire, once per assist level (four sets of answers) | Recorded: medians beside the GDD's targets (≥ 4.0 believable and satisfying, ≥ 3.8 easy to learn on Full) |

### 2. AC-4.5 precursor (exit item 2)
| # | Test | Pass |
|---|---|---|
| BP1 | `cargo xtask sortie`: seeds 1 to 50 × {Full, Standard, Reduced, Manual}; `rugged`; runway spawn; scenario DUEL, skill 1 (Recruit); script SORTIE; at most 36 000 ticks (600 s). A run **succeeds** if, in order: the aircraft lifted off; the bandit was destroyed with the kill credited to the player; the aircraft then touched down and stopped (ground speed < 2 m/s) on the runway (|across| ≤ 22.5 m, 0 ≤ along ≤ 3 000 m); and at no time was it destroyed | **≥ 40 of 50** successes in **each** assist (gate) |
| BP2 | The same, Standard assist only, skills 2 and 3 | Informational: success rate and player deaths per skill |
| BP-owner | The owner's 10-minute task once per assist (Dogfight, Recruit) | Recorded: the debrief's task time per assist |

### 3. Gun (exit item 3)
| # | Test | Pass |
|---|---|---|
| GN1 | AC-15.1, `weapons::ballistics` unit tests: a round in vacuum (drag off) after 1 000 m | Velocity and drop within 0.5 % of `v = v0`, `drop = ½ g t²`; with drag, position at 1.0 s within 1 % of a reference integrated at 10× finer steps (gate) |
| GN2 | AC-15.2: `weapons::sight` against a non-manoeuvring crossing target at 600 m, 10 geometries (crossing angles 30° to 150°, speeds 150 to 300 m/s) | Angle between the solution and a 10×-finer reference solution < 1 mrad (gate) |
| GN3 | AC-54.2: shooter aims its gun so the target sits exactly on the pipper (no pilot error), dispersion on; constant-velocity Kestrel-sized target (the five capsules) at 300, 600 and 1 000 m, aspects 0°, 45°, 90°, 135°; 100 rounds per case | ≥ 90 % of rounds hit in each of the 12 cases (gate) |
| GN4 | AC-15.8: 1 000 rounds at 1 000 m/s relative crossing a 1 m-radius capsule at 60 Hz, random offsets inside 0.9 m; 1 000 more passing at 1.2 m | 1 000 of 1 000 detected; 0 of 1 000 detected (gate) |
| GN5 | Rate and count (sim-core unit test) | 150 rounds take 150/28.333 s = 5.29 s ± 1 tick; firing stops at 0 rounds; no round leaves while weight is on the wheels or within 1.0 s of lift-off (gate) |
| GN6 | Damage (sim-core unit test): hits at a fixed relative speed until destroyed | **20** hits at v_rel 500 m/s, **14** at 600 m/s, **7** at 900 m/s (`½ · 0.26 · v² / 6370` hp each against 100 hp; these speeds avoid a result that lands exactly on a whole number) (gate) |

### 4. IR missile (exit item 4)
| # | Test | Pass |
|---|---|---|
| MS1 | AC-15.3: a 60 s flight with a fixed per-tick command schedule, against a reference with the same commands at 480 Hz RK4 | Position error < 0.2 m at 60 s; in an unguided coast with thrust off, specific energy `½V² + g h` never rises from one tick to the next (gate) |
| MS2 | Seeker unit tests | Lock range against intensities 0.1, 0.5, 1.0, 2.0 within 1 % of `6000 √(I/1.0)`; no tone with the target's aspect above 35° (rear-aspect cutoff); lock lost outside the 30° gimbal; tone before lock; lock after 0.5 s ± 1 tick of continuous tone (gate) |
| MS3 | Guidance unit tests | The PN command is perpendicular to the line of sight (before gravity compensation) and zero on a collision course; the lateral limit is `min(30 g, q̄ S 12 / m)` and zero below 150 m/s; the autopilot reaches 63 % of a step command after τ = 0.25 s ± 1 tick (gate) |
| MS4 | Fuze and warhead unit tests | No detonation before 300 m flown or 0.5 s; detonation at the swept closest approach when within 8 m; damage `2 548 000 × (1 − d/8)²` J; self-destruct at 25 s, or below 150 m/s after burnout, or 2 s after the target is destroyed; ground impact expires the missile (gate) |
| MS5 | NN-3 sweep (`fg sortie --sweep`, run by `cargo xtask sortie`), set up with `combat_setup`: launcher (script HOLD_FIRE) and target co-speed 230 m/s TAS, 4 000 m MSL, the launcher pointing at the target; target aspect 0° and 30° (off its tail); range 1.0, 2.0, 3.0 km; 200 shots per cell with seeded jitter (launch position ±30 m, seeker noise); target either `BrainKind::HoldCourse` (non-manoeuvring) or a Veteran with `BrainKind::DefendOnly`, `gci: false` | Non-manoeuvring: ≥ 85 % hits in every cell the predictor marks `IN RNG`. Defending: **strictly lower** than non-manoeuvring in every cell, **< 95 %** in every cell (AC-15.6), and **> 5 %** in at least one cell (gate) |

### 5. The bandit (exit item 5)
| # | Test | Pass |
|---|---|---|
| AI1 | Pursue, set up with `combat_setup`: the player flies script HOLD (4 000 m MSL, 200 m/s TAS, heading 090) starting 8 km due north of the bandit, which starts active (`bandit_active: true`, `gci: true`, `BrainKind::Normal`) heading 000 at 4 000 m; 50 seeds × 3 skills | The bandit reaches a gun position (range ≤ 1 000 m, its antenna train angle ≤ 10°, aspect angle ≤ 60°) within 180 s in ≥ 95 % of runs per skill (gate) |
| AI2 | Defend against the gun, set up with `combat_setup`: the bot (script SORTIE, which starts in Attack when spawned in the air) starts 1 500 m directly behind the bandit, co-speed 200 m/s, co-altitude 4 000 m, and tries to hold a gun solution for 30 s; 50 seeds | The median time the bot holds a solution (range ≤ 1 000 m, aim error ≤ 6 mrad) against Regular **and** against Veteran is lower than against a non-manoeuvring copy (the same bandit with its brain replaced by "hold course") (gate) |
| AI3 | Defend against missiles | MS5's ordering (gate) |
| AI4 | Same physics (AC-19.2 subset), over every BP1, BP2, AI1 and AI2 run | The bandit's structure never reports `OVERSTRESS`, and its CAS never exceeds V_mo × 1.01 (gate) |
| AI5 | Terrain (AC-19.6 spirit), over the same runs (≥ 500 bandit sorties over `rugged`) | 0 ground impacts by a living bandit (gate) |
| AI6 | Fail-soft (unit test) | A NaN in any percept field gives a finite in-range command and the RECOVER state within one tick; no panic (gate) |
| AI7 | Skill ordering, BP2 data | Informational: success rates and player-death counts by skill |
| AI8 | Holds fire, over every run | A Recruit fires 0 rounds; Regular and Veteran fire in ≥ 80 % of AI1 runs (gate) |

### 6. Debrief and statistics (exit item 6)
| # | Test | Pass |
|---|---|---|
| ST1 | AC-65.1 subset: 4 sim cases (`combat` unit tests: gun kill, missile kill, crash after damage within 30 s, crash with no damage) and 12 recorder cases (`recorder.test.ts`, synthetic event streams) | Each gives the expected kill / no-credit / death (gate) |
| ST2 | AC-65.2: aggregate one result 5 times; inject a throw between the store's write steps | Totals equal one aggregation; after the fault and a retry, the mission is counted exactly once (gate) |
| ST3 | AC-65.4 subset: 10 scripted status sequences (take-off, touch-and-go, landing, a crash, an air start) | Sortie start/end, successful landing, longest sortie, highest altitude, fastest TAS and Mach as §65.2 defines them (gate) |
| ST4 | `debrief.spec.ts` (and the recorder's unit tests for the timing) | The debrief opens on its own 5 s after a full-stop landing in a duel (the recorder's `sortieEnded()`; e2e allows 10 s), and from "End sortie"; it is fully keyboard-operable; its numbers equal the snapshot counters' change since Fly; its DOM is gone after it closes (gate) |
| ST5 | `stats.spec.ts` | Statistics shows totals; export → import round-trips byte for byte; Clear empties them (gate) |
| ST6 | `feel-survey.test.ts` and `debrief.spec.ts` | Three 1-to-5 answers and an optional comment ≤ 500 characters are stored with assist, skill, build and date; keyboard-operable; exported (gate) |

### 7. Soak (exit item 7)
`tests/e2e/soak/soak.spec.ts` (Playwright project `soak`), page
`?debug=1&fly=1&scenario=duel&skill=2&script=sortie&res=640x360&quality=low`, Chromium with
`--js-flags=--expose-gc --enable-precise-memory-info`. Warm-up 120 s (`SOAK_WARMUP_SECONDS` to
change it; the quick CI check uses 30), then the measured window (1 800 s for the gate,
`SOAK_SECONDS` to change it). Each time the debrief opens, the harness
presses "Fly again". A sample every 60 s.

| # | Measure | Pass |
|---|---|---|
| L1 | JS memory: `performance.measureUserAgentSpecificMemory()` total bytes (if the API is missing: `gc()` then `performance.memory.usedJSHeapSize`; the report says which) | Least-squares slope over the window ≤ **1.0 MB per 10 minutes** (gate) |
| L2 | DOM: after `HeapProfiler.collectGarbage` (CDP), `Memory.getDOMCounters()` at each sortie start | Nodes at the last sortie start ≤ nodes at the second + 20; JS event listeners ≤ second + 2 (gate) |
| L3 | WASM: `memory.buffer.byteLength` of the sim module (`SimStats.wasmBytes`) | Equal at every sample after the first sortie start (gate) |
| L4 | Workers | 0 sim-silence warnings (the supervisor's `onSilent` calls), 0 worker `error` events, 0 device losses (render `lost` messages), the number of `Worker` objects the app created constant (gate) |
| L5 | GPU: live buffers and textures (new backend counters) at each sortie start | Equal at every sortie start after the second (gate) |
| L6 | Audio: live `AudioNode` count (`WebAudioBackend` counter) | Equal at every sample after the first sortie start (gate) |
| L7 | Health | Sim average over the window ≥ 59.5 Hz on the reference machine, ≥ 57 Hz in CI (ADR-28's CI figure); no console error; in a full run, at least 3 sorties completed (a sortie takes 7 to 9 minutes), so L2 and L5 have at least 4 sortie starts to compare (gate) |

Seeded faults (debug only, `debug=1` required): `fault=leak-js` keeps 2 KB per frame in a global
array → L1 must fail; `fault=leak-gpu` creates one GPU buffer per sortie start and never destroys
it → L5 must fail. Both proven once with `SOAK_SECONDS=300` and recorded in the exit doc.

Where: (a) the nightly workflow gains a `soak` job (not required); (b) `perf-smoke` (required) runs
the soak harness for `SOAK_SECONDS=120` with the gates L3, L4, L6, L7 only (a harness check);
(c) the exit needs one green full run on the final commit (nightly or dispatch) **and** one full run
on the Mac M4 (WebGPU, mains, `pnpm soak`), report in `docs/exit/v0.1d/soak/`.

### 8. HUD (exit item 8)
| # | Pass |
|---|---|
| HU1 | A worked-example unit test per new element: pipper and ring at the solution; the ring's arc is `range / ring_max_m` of a turn; target box at the bandit's direction inside the TFOV and the arrow on the locator circle outside it; seeker circle caged at the boresight, diamond on the target when locked; `weapon_status` texts; `HIT` for 0.3 s and `SPLASH` for 2.0 s (gate) |
| HU2 | The drawn pipper direction equals the direction from the eye to the interpolated `C_PIPPER_BF` within 0.05 mrad (gate) |
| HU3 | AC-54.8: the shipped `hud/1` passes; the invalid-layout corpus grows from 40 to **50** files (10 for the new types) and every one fails with file, JSON pointer and hint (gate) |
| HU4 | The font covers every new HUD string; the HUD's garbage harness keeps its bounds (gate) |
| HU5 | `combat.spec.ts`: with `?debug=1&fly=1&spawn=air&scenario=merge` (the bandit starts 3 km ahead), HUD pixels appear around the bandit's projected position, and the gun reticle draws once the range is inside 2 km (gate) |

### 9. Combat audio (exit item 9)
| # | Pass |
|---|---|
| AU1 | While the gun fires, the gun voice's envelope repeats at 1 700/60 Hz within 1 % (autocorrelation over 2 s rendered offline) (gate) |
| AU2 | The growl's fundamental rises monotonically with the signal ratio over 1 to 4; the lock tone's frequency is within 1 % of the profile's (gate) |
| AU3 | In the browser, launch, hit-taken and explosion transients start within 100 ms of the snapshot whose counter changed (p95 over 10 events each) (gate) |
| AU4 | `process()` still allocates within A7's bound (gate) |
| AU5 | A5 (the circuit fixture's −16 ± 1.5 LUFS) still passes; a 60 s combat fixture stays ≤ −1 dBTP (gate) and its loudness is reported (informational) |

### 10. Controls (exit item 10)
| # | Pass |
|---|---|
| CT1 | Catalogue v3 has both actions; every built-in profile is conflict-free and adds no new browser-shortcut warning; a stored catalogue-2 custom profile gains the weapon bindings where the inputs are free and a note where not (gate) |
| CT2 | The pump sets `FIRE_GUN` while held and `FIRE_MISSILE` while pressed; a neutral frame (focus lost) clears both; an action held while a menu closes does not fire until released (gate) |
| CT3 | `combat.spec.ts`: Space fires (rounds count falls); M launches when locked (gate) |

### 11. Performance (exit item 11)
| # | Pass |
|---|---|
| PF1 | COMBAT scenario on run A (Mac M4, headed Chrome, WebGPU, mains, High, `?res=1920x1080`, cockpit, HUD and sound on): bot sortie against a Regular bandit from an air start, 120 s after a 5 s warm-up. The ARENA gates: average ≥ 60 FPS, frame interval p99 ≤ 22.2 ms, render CPU average ≤ 7.0 ms and p99 ≤ 11.0 ms, PS-0's strict sim gates, no device loss, no main-thread task over 50 ms after warm-up (gate) |
| PF2 | Control latency run L with the COMBAT scenario running: p95 ≤ 25 ms (gate) |
| PF3 | Run B (WebGL2, 4× CPU throttle) COMBAT ≥ 30 FPS average (gate) |
| PF4 | CI `perf-smoke`: the COMBAT report is valid and the sim keeps ≥ 57 Hz (gate) |
| PF5 | `cargo xtask bench` (erratic, no combat) within its 5 % budget; `fg bench --scenario duel --skill 2 --script sortie` reported (informational) |

### 12. Determinism (exit item 12)
| # | Pass |
|---|---|
| DT1 | `cargo xtask golden-diff --against v0.1c` after the one re-bless (end of WP4): same files, checkpoints and session strings; `tick_h`, `rng`, `bodies`, `ownship`, `input` byte-identical at every checkpoint; `content` one constant across all checkpoints of all four files; `combat` equal to the no-combat constant everywhere (gate) |
| DT2 | New suite `combat`: script `sortie`, session `--spawn runway --assist standard --g full --scenario duel --skill 2`, 36 000 ticks, checkpoints every 600 (61); native release = debug (at tick 3 000) = WASM single = WASM threads; arm64 (gate) |
| DT3 | `fg_reset` after a duel, then configure and start, equals a fresh core in every column including `combat` (gate) |

### 13. The rest
| # | Pass |
|---|---|
| NA | No allocation: `no_alloc.rs` gains a duel sortie under each assist (bursts, launches, a kill, the wreck, the respawned bandit, the player shot down and respawned) with 0 allocations after `fg_start`; the recorder's per-frame update makes 0 B at rest (garbage harness) (gate) |
| LY | `check-layers` (with the two new crates' rules), `check-content-ids`, `depcruise` (gate) |
| DOM | AC-53.4: in flight in a duel with the overlay open, ≤ 150 DOM nodes (gate) |
| SG | `deploy-staging` and `staging-smoke` green on the final commit (gate) |
| CI | Every required job green on the final commit (gate) |


## Alternatives considered

- **Humans gate the tag** (the roadmap as written: a blind panel of ≥ 12 testers and external
  playtesters). Rejected by the owner: the project is solo (OQ-02) and no panel exists. The study is
  kept as OQ-11 for when one does.
- **Keep the old goldens byte-identical with a second pack** that holds only the combat data, so the
  free-flight sessions would hash exactly as before. Rejected (D2): more plumbing in the loader, the
  bake and the hash for no gain in safety, since DT1 proves column by column that free flight did
  not change.

## Consequences

- G1(a), the blind panel, stays open as OQ-11. M1 closes on the bots' evidence, the conformance
  suite and the NN-2 audit, with the owner's own ratings recorded.
- The bot playtester is an instrument: it reads the exact state, so BP1 shows that the task can be
  done within 10 minutes under every assist level, not that a new player will do it. The owner's
  10-minute runs are the only human evidence in v0.1d.
- The CI `determinism` jobs are expected to fail on the draft PR from the first content change until
  the one re-bless; `golden-diff --fresh` is the check in between.
- Every gate above is written before the code. A gate that turns out to be unmeasurable or wrong is
  changed only by an amendment to this ADR with the reason, never by editing a threshold in a test.

## Amendment 1 (2026-11-10): BP1 under Full, and MS5's cells without a lock

**BP1 under Full is informational (owner decision D6).** Full flies the stick as a bank angle of at
most 60° and a flight-path rate of at most 10°/s (ADR-32): a level turn of at most 2 g. A Recruit
pulls about 5 g, so under Full the bot cannot turn with it; its only kills come from head-on gun
passes, at the merge or a later one (37 of 50 with the bot's final aim, ADR-47 amendment 1). Full
is built for flying, not for dogfighting, and the owner chose not to change that in v0.1d. BP1 now
reads: **≥ 40 of 50 successes under Standard, Reduced and Manual each (gate); Full's rate reported
(informational).** `fg sortie` still flies and reports the Full cell; the exit doc records its
rate. Whether Full should be able to fight is a design question for later.

**MS5 judges the cells where the seeker can lock.** With launcher and target co-speed and the
launcher pointing at the target, the 30° cells cross the line of sight at about 115 m/s. At 1 km
that is 6.6°/s, and the target leaves the caged 4° field of view before the 0.5 s of tone the lock
needs; at 3 km the target's 30°-aspect signature is below the lock threshold. No missile leaves in
those two cells, under either target, so they have no hit rate. The defending criteria (strictly
lower, < 95 %, > 5 % somewhere) are applied over the cells where the non-manoeuvring target was
marked `IN RNG` and the defending target saw launches (4 of the 6); `fg sortie` prints the other two
as "no launch". After the Dart-1's tuning (`content/core/weapons/README.md`) the result is:
non-manoeuvring 100 % in all four judged cells; defending 80 % at 0°/1 km and 0 % at 0°/2 km,
0°/3 km and 30°/2 km.

## Amendment 2 (2026-11-10): the browser specs as built

**No lock comes at the merge with the stick hands off, so no browser test sees a missile launch.**
The merge starts the bandit 3 km ahead on the player's heading, tail-on. The player's nose sits
2.85° above the bandit (the angle of attack), outside the Dart-1's caged 2° half-angle. The Recruit
turns into the player at once, so its aspect passes the seeker's 35° limit at 1.9 s, before the
nose comes within 2° of it at 2.0 s (seed `1f1e5eed00000001`; the `fg` probe that measured this is
not committed). The bot playtester seldom launches: 0 missiles in 44 sorties before ADR-47
amendment 2 (20 against the Recruit, 12 each against the Regular and the Veteran, Standard), and 6
in the 150 BP1 runs after it. Nearly all its kills are gun kills. The launch path is judged where
its geometry is fixed: CT2 and the core's
missile tests, MS1 to MS5 in `fg sortie`, and the scene's tests for drawing a missile in flight.
So:

- **CT3** checks that M reaches the launch: either a missile leaves, or the core refuses it with
  `NO LOCK` and keeps it on the rail. Space must still lower the rounds.
- **HU5** in `combat.spec.ts` checks the core's cue, not the pixels. The `PIPPER` flag must be set
  while the range is inside 2.5 km, the gunsight's ranging (ADR-48), rather than 2 km: hands off,
  the bandit leaves the nose soon after 2.5 km. The pixels around the bandit's projected position,
  and the reticle and target box drawn on it, are the renderer contract's `HU5` case. It is a fixed
  scene (the bandit 400 m ahead, 40 m right) run on both backends.
- `combat.spec.ts` also checks that the bandit and the tracers are drawn. It reads them from the
  render stats' new `drawnFrames` counters: frames drawn with the bandit, with a missile in flight,
  and with a tracer.

**The dogfight spec's bot kill flies the sortie from the runway, not the merge.** The bot is built
for its sortie: take-off, climb, attack (ADR-47). From the merge's tail chase it circles the
Recruit without a kill (5 rounds in 7 minutes). `dogfight.spec.ts` (b) is
`scenario=duel&skill=1&script=sortie` and needs a kill within 5 minutes of wall time. In the CLI,
17 of 20 seeds kill the Recruit, 15 of them between 53 and 79 s of simulated time. The browser's
run took 1.2 minutes of wall time, including the page load.

**The `leak-gpu` proof needs three sortie starts.** L5 compares the GPU resources at every sortie
start after the second. The browser's sortie, from take-off to the stop, takes about 5.3 minutes.
`SOAK_SECONDS=300` after a 120 s warm-up holds only two starts, so it cannot fail L5. The seeded
`leak-gpu` run uses `SOAK_WARMUP_SECONDS=0` and `SOAK_SECONDS=900`, and `leak-js` keeps
`SOAK_SECONDS=300`.

**G1-c2 as measured.** `crates/fcs/tests/nn2.rs` was missing when the code was otherwise ready for
merge 1: the plan names it in the gate table but gives it to no work package. It was written before
the merge. The behavioural test expected the turn at M 0.85 to bleed speed, which would raise the
heading rate. At military power the Kestrel gains speed instead (292 to 323 m/s), so the rate falls
from 3.39 to 2.99°/s (-12 %). The gate, a change of at least 5 %, holds either way: a fixed-rate
turn would give no change. At M 0.5 the rate is 5.51°/s, 63 % above M 0.85's (gate ≥ 10 %). The
structural audit (G1-c1) made 24 000 calls, 800 of them on the wheels; the control system changed
only the pitch trim (9 081 times).

**Where two gates are tested.** ST6's unit tests are in `packages/ui/src/debrief.test.ts` ("ST6:
the feel questionnaire"), not a `feel-survey.test.ts` of their own. The DOM budget in a duel, with
the overlay open, is a test in `combat.spec.ts`. `overlay.spec.ts` keeps its free-flight one.
