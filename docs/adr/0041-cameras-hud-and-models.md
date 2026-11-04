# ADR-41: Cameras, view input, the HUD and the procedural aircraft

- **Status:** Proposed
- **Owner:** Bl4202
- **Date:** 2026-11-03
- **Affects:** ADR-20 (amended), ADR-37; GDD 44.8, 54.2, 54.3, 54.10, 54.11, 57.3, 58.5;
  `packages/render`, `packages/input`, `packages/sim-bridge`, `packages/ui`, `packages/app-shell`

This ADR is written in parts as the work packages land. This version holds the camera and view-input
part (WP4) and the exterior-model and cockpit part (WP5); the HUD part is added with WP6.

## Context

v0.1c adds a cockpit camera with head look and zoom and a chase camera behind a procedural Kestrel,
both of which must keep clear of the ground (AC-54.6) and respect reduced motion (AC-57.3). The
player's view is not part of the simulation: it must never reach the sim, change an input frame, or
move a hash. The input stack (ADR-37) has no notion of a view action yet, and its default keys
already use most of the Ctrl-chord letters and digits the browser keeps for itself.

## Decision

### The camera rig

`CameraRig` (`render/src/camera-rig.ts`) lives on the render side, takes the aircraft's state from
the snapshot and the player's view (below) and gives an eye position, an attitude, a field of view and
a near plane every frame. The step makes no objects: its state is in typed arrays, and a test counts
the garbage (a handful of boxed numbers, no arrays or objects).

**Cockpit.** The eye is the cockpit data's design eye (`eye_point_m`, BODY FRD from the reference
point), moved by the load the pilot feels (4 mm per g, at most 2 cm, none with reduced motion). The
head turns within the limits in the data (default 135 degrees of yaw, 60 up and 60 down) through a
critically damped spring whose step response reaches 63 per cent in 60 ms (a test measures it).
Look keys turn it at 90 degrees a second, the mouse turns it while free look is held (or when the
mouse mode is off), `/` centres it. The vertical field of view is a setting (50 to 120 degrees,
default 75); holding zoom narrows it to 30 degrees. Near plane 0.3 m.

**Chase.** The camera follows the aircraft's velocity direction (its nose below 20 m/s, where the
velocity means nothing, blended between), 45 m behind (40 to 150, a setting) and 20 per cent of that
above (a design choice; GDD 54.11's 25 per cent is the third-person camera's). It follows with a
first-order lag of 0.8 s (the range is 0.6 to 1.2), looks at the aircraft with the horizon level, and
widens the field of view by 8 degrees towards the aircraft's top speed (none with reduced motion).
Free look orbits it. It snaps when the aircraft jumps (a respawn). Near plane 0.5 m: GDD 44.8's 5 m
would clip the ground when the camera is clamped 1.5 m above it, as v0.1b's value already did
(a clarification).

**Terrain clearance (AC-54.6).** Both cameras keep the eye at least 1.5 m above the terrain. The
chase camera's arm (the segment from the aircraft to the camera) is marched against the terrain every
4 m and the camera is pulled in to just before the first sample that is under the ground (not closer
than 6 m), then lifted to the clearance. The terrain comes from the levels the renderer already
holds, on the very triangles it draws (`TerrainSet.heightAt`). The gate is 1 000 seeded low-level
paths over the committed height grid of the rugged arena (`tests/vectors/rugged_heights.bin`): neither
camera below terrain + 1.5 m, and the arm never under the surface at half-metre spacing.

**Shake** from buffet (from 3 degrees below the warning angle to the warning angle), ground roll and
the afterburner is scaled by the player's setting (0 to 100 per cent). The amplitude is the size of
the whole deviation, at most 1.5 degrees, and 0.5 degrees with reduced motion (AC-57.3).

### View input

View actions are `flight.camera_cycle`, `free_look`, `zoom`, `look_left/right/up/down`,
`look_recentre`, `hud_declutter` and `hud_brightness_up/down` (catalogue version 2). They **never
enter the input frame**: the pump reports them as `ViewInput`, a set of numbers in a typed array
(look rates, running totals of mouse look, zoom and free look while held, and counters of presses),
because `onAction` fires only on presses and a counter survives a dropped frame. A `ViewController`
turns that and the player's settings into a `ViewState`: the camera cycles on each press, the HUD
declutter steps through three levels, brightness moves in tens between 20 and 100.

`ViewState` travels from the main thread to the Render Worker in the **view channel**
(`sim-bridge/view-channel.ts`): one 64-byte record in a `SharedArrayBuffer`, updated under a sequence
lock and read without blocking, or the same state in a `view` message when the page is not isolated.
The layout lives in `sim-bridge` because `input` and `render` share a tier and cannot import each
other.

**Default keys**, all outside the browser's Ctrl chords and the function keys the profiles already
avoid: `C` camera, `X` hold for free look, right mouse button hold for zoom, `ArrowUp`/`ArrowDown`
look up and down, `,` and `.` look left and right, `/` centre, `I` HUD declutter, `-` and `=` HUD
dimmer and brighter, and the pad's right stick press for the camera. GDD 58.5's `C` for free look
and the digit keys for cameras change: Ctrl is the throttle, and Ctrl+digit switches the browser's
tabs and cannot be blocked (a clarification).

**Existing profiles.** A profile records the catalogue version it was made for (`catalogue`; none
means 1). `migrateProfile` gives the actions added since their default bindings, taken from the
built-in profile of the same device class, except where that control is already bound in the profile,
and never beyond four bindings an action. An action whose every default input is taken stays unbound
and is listed, so the editor can say so (`ControlsStore.migrationNotes`). The profile then records the
new version, so the migration happens once and a binding the player removes later does not come back.
The store migrates on load (and stores the result) and on import.

### The procedural aircraft and cockpit

The Kestrel's exterior and cockpit are generated in code from presentation data (`model/1` and
`cockpit_layout/1`, ADR-43), original (ADR-11), in at most 8 000 triangles (the Kestrel needs 812).
`buildModelMesh` makes one mesh in body axes from rings at stations (a 16-sided fuselage, a 12-sided
canopy), trapezoid slabs (wings, tail, fin, the movable surfaces, each with its twin if mirrored),
boxes (intakes, plates), a short cone for the nozzle and a strut and a 12-sided wheel for each gear
leg. The baker counts triangles by the same rules (`present_check.rs`), and a test checks that the
builder's count is the baker's. Every primitive is wound against an outward hint, and tests check that
every triangle's normal agrees with its vertices', that each solid is closed (every edge shared an
even number of times; the one deliberate opening is the nozzle's front ring, inside the fuselage), and
that the extents are the aircraft's (nose 7 m, nozzle 6 m aft, 4.9 m of span a side).

Each vertex names its part (0 for the fixed airframe) and a material. **Parts** are bound to channels
the sim publishes: stabilators to the elevator, ailerons, rudder, flaps to the trailing-edge flap,
the airbrake, the three gear legs to the gear and the nozzle's glow to the afterburner. `ModelRig`
writes one matrix per part into a uniform block (at most 16: the fixed airframe and 15 parts) every
frame, without making garbage: the part's rotation about its hinge, then the airframe's attitude, then
its position relative to the eye. Signs and units are the sim's (`crates/flight/src/actuators.rs`):
elevator and flap positive trailing edge down, aileron positive rolling the right wing down, rudder
positive trailing edge left, gear 0 down to 1 up; a test moves each surface and checks the way it goes.
The Kestrel's main gear retracts through 150 degrees (it was 135, which left the wheel 0.13 m below the
belly); a test checks that every retracted wheel's hub lies inside the fuselage.

The canopy is last in the index buffer, so **the cockpit view draws the exterior without it** and the
pilot looks out through where it is; the cockpit interior (the canopy bows, glare shield, panel and
consoles, all fixed to the airframe) is drawn only in that view. The baker keeps every cockpit surface
at least 0.35 m from the design eye, over the 0.3 m near plane. One mesh pipeline draws both, in a new
`mesh.wgsl` with a `Model` block at binding 2; the classify view gives the aircraft and the cockpit
their own key colours.

## Consequences

- The sim, its hashes and the input frame are untouched by the camera and the HUD.
- Joystick hat look, gamepad look and chord bindings are not in v0.1c (cut list).
- Terrain beyond the finest levels the renderer holds is not known to the clamp (`NaN`): over the
  coarse levels the camera is clamped to the coarse surface, which is what is drawn there.
