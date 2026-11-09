# The controls

How the player's hands become input frames: the action catalogue, the devices, the `controls/1`
profile, the pump, the binding editor and the latency measurement. The decisions are in
[ADR-37](../adr/0037-controls-stack.md) (the stack) and [ADR-36](../adr/0036-session-config-abi-v3-input-snapshot.md)
(the frame the pump writes); the requirements are GDD 58. What the sim does with the frame is in
[fcs.md](fcs.md) and [runtime.md](runtime.md).

## Where the code is

`packages/input/src/` (it may not import `render` or `sim-host`):

| Module | What it does |
|---|---|
| `actions.ts` | The catalogue: 24 actions, each with a kind, a context and a group |
| `sources.ts` | What a binding can name: a key, a mouse button, the wheel, a pad button or axis, a hat direction |
| `profile.ts` | The `controls/1` type, `parseControlsProfile` (errors as JSON pointers), `serializeProfile` (canonical), the three built-ins, `chooseProfile` |
| `profile-edit.ts` | The pure edits the editor makes to a profile |
| `binding-map.ts` | Compiles a profile into flat typed arrays for the pump |
| `shaping.ts` | The axis pipeline: dead zone, outer dead zone, expo or power curve, gain, smoothing, rate limit, invert, offset |
| `conflicts.ts` | The same control on two actions, a whole axis over a half, keys the browser keeps |
| `capture.ts`, `calibrate.ts` | "Press the control" and the centre-and-travel calibration, with ghost-axis rejection |
| `keyboard.ts`, `dom.ts` | Key state with the ramp emulation; the event listeners and the two clocks that sample devices between events (each animation frame, every 4 ms); a `swallow` hook lets the page keep the overlay chord for itself |
| `devices/gamepad.ts` | `GamepadHub`: every pad, standard or raw, hat decoding of both encodings, hot-plug, identity by class |
| `devices/mouse.ts` | Pointer Lock with unadjusted movement, the fallback warning, accumulated motion and wheel |
| `mouse-aim.ts`, `mouse-stick.ts` | The world-frame aim (35° cone around the nose) and the virtual stick |
| `throttle.ts` | The lever: absolute or incremental, the soft stop at military power, the afterburner toggle |
| `pump.ts` | `InputPump`: actions to input frame v3 |

`packages/ui/src/` holds the parts the player sees: `settings.ts` (`settings/1`), `controls-store.ts`
(profiles in the key-value store), `pause-menu.ts`, `editor-model.ts` (DOM-free) and
`binding-editor.ts`, `flight-warnings.ts` (the strip), and the overlay's `ctl` row. The page wiring
is `packages/app-shell/src/{controls,menus,status-feed,demo}.ts`.

## Defaults

Keyboard and mouse (`default-km`; the pad and joystick profiles carry the same keys, minus the
Page keys):

| Action | Keys |
|---|---|
| Pitch | `W` pushes the nose down, `S` pulls up |
| Roll, yaw | `A`/`D`, `Q`/`E` |
| Throttle | `Shift` or `Page Up` up, `Ctrl` or `Page Down` down, the wheel; `Z` the afterburner |
| Trim | `]` nose up and `[` nose down while held, `\` reset (Manual assist only moves the elevator) |
| Gear, flaps | `L` gear lever (up is refused on the wheels), `V` flaps up, take-off, landing, up |
| Brakes | `B` wheel brakes while held, `P` parking brake, `N` airbrake (toggle) |
| Assist | `U` recover (held), `F8` next assist level |
| Mouse | `O` aim, then stick, then off; Pointer Lock on a click |
| System | `R` respawn, `Esc` pause menu, `Ctrl+Shift+D` the debug overlay |

Gamepad: right stick roll and pitch, triggers yaw, left stick Y the throttle, left stick click the
afterburner (held), the d-pad trim (up and down) and flaps (left and right), Y gear, X brakes, B
airbrake, A parking brake, left bumper recover, Back respawn, Start pause. Joystick or HOTAS: axes
0 and 1 the stick, axis 2 an absolute throttle lever, axis 3 or 5 yaw, the hat trim and flaps,
buttons 0 to 3 brakes, airbrake, gear and flaps. **Both pad profiles were tested with stubbed
Gamepad API objects only**; no hardware was available (the exit record says so). The joystick
profile is a guess at the usual layout, and the editor is how a real device gets fixed.

## The pump

`InputPump.update()` runs from animation frames, a 4 ms timer and device events (`poke`). It
writes a 96-byte frame (ADR-36) into one buffer and hands it to the sink, which is the shared ring
or a message.

- `t_ms` is the **earliest unsent source event** (`event.timeStamp` mapped to the shared clock);
  `t_sent_ms` is the write time. `CHANGED` marks a frame that carries a new pilot input; heartbeats
  (every 8 ms through the ring, 33 ms by message) do not. A key press applies at least a
  millisecond of ramp so that the frame at the event shows a change.
- The pump keeps the player's standing requests: the **assist level** (absolute), the **spawn** for
  the next respawn, and `spawn_ack`. When `spawn_seq` changes in the snapshot it sets the throttle
  lever to the aircraft's throttle and acknowledges, which releases the throttle the core held.
  `newSortie()` forgets the last sortie's acknowledgement when Fly starts a fresh core (which numbers
  its spawns from 1 again); the pump also adopts the idle core's `spawn_seq` of 0 between sorties, so the
  reset is the second guard and the end-to-end second-Fly test cannot tell it from the first.
- The `menu` context centres the sticks, holds the throttle and ignores the flight buttons; pause
  still works. A pad that goes away reads neutral at once (the throttle holds) and the profile
  re-binds within a quarter of a second of its return (AC-58.6).
- It makes no objects, arrays, strings or closures per update. V8 still boxes some fractional
  numbers, so the bound is measured (`pump.garbage.test.ts`: nothing at rest with whole-number
  clocks, under 1 024 bytes an update with everything in play on fractional clocks).

A **scripted pilot** (`?script=circuit`, the erratic script of the perf runs) has no pump: the sim
takes the script's frame and then the host's, the last winning, so neutral heartbeats would
override it.

## The view (v0.1c, ADR-41)

Where the pilot looks and what the HUD shows are the player's, not the aircraft's, so **none of the view
actions reach the input frame**: the sim does not know where the pilot looks, and determinism does not
depend on it. The pump fills a `ViewInput` each update (look rates, running totals of mouse look, held zoom and
free look, the camera and recentre requests, HUD declutter and brightness steps) without allocating, because
`onAction` fires on presses only; `view-controller.ts` keeps the state (the camera, the declutter level) and
the app shell writes it to the 64-byte view channel the render worker reads
([runtime.md](runtime.md)).

| Action | Default | Does |
|---|---|---|
| `flight.camera_cycle` | `C` | cockpit and chase, in turn (a gamepad's right stick press does the same) |
| `flight.free_look` | `X`, held | the mouse moves the head, or orbits the chase camera |
| `flight.zoom` | right mouse button, held | the cockpit's field of view narrows to 30 degrees |
| `flight.look_up`, `look_down` | `ArrowUp`, `ArrowDown`, held | the head turns up and down (90 degrees a second) |
| `flight.look_left`, `look_right` | `,` and `.`, held | the head turns left and right |
| `flight.look_recentre` | `/` | the head returns to the front |
| `flight.hud_declutter` | `I` | the HUD's three detail levels in turn |
| `flight.hud_brightness_down`, `up` | `-` and `=` | the HUD's brightness |

With the mouse mode off the mouse looks round; the right mouse button works without the pointer lock (the left
click that asks for the lock is still not a control). These keys avoid `CTRL_SHORTCUTS` and the function
keys. GDD 58.5's `C` for free look and `1` to `4` for the cameras change: Ctrl is throttle-down, and Ctrl and a
digit switches browser tabs and cannot be blocked (a GDD clarification, 0.2.3).

**Existing profiles.** The action catalogue is versioned (`ACTION_CATALOGUE_VERSION`, stored as `catalogue`
in a profile; none means 1, the v0.1b catalogue). `migrateProfile` gives a stored custom profile the default
bindings of the actions added since its version where those inputs are free, and leaves an action unbound,
with a note in the editor, where they are not. A profile that already binds `C` to something keeps it.

## Settings and profiles in the browser

| Key (key-value store) | Contents |
|---|---|
| `settings/2` | v1's fields (assist, g-effects, effect strength, respawn spawn, mouse mode, sensitivity, spring, ground-collision avoidance) and, new in v0.1c, the arena, camera, field of view, shake, reduced motion, the HUD (palette, brightness, declutter, overlay scale), video (renderer, preset, resolution scale) and audio (the four buses and mute-when-hidden); `migrateSettings` reads `settings/1` and keeps every field (AC-53.6); export and import are `flying-settings.json`, byte for byte |
| `controls/profile/<id>`, an index, `controls/active` | the player's profiles in canonical JSON |

`?spawn=`, `?assist=` and `?g=` override the stored settings for one visit; the menu shows the
override until the player changes that setting. A profile that no longer parses is reported and left
alone. A built-in profile is not edited: the first change saves a copy. Export downloads the
canonical JSON; import reports each problem by JSON pointer, and export, import, export gives the
same bytes (AC-58.5).

## Latency (AC-58.3)

Defined in ADR-33 section 7: from the source event to the end of the sim tick that acted on it,
measured only on `CHANGED` frames, display latency excluded. The sim worker records, for each such
frame it drains, the **control** time (event to end of tick), the **poll** time (event to write)
and the **tick** time (write to end of tick), in histograms that reset every report interval and,
separately, since `resetRun`. The overlay's `ctl` row shows the run-scoped ones.

| Where | Gate |
|---|---|
| `tests/e2e/latency.spec.ts` (CI, headless) | at least 200 changes, p95 at most 33.3 ms (two ticks) |
| `LATENCY_STRICT=1`, or `pnpm perf:local --latency 500` on the reference machine, headed Chrome | at least 500 changes, p95 at most 25 ms, with the display rate recorded |

Headless CI measured p95 between 16.5 and 21 ms. The reference run is in
`docs/exit/v0.1b/machines/`.

## Checks that guard it

| Check | Catches |
|---|---|
| `pnpm exec vitest run packages/input packages/ui` | AC-58.2 (the 200-case shaping table and the 63 % step response), AC-58.5 (conflicts over every duplicate class, the byte-identical round trip, a stubbed unknown device bound through capture), hat decoding, hot-plug, `CHANGED` and `t_ms`, the throttle detent, the mouse-aim cone and encoding |
| `tests/e2e/{input,bindings,gamepad,mouse,assist,ground}.spec.ts` | The same through the real page: rebinding and reload, a stubbed pad and HOTAS, unplug and replug, Pointer Lock stubs, assist chips, the runway start |
| `crates/fcs/tests/mouse_aim.rs` | AC-58.4 (scaled): the sim's side of the mouse |
| `dependency-cruiser` | `input` importing `render` or `sim-host` |
