# ADR-37: The controls stack: actions, devices, `controls/1` profiles, the pump and the binding editor

- **Status:** Proposed
- **Owner:** Bl4202
- **Date:** 2026-10-24
- **Affects:** GDD 58.1 to 58.6, 53.4, 59 (settings); `packages/input`, `packages/ui` (settings, controls store, pause menu, binding editor, warning strip, overlay rows, game flow), `packages/sim-bridge` (`InputFrameWriter`, `AircraftStatus`), `packages/app-shell` (wiring)

## Context

Until v0.1b the keys and the standard-mapping gamepad were fixed in code: eleven actions, one pad,
no way to rebind, no raw joysticks, no mouse. The milestone adds a runway start, four assist
levels, gear, flaps, brakes, a mouse that flies (aim and stick), HOTAS levers and a binding editor
(GDD 58.5), and it measures the delay from the player's hand to the control surface (AC-58.3,
ADR-33). The controls had to become data a player can change and share, and the code that turns
them into input frames had to be fast, steady and honest about when things happened.

## Decision

### Actions and sources

An **action** is something the game understands (24 of them in `actions.ts`): the three stick
axes, the throttle (an absolute lever, or up and down as rates), afterburner and airbrake
(toggles), trim, gear, flaps, parking and wheel brakes, recover, the mouse mode, the next assist
level, respawn and pause. Each has a kind (axis, button, hold, toggle, rate), a range for axes, a
context (flight, or menu for pause, which works in both) and a group the editor lists it under.
`ActionId` is the union of the ids, so a mistyped id is a compile error.

A **source** is one physical control: a key (by `KeyboardEvent.code`), a mouse button, the wheel,
a pad button, a pad axis (whole, or one half), or a hat direction. Devices are named **by class**,
not by the slot the browser gave them: `pad` is any pad with the `standard` mapping,
`joy:vendor:product` a raw device (read from the id string, Chrome and Firefox forms), `joy` any
raw device. A binding to a device that is not connected reads as neutral, and when a device
appears the bindings find it again (AC-58.6).

### `controls/1` profiles

A profile is JSON: id, name, device class, up to four bindings per action (`action`, `source`,
and where they matter `direction`, `invert` and `threshold`), a shape per axis or rate action
(dead zone, outer dead zone, expo or power curve, gain, smoothing, rate limit, invert, offset:
the pipeline of GDD 58.2), a calibration per physical axis, the keyboard ramp rates (3 and 4 per
second), the throttle rates and the toggle modes. `parseControlsProfile` checks all of it and
reports every problem with a **JSON pointer**; unknown keys are errors. `serializeProfile` writes
a canonical form, so export, import and export again give the same bytes. Three profiles are
built in: keyboard and mouse, gamepad, joystick or HOTAS (each pad profile carries the keyboard
bindings too). The built-ins cannot be edited; the first change a player makes is saved to a copy.
`chooseProfile` picks the player's choice if the devices it binds are connected, otherwise the
built-in for what is connected.

### The pump

`InputPump` reads the devices through the compiled profile (`binding-map.ts`: flat typed arrays,
grouped by action) and writes input frame v3 (ADR-36). What the frame says about time:

- `t_ms` is the time of the **earliest unsent source event**: a key press, a mouse event, a pad
  sample (its own `timestamp` if recent). Devices tell the pump with `poke(event.timeStamp)`.
  `t_sent_ms` is when the frame is written.
- `CHANGED` marks a frame carrying a new pilot input: an axis moved by more than 1e-5, or a button
  or flag changed. Heartbeats (every 8 ms through the shared ring, 33 ms by message) are not
  CHANGED. A key press applies at least a millisecond of ramp so that the frame at the event shows
  a change. An update that changes nothing drops the event, so it cannot date a later change.
- A frame the channel refuses leaves its change, and its event time, for the next attempt.

The pump also carries the player's standing requests: the assist level (absolute, so it survives
respawns), the spawn for the next respawn, and `spawn_ack`: on a new spawn it sets the throttle
lever to the aircraft's throttle (`observe`) and acknowledges, which releases the throttle the
core held for it. An absolute lever (HOTAS) takes over at that point, then follows its movement;
the keys and wheel still work when it has not moved. The lever is soft at military power (0.8): a
held key stops there for 300 ms before running on into the afterburner; a wheel notch lands on it.

**Garbage.** The pump runs every few milliseconds all flight, so it makes no objects, arrays,
strings or closures per update: its state is in typed arrays (V8 allocates for every store of a
fractional number into an object field), results go into arrays rather than return values, and
the frame is written into one buffer (`InputFrameWriter`). It does not make *no* garbage: V8 boxes
a fractional number that crosses a call it does not inline (16 bytes, gone at the next young
collection), and a browser clock read boxes its result. `pump.garbage.test.ts` measures it: nothing
at rest with whole-number clocks, under 512 bytes an update with a pad, the mouse aim and four keys
in play on fractional clocks (about 300 measured).

### Mouse

**Mouse-aim runs in the simulator** (ADR-34). The page keeps a world-frame aim direction (azimuth
clockwise from north, elevation up), moved by mouse motion, limited to a 35 degree cone around the
latest nose, and sends it as `CURSOR_X = az / pi`, `CURSOR_Y = el / (pi / 2)` with the MOUSE_AIM
flag; the core turns the error into stick. **Mouse-stick** stays on the page: a virtual stick with
a sensitivity (0 to 3 times) and a spring (0 to 1). The mouse is read under Pointer Lock with
unadjusted movement where the browser allows it, falling back to the ordinary lock with one
warning; motion and wheel notches are accumulated and handed over once per update, never written
frame by frame.

### Settings, profiles in the browser, the pause menu

Settings (`settings/1`: assist, g-effects, effect strength, respawn spawn, mouse mode, sensitivity,
spring, ground-collision avoidance) live in the key-value store, read forgivingly: a missing or
out-of-range field takes its default. The profiles are `controls/profile/<id>` (canonical JSON), an
index (the store cannot list keys) and `controls/active`. A profile that no longer parses is
reported and left alone.

The **pause menu** (Esc, or losing the pointer lock) holds the assist chips with a preview of what
each level does, where a respawn goes, the mouse mode and tuning, the g-effects and their
strength, Controls and Respawn. Opening it **holds the simulation**; the pump goes to its menu
context (sticks centred, throttle held, flight buttons ignored, pause still heard). The DOM exists
only while the menu is open. Automatic ground-collision avoidance is not in v0.1b (cut list); its
setting and flag are kept and the menu leaves the option out.

### The binding editor, v1

A native-control dialog over a DOM-free `EditorModel`: choose, create, duplicate, rename, delete,
export and import profiles; every action with its chips and "Add" (press the control; Esc cancels;
a bipolar axis has Add - and Add +); remove and restore (one action, a group, everything);
conflicts (the same control on two actions, a duplicate, a whole axis over a half) and keys the
browser keeps with Ctrl marked in words as well as a shape; per-axis dead zone, expo or curve
power, gain, smoothing and invert; the connected devices with a live view of axes, buttons and
hats; and a calibration (centre, then full travel; ghost axes are skipped) for each. Not in v1:
chords, shift layers, per-aircraft overrides, the curve preview canvas.

### The warning strip and the overlay

One element shows the assist name and, behind a shape glyph (a triangle for a warning, a diamond
for a caution), STALL, PULL UP, DEPARTED, OVERSTRESS, α-LIM, G-LIM, OVERSPEED, GEAR SPEED, FLAP
SPEED and GEAR. PULL UP is a time-to-impact rule (under 5 s at the present sink, below 400 m),
silent on the wheels and in the landing configuration. The debug overlay gains `assist`, `ground`,
`g` and `ctl` rows (control latency split into control, poll and tick, and the old input age);
related rows were merged (build and ABI, adapter and device, TAS and Mach, throttle and thrust,
dropped and queues, frame and render, snapshot age and frames) so the page stays within 150 DOM
nodes (AC-53.4: 147 measured with 44 rows; 40 rows and the strip is about 141).

## Alternatives considered

- **Bindings in code, edited by recompiling.** No player can remap; no imports.
- **Devices by browser index.** A joystick moved to another USB port loses its bindings.
- **Writing a frame on every pointer move.** The input ring takes eight frames a tick; a fast
  mouse would flood it. Accumulate and write on the next update.
- **Mouse-aim on the page.** AC-58.4 could not then run headless and deterministically.
- **Strict zero allocation in the pump.** Possible only by passing every fractional number through
  typed arrays at every call, and still subject to the browser's clock reads. The measured bound
  guards the part that matters (no per-update objects) without contorting the code.
- **The menu leaving the simulation running.** The assist switch is bumpless either way; holding
  the simulation is what a pause menu is, and it keeps the aircraft from flying itself while the
  player reads.

## Consequences

- Gamepad and HOTAS are tested only with stubbed Gamepad API objects (no hardware was available
  for v0.1b); the exit record says so. The generic joystick profile is a guess at the usual axis
  layout, which the editor fixes.
- `app-shell/demo.ts` is wired in the wiring work package: settings and profile stores, the pause
  menu and editor, the warning strip, the spawn hand-over, the snapshot reader on the main thread.
- The e2e specs that read overlay rows by id (`ov-thrust`, `ov-latency`, ...) follow the merged
  rows there.
