# ADR-20: DOM for chrome, GPU for HUD

- **Status:** Accepted
- **Owner:** Bl4202
- **Source:** GDD §0.4 (imported verbatim at v0.0a)
- **Affects:** §53, §54, §57

## Decision

**UI split: DOM/CSS for menus, world map chrome, lobbies and settings; GPU/canvas rendering for HUD, MFDs, and cockpit displays** (render-to-texture so the same HUD renders in 3D cockpit and overlay modes).

## Alternatives considered

All-canvas UI; all-DOM HUD

## Rationale

DOM gives accessibility, text layout, IME and localization for free; the HUD must live in the 3D scene and run at render rate.
