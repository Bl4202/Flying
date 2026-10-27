# ADR-36: Session config, ABI v3, input frame v3, snapshot v3 and the throttle hand-over

- **Status:** Accepted
- **Owner:** Bl4202
- **Date:** 2026-10-24
- **Affects:** GDD 6.3 (`session_config/1`), 48.2, 58.2 (`InputFrame`), 58.3, 61.2, 61.4.4 (snapshot, state hash); `crates/sim-core` (`layout`, `input`, `session`, `sim`, `ownship`, `spawn`, `pilot`, `systems`), `crates/sim-wasm` (`fg_configure`), `crates/xtask` (`wasm`, `determinism`), `packages/sim-bridge`

## Context

Until v0.1b the core started one way (trimmed in the air, the stick moving the surfaces), the input
frame carried the stick, the throttle and eleven buttons, and the snapshot told the renderer where
the aircraft was and how it was moving. v0.1b adds a runway start, four assist levels, levers (gear,
flaps, brakes), the pilot's body and a flight control system that must know what the player asked
for. All of it has to be deterministic and replayed, so it travels in the input frame, and the
host has to hand its throttle lever over at every spawn without a jump.

## Decision

### Session configuration (ABI v3)

`fg_configure(spawn, assist, g_effects) -> config_status` (GDD 6.3), accepted only while the core
is idle. `spawn` is runway (0, the default) or air (1); `assist` is Full 1, Standard 2 (the
default), Reduced 3 or Manual 4; `g_effects` is Off 0, Reduced 1 or Full 2 (the default). Bad
codes are refused (`INVALID`) and change nothing, a started core refuses (`NOT_IDLE`). The session
is part of the aircraft's hashed state (the `ownship` hash, not the tick hash), so two cores with
different sessions never compare equal; a core without aircraft content (the spectator) ignores
it and hashes as it always did. The session picks where the aircraft starts and where it
respawns after a crash; the assist level is where the control system starts, and the player's own
choice (below) survives respawns.

### Changes during the sortie travel in the input frame

The frame stays 96 bytes (`input_frame`). New fields in the reserved bytes:

| Offset | Field | Meaning |
|---|---|---|
| 74 | `assist_req` | 0 keep the level, 1 to 4 an absolute level (anything else is "keep") |
| 75 | `spawn_ack` | the snapshot's `spawn_seq` the host has synchronised its lever with |
| 76 | `spawn_req` | read on the RESET press: 0 the session's spawn, 1 runway, 2 air |
| 80 | `t_sent_ms` | when the frame was written (diagnostic; never hashed) |

`t_ms` is now the time of the source event (the key press, the pointer move, the gamepad sample)
on the shared clock, so the control latency of AC-58.3 can be measured; it is never hashed either.
Flags: NEUTRAL 1, **CHANGED 2** (a control changed; latency is measured on these frames),
**MOUSE_AIM 4** (the cursor axes carry a direction: `CURSOR_X = azimuth / pi`, `CURSOR_Y =
elevation / (pi / 2)`), **AUTO_GCAS 8** (reserved; the feature is off). Buttons: RESET, GEAR, FLAPS
DOWN / UP / CYCLE, PARKING BRAKE and TRIM RESET act on the press; the trim, AIRBRAKE and RECOVER
buttons act while held (the host turns a toggle binding into a held state). The core asks for a
level once: `assist_req` equal to the level in force does nothing, a different one is a bumpless
switch (ADR-34) counted in `assist_changes`. The erratic script's frames are byte-identical to
v0.1a (a vector test), so the only change to their hash is what the aircraft does with them.

### The levers belong to the core

The gear lever (a toggle; **raising it is refused while the aircraft is on its wheels**), the flap
lever (up, take-off at half travel, landing at full; down and up step, cycle goes round), the
parking brake (a toggle; it sets both wheel brakes) and the pitch trim reset are state in the
aircraft's slot and are hashed. The lever is what the pilot wants; the gear and the flaps are
where the actuators are (the gear takes 6 s). The airbrake is open while the button is held.

### Spawn and the throttle hand-over

Spawning is `spawn::spawn(kind)`.

- **Runway** (GDD 48.2 "runway hot"): 150 m past the threshold on the centreline, heading 000, 60 %
  fuel, at rest on the struts (the static pose of ADR-35: pitch about 3 degrees, no creep), engine
  at steady idle, parking brake on, gear down, take-off flaps, the control system in ground mode.
  The contacts start loaded, so the first tick is no touchdown.
- **Air**: trimmed level at 200 m/s 2 000 m above the threshold, gear and flaps up, flight law.
  A model without gear, or a pose that cannot be solved, falls back to the air spawn (the latter
  counts as a trim failure).

Every spawn increments `spawn_seq` (a wrapping byte, in the snapshot). The core **holds the
throttle** at the spawn setting (idle on the runway, the trim setting in the air) until an input
frame carries `spawn_ack == spawn_seq`. The host sees `spawn_seq` change, moves its lever to the
snapshot's throttle, and acknowledges from then on; absolute levers (HOTAS) take over on the
acknowledgement. This is deterministic (the acknowledgement is in the frame and replayed), there is
no jump, and nothing depends on timing. A scripted pilot has no lever: the core acknowledges every
spawn on its behalf. THROTTLE_HELD in the snapshot says the hold is on.

### The pilot's body in the tick

After the buttons and the assist request, the controls are built from the frame and the levers,
then filtered by the pilot's physiology as of the previous tick (ADR-38): a blackout delays the
stick and pedals by up to 18 ticks, G-LOC zeroes them and the ramp back scales them. The delay
reads a ring of the pilot's last stick positions (pitch, roll, yaw; the throttle and levers are
not delayed). Mouse-aim is dropped while the pilot is out. The control system then runs once, the
flight model ticks twice, and the physiology and the structure take the load factor of the tick.
What the ground did (touchdowns, hard landings, destruction) waits in the slot for the contact
stage, which counts it and starts the 2 s respawn countdown.

### Snapshot v3

The ownship block is 224 bytes (64 to 287), so the bodies start at 288 and the snapshot is 10 528
bytes (`snapshot_sab::SLOT_BYTES` follows); offsets 64 to 185 are as in v2. New: the assist level,
the control system's mode, `flags2` (weight on wheels, parking brake, the three limiters,
overstress, blackout, G-LOC, red-out, a hard landing latched for 3 s, gear and flap overspeed,
departed, the gear warning, mouse-aim in use, throttle held), `spawn_seq`, the flap detent, the
gear, flap and airbrake positions, the brakes as commanded (the parking brake included), the
steering angle, ground speed, vertical speed, the height of the lowest wheel, the pilot's lagged g,
stress and red-out, the load-factor command, the last touchdown's sink rate, the aim in use, the
crash cause and the gear bits (per leg weight-on-wheels and collapsed, lever down, all down and
locked). Two readings worth stating:

- `g_stress` and `redout` are **what the picture shows**: zero with the dial Off (the state still
  advances, so turning the dial on mid-sortie, were it offered, would not start from rest).
  `g_lagged` is always the state.
- `agl_m` is the height of the lowest wheel or contact point at **full extension**: at rest on
  the struts it reads -0.12 m, not zero. HUD code that wants a radar altitude clamps it.

The gear warning (low, slow, throttle back, gear up) is `// ALGO:` in `ownship.rs`: below 150 m,
below 100 m/s calibrated, lever under half.

### Profile block

13 counters (SIZE 160): touchdowns, hard landings, G-LOC events and assist changes join the nine
of v0.1a.

## Alternatives considered

- **The assist level as an ABI call.** A call would not be in the replay. An absolute byte in the
  frame is deterministic, and "keep" is 0 so a host that never writes it changes nothing.
- **A relative assist command (next, previous).** Lost frames would desynchronise the host's idea
  of the level from the core's. Absolute is idempotent.
- **No hand-over: jump the lever to the host's value.** Respawning with a HOTAS at full
  afterburner would launch the aircraft from the runway threshold at full power.
- **A timed hand-over (the core holds for N ticks).** Depends on how fast the host reacts; the
  acknowledgement does not.
- **Delaying the whole control set in a blackout.** The throttle and levers are not what a
  blackout delays (a hand already on the throttle stays there), and a ring of whole control sets
  is several times the state to hash.
- **The lever state in the host.** Lost on a reload of the worker, not replayed, and the core
  needs it for its own interlock (gear up refused on the wheels).

## Consequences

- The determinism goldens changed once, in the dedicated commit of WP9 (`5506df6`): the erratic
  and flight60 suites now fly under their sessions (air, Standard, g-effects on; air, Manual, g-effects
  off) through the control system and the gear model, and a third suite, the circuit (runway,
  Standard, g-effects on), was added. The `tick_h`, `rng` and `bodies` columns of the erratic suite
  were proven unchanged from v0.1a before blessing.
- Every reader of the snapshot or the frame follows the layout: `cargo xtask gen-abi` regenerates
  `layout.gen.ts` and the vectors.
- A host that does not acknowledge a spawn keeps the aircraft at idle (runway) or the trim
  throttle (air), visibly (THROTTLE_HELD). The pump (ADR-37) acknowledges.
- Scripts need no change and acknowledge themselves.
