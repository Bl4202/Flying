# Cockpits (`cockpit_layout/1`)

The pilot's view from inside (GDD 54.10, A.11): the design eye, how far the head turns, the
combiner glass and a few solids that make the canopy, glare shield, panel and consoles. The
cockpit is procedural (owner decision, 2026-11-02): the renderer builds it from these boxes and
arches, so there is no mesh to ship and none of the file's numbers need a modelling tool.

**Frame.** Body axes from the aircraft's reference point: x forward, y right, z down (so "above"
is a smaller z). Distances in metres.

**What the baker checks.**
- Every surface (box, arch, the glass) is at least 0.35 m from the design eye: the cockpit camera's
  near plane is 0.3 m (GDD 44.8), and anything nearer would be clipped away.
- The eye is above the panel, and inside the model's fuselage or canopy.
- The glass is a rectangle and, seen from the eye, covers the HUD's total field of view (28 x 20
  degrees for the Kestrel), so the HUD is never cut by the glass edge.

**Kestrel (`kestrel.json`).**

| Value | Source |
|---|---|
| `eye_point_m` [3.6, 0, -0.85] | *target*: 3.6 m ahead of the reference point and 0.85 m above it, in a 13 m fuselage with the nose at x = 7 (GDD C.13 gives no number) |
| `head_limits_deg` 135 / 60 / 60 | GDD 54.11, C.13 |
| glass 0.55 m ahead of the eye, 0.275 x 0.195 m | the 28 x 20 degree field at 0.55 m is 0.274 x 0.194 m, so the glass has about half a millimetre to spare each side |
| bows, glare shield, panel, consoles | *target*: plausible solids, placed by eye; tuned in WP5 against screenshots |

Tuning log: none yet.
