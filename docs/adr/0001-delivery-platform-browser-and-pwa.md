# ADR-01: Delivery platform: browser and PWA

- **Status:** Accepted
- **Owner:** Bl4202
- **Source:** GDD §0.4 (imported verbatim at v0.0a)
- **Affects:** All

## Decision

**Delivery platform: desktop web browser, installable as a PWA.** Primary renderer **WebGPU**; a reduced **WebGL2 "Compatibility" renderer** is a fallback for browsers/GPUs without WebGPU. Mobile/touch is out of scope.

## Alternatives considered

Native Unity/Unreal build; Electron/Tauri wrapper (possible later as a thin shell around the same web build)

## Rationale

Required by the project owner: the game must run in a browser. Zero-install distribution, instant updates, trivially shareable seeds and links.
