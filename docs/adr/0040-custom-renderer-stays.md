# ADR-40: The renderer stays custom; sky, terrain levels and quality presets

- **Status:** Accepted
- **Owner:** Bl4202
- **Date:** 2026-11-03
- **Affects:** ADR-18 (superseded), ADR-24, OQ-01, GDD 0.4, 44, 54, 60.3, 61.5; `packages/render`,
  `packages/tools`

## Context

ADR-18 named Three.js `WebGPURenderer` with custom passes as the reference stack. ADR-24 built a thin
custom renderer over raw WebGPU and WebGL2 for v0.0b instead, and said the choice would be revisited
at v0.1c, "when aircraft models, materials and a HUD make a scene graph pay for itself". v0.1c brings
exactly those: an exterior aircraft, a cockpit, a HUD, a sky and terrain to the horizon.

What v0.1c draws is still small: one aircraft, a procedural cockpit, a HUD made of line segments,
four terrain grids and a fullscreen sky. Every one of those is either custom in any stack (terrain,
sky, HUD) or simple enough to generate in code (the procedural low-poly Kestrel, owner decision
2026-11-02). There is no glTF, no texture and no material library to load yet.

## Decision

**Keep the custom renderer** (owner decision, 2026-11-02). Extend the raw WebGPU and WebGL2 backends
behind the `Renderer` seam (ADR-24); add no rendering dependency.

- This **supersedes ADR-18** and closes the renderer half of OQ-01.
- Three.js (or another library) stays possible later behind the same seam, for example when glTF
  aircraft, PBR materials and many objects arrive. Nothing above the seam would change.

### Architecture of the v0.1c renderer

**One frame** (both backends; sample count 1 or 4, resolved into the swapchain):

1. the **sky and analytic globe** (one fullscreen triangle, no depth write);
2. the **terrain levels**, finest first (holes and skirts; the runway is painted on the finest);
3. the probe markers and mouse-aim cues (as in v0.1b);
4. the screen effect (the pilot's g and red-out, unchanged).

Later work packages add the exterior model, the cockpit and the HUD between 3 and 4.

**Uniform blocks.** The shader tooling (`wgsl-gen`, `gen/manifest.json`) records for each stage every
uniform block naga emits, with the `@binding` of its WGSL variable, so a shader may read several. The
WebGL2 backend binds each block to its binding; the WebGPU backend has one bind-group layout with two
bindings and four bind groups, one per terrain level. `Frame` (binding 0, 224 B, once per frame) holds
the view-projection, its inverse rotation, the sun, time/field of view/aspect/eye height, the globe
(`earth`: eye height above the globe, globe radius, sin of the horizon dip, `h (2 R + h)`), local up,
the screen-effect inputs and the debug switches. `TerrainLevel` (binding 1, 48 B, once per level) holds
the patch origin minus the eye, the level, the post spacing and span, and the runway.

**The sky and the globe** (`sky.wgsl`, `sky.ts`). BF is a tangent plane on a sphere of radius R, so
the Earth's centre is at `(anchor east, anchor north, -R)`. The sky pass tests each view ray against
a sphere of radius `R + floor`, where `floor` is a proven lower bound of the arena's terrain height
(0 m for `rugged`, 37 m for `plain`), so the globe never rises through the terrain and any pixel that
shows it is a hole. Everything the shader needs is computed in `f64` on the CPU in the form that keeps
its precision: the eye's height above the globe, the sine of the horizon dip and `h (2 R + h)`. A ray
points below the horizon if `sin(elevation) < -sin(dip)`; its distance to the globe is
`c / (b + sqrt(b^2 - c))`, never a difference of large numbers. The dip is `acos(Rg / (Rg + h))` at
any height (GDD 7.4.2), tested to 0.005 degrees in Node and measured in the screenshot tests.

**Terrain in four nested levels** (`sim-bridge/terrain-levels.ts`), all from the sim's own terrain
function (`fg_terrain_fill`, curvature included), so the picture and the physics share one ground:

| level | spacing | footprint | re-centred at | centre snaps to |
|---|---|---|---|---|
| L0 | 250 m | 32 km | 8 km | 1 km |
| L1 | 1 km | 128 km | 32 km | 4 km |
| L2 | 4 km | 512 km | 128 km | 16 km |
| L3 | 16 km | 2 048 km | 512 km | 64 km |

A centre snaps to the next coarser spacing, so a finer footprint's edges lie on the coarser grid and
the coarser level leaves a hole of whole cells exactly there. The Sim Worker refreshes at most one
level per check (nearest first), and the distances leave room for that: a property test flies random
courses at up to 1 500 m/s and checks after every check that the aircraft is inside L0 and every
finer footprint inside the next coarser one. Each mesh has a **hole** (rebuilt, with the skirt
vertices, only when the finer level moves) and **skirts** that hang straight down from its outer edge
and from the edge of its hole, drawn from both sides, `max(100 m, spacing)` deep. The skirts close the
gaps that two differently sampled edges could leave, so no crack shows sky. L3's corners are up to
1 448 km from the anchor, past GDD 7.4.4's 1 000 km frame limit: the levels are only drawn, never
simulated on (a clarification). Terrain covers the horizon to about 20 km of height (L3 reaches at
least 512 km); above that the analytic globe forms it.

**Haze.** Terrain, globe and markers fade into one ground-haze colour along the path, through a haze
whose density falls off exponentially with height (integrated exactly along the line from the eye to
the surface). The sky at the horizon is at least 0.08 luma brighter than that colour, so the horizon
is a visible edge at the geometric dip at every height; the screenshot tests find it by that step.

**Quality.** A preset is a resolution scale and a sample count (`quality.ts`): Low 66 % without MSAA,
Medium 85 % without, High 100 % with 4x. The player may override the scale (50 to 200 %), and
`?quality=` and `?res=WxH` fix them for tests and perf runs. 4x MSAA is a multisampled colour and
depth target resolved into the swapchain on WebGPU (both sample counts' pipelines are built at start,
so switching never stalls a frame) and a multisampled renderbuffer blitted to the canvas on WebGL2
(where `MAX_SAMPLES` below 4 means no MSAA). The default before a preset is chosen is the v0.1b picture.

**The classify view** draws flat key colours (sky, globe, each terrain level, markers) with no
lighting or haze, so a test can tell from a pixel what drew it. The keys are constants in the shaders
and in `classify.ts`, and a test compares them. The contract's "looking up shows only sky" and the
screenshot tests for terrain cracks use it.

**The seeded flat-earth fault** (`?debug=1&fault=flat-earth`) adds the sphere's drop back in the
terrain vertex shader and takes the radius as infinite in the sky pass. The screenshot test for the
horizon dip must fail with it (ADR-39, S3).

## Alternatives considered

- *Three.js `WebGPURenderer`* (ADR-18): its scene graph, loaders and materials would carry little of
  v0.1c's work, while the terrain, sky and HUD would still be custom passes inside it. Adopting it now
  would also mean re-proving rendering in a worker on an `OffscreenCanvas`, the WebGL2 fallback and
  the relative-to-eye precision path inside a library's conventions.
- *Babylon.js:* the same trade-off with a larger footprint.

## Consequences

- The project keeps owning every line of rendering code. The renderer grows from about 3 900 lines,
  and the backends are split into per-pass modules so no file passes 1 500 lines.
- Asset formats (glTF, KTX2) will need loaders of our own, or the library decision again, when real
  art arrives.
- The question is reopened, with a new ADR, if a later milestone's scene needs a scene graph.
