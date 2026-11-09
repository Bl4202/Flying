# Presentation architecture, as built in v0.1c

What is drawn, where the code is, and how to test it. The decisions are in
[ADR-40](../adr/0040-custom-renderer-stays.md) (the renderer, the sky, the terrain levels, the quality
presets), [ADR-41](../adr/0041-cameras-hud-and-models.md) (cameras, view input, the HUD, the aircraft
and the cockpit) and [ADR-43](../adr/0043-arenas-abi-v4-and-the-presentation-bundle.md) (arenas and the
presentation bundle); the gates are in [ADR-39](../adr/0039-v0-1c-gates.md). The threads and the bytes
between them are in [runtime.md](runtime.md); the sound is in [audio.md](audio.md). Everything here is
render-side: **nothing in the presentation reaches the simulation**, and the sim pack's hash does not
depend on any of it.

## One frame

Both backends (raw WebGPU and WebGL2 behind the `Renderer` seam, `packages/render/src/renderer.ts`) draw
the same passes; with the High preset they draw into a 4x multisampled target that is resolved into the
swapchain.

| # | Pass | Source |
|---|---|---|
| 1 | Sky and the analytic globe: one fullscreen triangle, no depth write. A ray below the horizon (the dip is `acos(Rg / (Rg + h))` at any height) meets the globe; one above is sky | `shaders/sky.wgsl`, `src/sky.ts` |
| 2 | Terrain, four nested levels, finest first: L0 250 m posts (32 km), L1 1 km (128 km), L2 4 km (512 km), L3 16 km (2 048 km); each coarser level has a hole where the finer one lies and skirts down from its outer and hole edges, so no crack can show the globe | `shaders/terrain_patch.wgsl`, `src/terrain-levels.ts`, `src/terrain-mesh.ts` |
| 3 | The exterior model (the Kestrel; not drawn in the cockpit view with its canopy) and the cockpit interior | `shaders/mesh.wgsl`, `src/model-mesh.ts`, `src/model-rig.ts` |
| 4 | Probe markers and the mouse-aim cues (as v0.1b) | `shaders/marker.wgsl`, `src/markers.ts` |
| 5 | The HUD: segments in the HUD's tangent plane, projected as **directions at infinity** (`w = 0`), so moving the eye moves nothing; an analytic combiner aperture and eye-box fade in the cockpit, a plain overlay in the chase view | `shaders/hud.wgsl`, `src/hud/` |
| 6 | The screen effect (the pilot's grey-out and red-out), unchanged | `shaders/screen_fx.wgsl` |

Shaders are WGSL, with GLSL for WebGL2 generated from them (`pnpm wgsl:gen`; `pnpm wgsl:check` fails if
the generated files are stale). Uniforms are two blocks, `Frame` (once a frame) and `TerrainLevel` (once a
level); `wgsl-gen` records every block of every stage with its binding.

## Terrain

The terrain comes from the sim's own terrain function (`fg_terrain_fill`, `ground_z_bf`, curvature
included) in four levels, so what is drawn is what the sim collides with. `packages/sim-host/src/terrain-follow.ts`
tracks the four centres, each snapped to the next coarser spacing so footprint edges lie on the coarser
grid; each level is re-centred when the aircraft is far enough from its centre (8, 32, 128 and 512 km,
chosen so every finer footprint stays inside the next coarser one even while the fills are staggered,
one level per check). All four are filled again on `configure` and `reset`. The render worker holds one
slot per level and re-uploads them after a device loss. L3's corners lie past the 1 000 km of GDD 7.4.4:
it is drawn and read for nothing.

## Cameras and view input

`CameraRig` (`src/camera-rig.ts`) runs in the render worker and never touches the sim. The cockpit eye is
the design eye from the aircraft's cockpit data; the head turns within data limits through a critically
damped spring; the chase camera follows the velocity direction with a lag on its offset from the aircraft.
Both keep the eye at least 1.5 m above the terrain (the chase arm is marched against the terrain levels the
renderer holds). Shake scales with the player's setting and is at most 0.5 degrees with reduced motion.

View actions (camera, look, zoom, free look, HUD declutter and brightness) never enter the input frame.
The input pump fills a `ViewInput` each update (`packages/input/src/pump.ts`), the app shell writes it to
the 64-byte view channel (`packages/sim-bridge/src/view-channel.ts`), and the render worker reads it each
frame. Default keys are in [controls.md](controls.md).

## The HUD

`packages/render/src/hud/`: `state.ts` derives local up, east and north in body axes, pitch, bank, true
heading and the flight-path marker from the snapshot; `elements.ts` is a registry of element types
(boresight, flight-path marker, pitch ladder, heading, speed and altitude tapes, vertical speed, G meter,
AoA bracket, bank scale, engine and fuel block, mode line, warnings) that write segments into a
preallocated array; `graph.ts` is the segment buffer and the stroke-font layout; `texts.ts` holds the
words (the app hands the shipped language over from the string table). The layout (`hud/1`), the stroke
font (`stroke_font/1`), the exterior model (`model/1`) and the cockpit (`cockpit_layout/1`) are content,
validated by `content-bake` with file, JSON pointer and hint, and baked into the **presentation bundle**,
not the sim pack. To add an element type: register it in `elements.ts`, add it to the schema and the
validator's list in `tools/content-bake`, give it a worked-example unit test and a place in the layout
file, and check that the font covers every character of its strings (a test does).

## Quality

Presets **Low** (66 % resolution scale, no MSAA), **Medium** (85 %, none) and **High** (100 %, 4x MSAA),
defaulting from the first-run benchmark and overridable in Settings, with a resolution scale of 50 to 200 %.
`?res=WxH` fixes the drawing buffer (performance and screenshot runs) and `?quality=` the preset.

## Testing the picture

The screenshot gates (S1 to S5 of ADR-39) run in the page: `?selftest=screens&case=horizon|terrain|hud`
fills the four levels from the real WASM sim, renders fixture poses (no flight, no clock) and reads the
image back (`readImage`, RGBA, top row first). The **classify view** (`?view=classify`) paints the sky,
the globe, each terrain level, the model, the cockpit and the HUD in flat key colours with no lighting or
haze, so geometry can be judged apart from shading. The horizon is found per column as the sub-pixel
position of the strongest luminance step and converted to an angle through the frame's own projection.

| Gate | What it measures | Spec |
|---|---|---|
| S1, S2 | the horizon dip at five heights (within 0.05 degrees) and the horizon as the image of the horizon circle (0.5 px RMS), under roll and at 100 degrees of field of view | `tests/e2e/horizon.spec.ts` |
| S3 | the seeded fault `?debug=1&fault=flat-earth` makes S1 and S2 fail | same |
| S4 | the HUD's zero line sits above the rendered horizon by the analytic dip (within 0.1 degrees) at ten attitudes; the seeded `hud-body-axes` fault must fail it | `tests/e2e/hud.spec.ts` |
| S5 | no globe pixel in 24 views across the level seams; the seeded `no-skirts` fault must fail it | `tests/e2e/terrain.spec.ts` |

In CI they run on WebGL2 on the software rasteriser at 1280 x 720; on a reference machine,
`SCREENS_RENDERER=webgpu PW_CHANNEL=chrome pnpm exec playwright test tests/e2e/horizon.spec.ts ...` runs
them on its GPU, and the specs fail a run that asked for WebGPU and fell back. There are no SSIM image
goldens yet (S6 was cut from v0.1c, ADR-39 amendment 1). Unit twins in Node cover the HUD's symbol
directions against world directions (10^4 states), collimation (100 eye positions in the eye box), the
camera rig (terrain clearance on 1 000 random low-level paths) and the allocation of the camera, scene and
model rig (a handful of boxed numbers a frame, no arrays or objects).

## What is not here

Free, weapon, padlock and tower cameras, head tracking, the MFDs (the HUD's render-to-texture of ADR-20
waits for them), atmosphere, clouds, ocean and biomes (v0.2c), geomorphing between levels, dynamic
resolution, glTF aircraft and an SSIM image regression. The cockpit's glare shield and panel are
placeholders that look heavy.
