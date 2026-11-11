# ADR-47: Combat AI: the bandit's state machine, the manoeuvre controller and the bot playtester

- **Status:** Accepted
- **Owner:** Bl4202
- **Date:** 2026-11-09
- **Affects:** ADR-33, ADR-34, ADR-45; GDD 19.4, 19.6, 19.9, 19.12, 66.3 (M1), B.5;
  `crates/ai-pilot`, `crates/fcs` (the tester's two additions), `crates/sim-core` (WP4),
  `content/core/skills`

## Context

v0.1d needs "one AI bandit (state machine) that pursues and defends" (the roadmap), and, because
the project is solo, a bot that proves the AC-4.5 precursor by flying a whole sortie (ADR-45, D1:
the bots gate the tag). ADR-45 fixed the gates first: BP1 and BP2 for the bot; AI1 to AI8 for the
bandit. GDD 19 describes a layered pilot AI with utility-based tactic selection; M1 asks only for a
state machine. GDD 19.12 requires the AI to fly through the same inputs as a player and the same
assist level, so it inherits the limiters.

## Decision

### The crate

`crates/ai-pilot` (lib `ai_pilot`, a name GDD 61.2 reserved) depends on `content`, `fcs`, `flight`
and `simmath` only (`check-layers`). It never touches the world: `sim-core` builds a `Percept` and
hands over the aircraft's state, and the brains answer with `PilotControls` and button presses
that `sim-core` turns into input frames, exactly like a player's. Random numbers arrive as inputs
(`Dice`: four uniforms and four normals a tick, drawn by `sim-core`). Every brain is `Copy`, owns
no heap memory and exposes `words()` for the state hash.

| Module | What |
|---|---|
| `geometry` | Range, closure, ATA, AA, the lead-collision point (B.5), the 19.4.1 classification |
| `maneuver` | The manoeuvre controller (`Goal` to sticks), the speed hold, direction helpers |
| `skill` | `SkillParams::from_def` (`n_cap = g_frac × n_max`) |
| `percept` | `Percept`, `OwnView`, `Contact`, `Dice` |
| `bandit` | `BanditBrain`: the state machine, `BrainKind` |
| `hold` | `HoldCourse`: straight and level at what it found (scripts HOLD, HOLD_FIRE; the HoldCourse brain) |
| `sortie` | `SortieTester`: the bot playtester |

### The manoeuvre controller (GDD 19.12)

A goal is a direction for the velocity, a load-factor cap and a throttle lever. The controller turns
at `ω = min(K_TURN θ, n_cap g0 / V)` towards the goal, so the lift must supply `a = V ω ê` and cancel
gravity's part across the path, `L = a − g⊥`; the load factor is `|L| / g0` within the cap and the
bank puts the lift along `L`. `fcs::tester::sticks::air_n` (new, the tester's `air` with the load
factor given) turns the load factor and the bank into the stick for the assist level, Manual
through the same incremental inversion the tester uses. Two details came from flying it:

- **`K_TURN` is 0.5 /s, not the plan's 1.5.** At 1.5 a 6° error asks for about 70° of bank at
  220 m/s, faster than Reduced and Manual can roll: the aircraft overshot and reversed for as long
  as it was flown (measured: still ±6° and rolling ±150° after 40 s). At 1.0 it settles in about 20 s,
  at 0.7 in about 10 s, at 0.5 without overshoot in every assist; large turns are unchanged (they are
  flown at the load cap either way). WP4 may tune it again against the gates.
- **Full** limits the bank and reads the stick as a flight-path rate, so the controller gives it the
  load factor whose vertical part, at the bank the aircraft has, is the one the goal needs. Without
  this a hard turn under Full climbed 2.7 km in 30 s.

Aiming a gun or a caged seeker is about the **nose**, not the velocity: those goals are the velocity
direction plus the error between the nose and the aim, so the angle of attack does not move the aim
off by several degrees.

### Skills (GDD 19.6)

`ai_skill/1` (cpack kind 6): Recruit, Regular and Veteran with GDD 19.6's perception delay, notice
probability, load fraction, gun aim error, defence delay, terrain clearance and commitment, plus
v0.1d's break-timing error, the Veteran's extend speeds and a repertoire bit set (pure, lead and lag
pursuit, break turn, jink, extend). Code finds a skill by its rank, never its id. **The Recruit never
fires** (`fires_gun: false`; owner decision D3, a deviation from GDD 19.6, where the Recruit fires).

### The bandit's state machine (M1)

States PATROL, INTERCEPT, PURSUE, GUN, DEFEND_GUN, DEFEND_MISSILE, EXTEND, RECOVER, RETURN, WRECK
(codes 0 to 9, the snapshot's `bandit_ai`). `decide` runs at 5 Hz, `act` every tick. The rules, in
priority order (a new state other than RECOVER, the two defences and WRECK waits for the skill's
commitment time):

1. WRECK is final (set by `sim-core`).
2. RECOVER below the skill's terrain clearance ahead or under 150 m above the ground; until clear by
   twice the clearance and climbing.
3. RETURN beyond the arena radius from home; until inside 0.8 of it.
4. DEFEND_MISSILE once a noticed missile has been in the percept for the defence delay. The break
   comes when the time to go falls below `3 s + g0 · break_timing_sigma` (`g0` drawn when the missile
   is first seen).
5. DEFEND_GUN when the player is seen within 2 500 m with its nose within 30° of the bandit and the
   bandit's aspect from it under 60°, for the defence delay.
6. EXTEND (Veteran) below 140 m/s, until 220 m/s.
7. GUN when seen within 1 200 m with the gunsight's aim point within `20 mrad × 1 200 / R` of the nose.
8. PURSUE when seen, or told within 8 km. 9. INTERCEPT when told. 10. PATROL.

Behaviours: PATROL orbits the CAP centre at its height (the point 30° ahead on the circle, CAS 180,
2 g); INTERCEPT flies to the lead point 300 m above the contact (afterburner beyond 15 km, 0.6 of the
skill's load); PURSUE is pure pursuit, lead pursuit (aim `t = R / (V + 300)`, at most 2 s ahead) or
lag pursuit (300 m behind, inside 600 m when closing faster than 50 m/s) by the repertoire; GUN puts
the nose on the aim point displaced by the skill's aim error (drawn every 0.5 s) and fires inside
1 000 m with the nose within 6 mrad of its aim, in bursts of 0.6 s with 0.4 s pauses; DEFEND_GUN
breaks across the line to the player at the skill's load with the afterburner, reversing every
1 to 1.5 s if it can jink; DEFEND_MISSILE turns the missile onto the beam (3 g), then at the break
pulls the skill's load across both the line of sight and the path, on the side its lift points, at
idle for 3 s; EXTEND unloads 10° below the horizon at 1 g with the afterburner; RECOVER climbs 20°
above the horizon wings level with the afterburner; RETURN flies home at the patrol height, CAS 220;
WRECK asks for Manual, rolls 0.6 and cuts the throttle.

**Fail-soft (AI6):** a non-finite number anywhere in the percept gives neutral sticks, the
throttle at military and RECOVER, in the same tick.

**Test brains** (the gate runners' knob, `BrainKind`): HoldCourse flies straight and level at its
start height, heading and CAS and never fires; DefendOnly does the same until it notices a missile,
then follows rules 2 and 4 only.

### The perception stub (GDD 19.9; built in `sim-core`, WP4)

The bandit knows the player by a GCI cue (position every 4 s with 300 m noise, once the player has
been airborne 10 s) and by eye within `R_vis = min(L_eff / 0.9 mrad, ln(C0/Cmin) · V_meteo / 3.912)`
with terrain line of sight, delayed by its perception delay. It notices a missile by eye (3 m of
plume while burning, 0.5 m coasting) with its notice probability per decision. The full
`PerceptionAudit` (AC-19.1) is M3.

### The bot playtester

`SortieTester` is an instrument, like the virtual tester it wraps (ADR-33): it reads the exact state
and the HUD's cues. The fcs `Tester` takes off; at its crosswind turn the bot climbs towards the
bandit (at most 10°, to 4 000 m, CAS 220); within 12 km it attacks: a missile press at `SHOOT` (at
most every 8 s); the nose on the bandit for the caged seeker from 600 m to 5 km when the aspect is
under 35° and missiles are left; lead pursuit beyond 1.5 km; inside it the pipper onto the bandit,
firing inside 900 m with the pipper within 5 m of it, in bursts of 0.5 s with 0.25 s pauses. After
the kill (or 480 s), it flies to a point 3 km up the extended downwind line (BF (−3 800, 1 200) at
the test airfield) at 600 m above the runway, then down the line at the tester's 300 m and
130 m/s, **opening the airbrake while more than 8 m/s fast** (at idle the clean Kestrel sheds speed
too slowly, and joining fast made the tester's final miss the centreline by 36 m), and hands the
aircraft back with `Tester::join_downwind` once abeam the crosswind corner with the heading within
30°, the height within 150 m and the speed within 20 m/s. The tester lands. A respawn starts it
again (the take-off on the runway; the attack or the climb in the air).

### Scenario constants (`sim-core`, `// DATA: ADR-47 (test scenario)`)

| Constant | Value |
|---|---|
| CAP centre (east, north) | (0, 15 000) m |
| CAP radius, height | 5 000 m, 4 500 m MSL |
| Bandit spawn TAS, patrol CAS | 200 m/s, 180 m/s |
| Activation | the player airborne 10 s |
| Arena radius | 40 000 m from the anchor |
| Wreck, respawn | at most 30 s; 20 s after it is gone |
| GCI | every 4 s, 300 m |
| Visual | `V_meteo` 40 000 m, `θ_min` 0.9 mrad |
| Arming | 1.0 s off the wheels |
| Gun ranging stub | 2 500 m, 30° |
| Target designation | 20 000 m |
| Kill credit window | 30 s (GDD 65.2) |
| Capacities | 192 rounds, 4 missiles, 8 explosions, 48 tracers, 32 events |
| Perception ring | 32 entries every 6 ticks |
| Decisions | every 12 ticks at phase 6 (5 Hz) |
| Predictor, seeker line of sight | every 30 and 6 ticks |

## Alternatives considered

- **Utility-based tactic selection (GDD 19.4.3).** The full design, but M1 asks for a state machine
  and the gates are easier to read against explicit rules. The rules' order is the utility order the
  GDD implies (survive, then defend, then attack).
- **The AI writing the flight model's commands directly.** Rejected by GDD 19.12: it must fly
  through the player's inputs and assist so it cannot do what a player cannot.
- **A bot that cheats on aiming (perfect pipper).** It reads exact state (it is an instrument) but
  aims through the real sight and controls, so BP1 measures the game, not the bot.

## Consequences

- `sim-core` (WP4) builds the percept, steps the brains in the `Ai` stage and turns their output into
  input frames; the bandit's slot is flown by `pilot::fly` like the player's.
- The gates (BP1, BP2, AI1 to AI8) run in `cargo xtask sortie` from WP4; tuning there is limited to
  this crate's ALGO constants (the plan's WP4 step 18).
- Lag pursuit, the jink and the extend are the cut list's item 5 if time runs short (a sim-pack
  change, before the re-bless only).

## Amendment 1 (2026-11-10): what WP4's gates asked of the bandit and the bot

`fg sortie` (ADR-45) flew the brains against the real core for the first time. What it showed, and
what changed (every value an ALGO constant of this crate; no sim datum except the Dart-1's, see
`content/core/weapons/README.md`):

**The bandit**
- **Speed guard (AI4).** A Recruit chasing the bot down from 6 km reached 1.18 `V_mo` in a 70° dive
  (RECOVER even lit the afterburner). From 0.92 `V_mo`, in every state it flies itself, the
  throttle closes, the airbrake opens and the path is held no steeper than 15° down, the floor
  rising to level at `V_mo`. The percept's `own` carries `vmo_mps` for it. Highest CAS since:
  0.945 `V_mo`.
- **Collision avoidance.** Head-on merges flown nose to nose passed within 4 m: 16 of 50 bot
  sorties under Full ended in a collision. When the contact (dead-reckoned from the percept to now)
  will pass within 60 m, either closing faster than 150 m/s inside 1 s or closing at all inside
  200 m, the bandit turns away from where it will pass at its load cap. Breaking at 2.5 s cost the
  bot nearly every kill (its head-on gun pass is fired inside about 0.6 s of the pass); 1 s keeps
  the pass.
- **Closing speed in PURSUE and GUN (AI8).** A Veteran closing at 130 m/s overshot every gun
  solution; Regular fired in 9 of 50 AI1 runs, Veteran in none. Inside 3 km the closure is held to
  `(R − 300 m) / 20 s`, between −30 and 150 m/s, at 0.02 throttle per m/s (never below military under
  200 m/s CAS), with the airbrake when 30 m/s too fast above 200 m/s CAS (below it a bandit slowed
  behind the landing bot stalled into the ground). GUN also feeds the line of sight's own rate
  forward (`ω_los × aim / K_TURN`), so a crossing target is tracked without the controller's lag.
  Regular and Veteran now fire in 50 of 50.
- **RECOVER (AI5).** A Recruit in a 79° dive from 2.2 km could not come level at its own load cap.
  Rule 2 now also fires when the height is below the pull-out arc at the skill's load plus 2.5 s of
  path (for the wings to come level) plus the skill's clearance; diving more than 10°, RECOVER pulls
  at the airframe's limit with the throttle closed and the airbrake open, whatever the skill. 0 ground
  impacts by a living bandit over the 824 bandits of the last run.

**The bot**
- **Ground guard.** Over the climb, the attack and the return: if the straight path 1 to 8 s ahead,
  or the aircraft now, is under 250 m above the ground (150 m on the way home, which comes down to
  the circuit's 300 m), it climbs at 20° in afterburner at 5 g or more.
- **Energy and g in the attack.** The load cap is at most 0.8 of what the clean wing gives at the
  speed (`(CAS / V_s)²`); under 150 m/s CAS it holds the path no higher than level in afterburner
  (Reduced and Manual stalled out of slow zooms); the pilot's g stress (now in `OwnInputs`) eases the
  cap from 7 g at stress 0.3 to 5 g at 0.6 (it pulled 6.9 g for 12 s and blacked out under
  `GEffects::Full`, the session BP1 flies).
- **The way home.** The lead-in point is 6 km up the extended downwind line; the bot flies there at
  140 m/s CAS, 2 g, descending at most 15°, to the circuit's height over the ground at the crosswind
  corner as a height above MSL (the ground under the line falls towards the airfield, and holding a
  height over it dived and gathered speed), but at least 300 m over the ground below; arriving away
  from that height within 3 km, it rounds the point (2 km radius, the way it is already turning)
  until the height is right; within 1.5 km and 150 m of it, it turns down the line and **hands over
  to the tester's downwind leg once its track is within 30°**, the tester settling the height and the
  speed over the 14 km to the base turn (with the airbrake when 8 m/s fast on the downwind and base
  legs). A goal more than 120° behind is turned onto level, 90° at a time: the controller flies a
  goal behind as a loop, and at the return's 2 g a slow loop climbed 2 km out of the circuit. The
  join's height and speed tolerances, and the go-around, are gone.
- The bot's own collision avoidance was tried and dropped: breaking early enough to clear the
  bandit cost the head-on shot. The bandit avoids instead.

**BP1 after this** (seeds 1 to 50, `rugged`, Recruit): Full 26, Standard 26, Reduced 38, Manual 36
(BP2, Standard: Regular 38, Veteran 41). Nearly every failure is now "no kill": the bot wins on its
head-on pass at the merge or not at all. After the merge it does not win the turning fight against a
Recruit, its seeker almost never sees the bandit's tail (it launches about 1 missile per 50
sorties), and under Full (60° of bank, 10°/s of flight-path rate) it cannot turn with the bandit at
all. The plan's step 18 asks the owner at this point.

**The owner's answer (2026-11-10).** D5: give the bot real dogfight logic (bot code only, no sim
data). D6: Full is informational for BP1 (ADR-45 amendment 1).

**The bot's aim after D5.** When the bot puts its nose (the caged seeker) or its pipper on the
bandit, it now feeds the line of sight's own rate forward (`ω_los × ŝ / K_TURN`), as the bandit's
GUN does. Without it the controller's turn gain left the nose lagging a crossing bandit, and the
second and third merges went by without a shot. BP1 after it (seeds 1 to 50, `rugged`, Recruit):
**Standard 42, Reduced 42, Manual 44** (gate: ≥ 40 each), Full 37 (informational); BP2: Regular
42, Veteran 43. AI1, AI2, AI4, AI5, AI8 and MS5 pass on the same build (AI5: 0 impacts over 855
bandits; highest CAS 0.943 `V_mo`). The margin is thin (Standard and Reduced by 2 runs); the runs
are deterministic, so it moves only when the code or the data does.

Tried and dropped: turning level towards a bandit behind instead of looping towards it (Reduced 41,
Regular 40, the rest unchanged), which also drew out fights long enough to empty the bandit's tanks
(below).

**Known limits** (left for later milestones):
- The bandit has no fuel logic (bingo is a v0.1d non-goal). After about 6.5 minutes in afterburner
  it is dry, glides and flies into the ground. No bot sortie fights that long; a human who drags
  out a fight can.
- The bot needs 234 to 369 s from the kill to the stop (median 250 s), so a kill after about 350 s
  ends "not stopped" at the 600 s limit (2 of 50 under Standard).
- Nearly every other failure is "no kill": after a missed merge the bandit gets behind the bot,
  and the bot does not win the rolling fight that follows within its 480 s of attack. The Recruit
  never fires, so the bot is never in danger from it.

## Amendment 2 (2026-11-11): the bandit's gun puts the pipper on the player

**The defect.** Rule 7 and the GUN behaviour above treat the gunsight's aim point as the place to
point the nose. The aim point is the director's pipper (ADR-46): the place the player must be for
rounds fired now to hit it, worked out along the bandit's present gun line. It moves with the
nose. So GUN steered the nose towards a point that its own nose carried along, and fired whenever
the nose was within 6 mrad of that point, which is nearly always. The player's position did not
enter the trigger at all. Found in the pre-merge walkthrough (plan section 13): a Regular sat in
GUN for minutes 150 to 1 100 m behind a player flying straight and fired all 150 rounds without a
hit, and still none with its aim error set to 0. Over 24 bot sorties against the Regular and the
Veteran it hit once. No gate measured the bandit's hits: AI1 measures position, AI8 firing.

**As built now** (the bot's own gun logic, which was right):

- **Rule 7:** GUN when seen within 1 200 m with the pipper within `20 mrad × 1 200 / R` of the
  player (the line of sight, extrapolated by the contact's age).
- **Steering:** GUN turns the velocity by the angle from the pipper to the player as the bandit
  sees it. That is the line of sight displaced by the skill's aim error, still drawn every 0.5 s.
  The line of sight's own rate is still fed forward.
- **Trigger:** it fires inside 1 000 m only with a gunsight solution and the seen player within
  6 mrad of the pipper. Bursts are 0.6 s with 0.4 s pauses, as before.
- **No solution:** the nose goes on the player.

No data changed. A Regular now shoots down a player flying straight at the merge in 33 s (28
rounds, 10 hits, seed `1f1e5eed00000001`). A new sim-core test,
`a_regular_behind_a_player_flying_straight_hits_it`, fails on the old code (150 rounds, no hit).

**The gates on the fixed build** (seeds 1 to 50, `rugged`): BP1 against the Recruit **Standard 44,
Reduced 41, Manual 43** (gate ≥ 40 each), Full 37 (informational). The Recruit never fires, but it
flies GUN too, so its manoeuvres changed. BP2: **Regular 32, Veteran 33**, with the bot shot down in
12 and 13 runs (before: 42 and 43, and no deaths). AI1 50/50 at every skill, AI2 (Regular 0.00 s,
Veteran 0.00 s, holding course 10.23 s), AI4 (highest CAS 0.954 `V_mo`), AI5 (0 impacts over 852
bandits), AI8 (Regular and Veteran fire in 50/50, the Recruit fires 0 rounds) and MS5 all pass. The
bot now launches a missile now and then: 6 in the 150 BP1 runs under Standard, Reduced and Manual.
The combat golden is re-blessed for this change, before merge 1, as the plan allows. The four
older goldens do not move: they fly no duel.

The known limit "the Recruit never fires, so the bot is never in danger from it" stands. Against a
Regular or a Veteran the bot is now in danger, as a player is.
