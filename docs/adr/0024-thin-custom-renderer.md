# ADR-24: A thin custom renderer over raw WebGPU and WebGL2

- **Status:** Accepted for v0.0b; the stack choice is revisited at v0.1c (see Consequences)
- **Owner:** Bl4202
- **Date:** 2026-10-13
- **Affects:** ADR-18, OQ-01, GDD 44.8, 54, 60.3, 61.6; `packages/render`

## Context

ADR-18 names Three.js `WebGPURenderer` with custom passes as the reference stack. v0.0b needs far
less than that: one lit terrain patch, instanced markers, a camera whose positions are made
relative to the eye in `f64` before they reach the GPU, a WebGL2 fallback, and rendering inside a
Worker on an `OffscreenCanvas` with a main-thread fallback. Fitting that behind a general scene
graph would cost more than writing it, and would tie the first milestone to a library's worker
and `OffscreenCanvas` support before the rest of the architecture has been proved.

## Decision

Write the renderer directly against raw WebGPU and raw WebGL2, behind a `Renderer` seam
(GDD 61.5). No rendering dependency is added.

- **The seam.** `resize`, `setTerrainPatch`, `render`, `finish`, `debugReadPixels`, `onLost` and
  `dispose`. A backend receives finished frame data (a 112-byte uniform block and marker
  instances built by `SceneBuilder`) and knows nothing of snapshots, cameras or the simulation.
  The backends are WebGPU, WebGL2 and a null renderer. All three pass
  `runRendererContract`, which returns results rather than asserting so the same checks run under
  Vitest (null renderer) and in the browser (`?selftest=renderer`, the `renderer` end-to-end spec).
- **One shader source.** WGSL in `packages/render/shaders/` (`common`, `terrain_patch`, `marker`),
  with includes expanded by `wgsl-pp`. The GLSL ES 3.00 that WebGL2 uses is generated from it by
  `naga` (`--profile es300 --keep-coordinate-space`), committed under `shaders/gen/`, and checked
  for freshness in lint. The build itself never needs `naga`.
- **Depth.** Reverse-Z with an infinite far plane (near 5 m), `depth32float`, compare `greater`,
  clear to 0. WebGL2 uses `EXT_clip_control` to keep WebGPU's 0..1 depth range where the
  extension exists, and otherwise builds the projection for -1..1.
- **Precision.** Positions are subtracted from the eye in `f64` on the CPU and only then cast to
  `f32`, so precision is spent near the camera (GDD 44.8, ADR-03). A test moves the whole scene
  400 km and requires the same picture.
- **Canvas policy.** A canvas holds one context type for life. WebGPU's adapter and device are
  obtained before the canvas is touched; any later switch of backend asks the main thread for a
  fresh canvas. A lost WebGPU device is rebuilt once, then the session falls back to WebGL2. A lost
  WebGL2 context is restored by the browser where it can be.
- **Where it runs.** In the Render Worker on a transferred `OffscreenCanvas` when the browser can
  do that and workers have `requestAnimationFrame`; otherwise the same session runs on the main
  thread (`MainThreadRenderHost`).

## Alternatives considered

- *Three.js `WebGPURenderer`* (ADR-18): the likely choice for the full game, deferred. Revisit at
  v0.1c, when aircraft models, materials and a HUD make a scene graph pay for itself. It would
  implement the same `Renderer` seam, so nothing above it changes.
- *Babylon.js:* the same trade-off with a larger footprint.
- *`wgpu` compiled to WebAssembly:* puts rendering in the Sim Core's module, against ADR-17's
  separation of roles, and makes the WebGL2 fallback a different code path.
- *Hand-written GLSL beside the WGSL:* two sources that drift. Generation plus a freshness check
  keeps one.

## Consequences

- The renderer is about 3 000 lines that this project owns. That is small enough to read, and it
  stays small only while it draws one patch and some markers.
- WebGL2 output depends on `naga`'s GLSL back end. The generated files are reviewed in diffs, and a
  contract failure on WebGL2 is the signal that the translation needs attention.
- CI has no GPU. The headless browser draws WebGL2 on a software rasteriser and cannot present
  WebGPU, so WebGPU is proved only on real GPUs (the reference-machine runs). The exit checklist
  says so.
- OQ-01's provisional note changes: the Rust-to-WASM half is proved, and the renderer half of the
  stack is custom until v0.1c.
