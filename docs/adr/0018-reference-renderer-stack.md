# ADR-18: Reference renderer stack

- **Status:** Accepted
- **Owner:** Bl4202
- **Source:** GDD §0.4 (imported verbatim at v0.0a)
- **Affects:** §44, §24, §54, §60

## Decision

**Reference renderer stack: Three.js `WebGPURenderer` for scene graph, glTF/KTX2 asset loading and standard materials, with custom WebGPU passes for planetary terrain, atmosphere, volumetric clouds, FFT ocean, and GPU particles.** Babylon.js is the evaluated alternative; a fully custom thin renderer is the fallback if either library constrains the terrain system. *Recommendation — see OQ-01.*

## Alternatives considered

Babylon.js; PlayCanvas; custom engine from scratch

## Rationale

The unusual rendering workloads are custom regardless of library; the library is chosen for asset pipeline, tooling and ecosystem.
