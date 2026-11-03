# ADR-40: The renderer stays custom; sky, terrain levels and quality presets

- **Status:** Proposed
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

The architecture of the v0.1c renderer (multiple uniform blocks, the sky and analytic globe pass,
four nested terrain levels with holes and skirts, haze, MSAA and quality presets, the classify debug
view) is added to this ADR with the work that builds it.

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
