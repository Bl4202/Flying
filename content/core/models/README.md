# Exterior models (`model/1`)

An aircraft's outside as a handful of generated solids (ADR-41), original (ADR-11), at most 8 000
triangles. `model-mesh.ts` in the renderer builds one mesh from it; the parts that move are bound
to channels the sim publishes, so control surfaces, flaps, the airbrake, the gear and the
afterburner glow follow the sim without any animation data.

**Solids.** `fuselage` and `canopy` are rings at stations (nose to tail, decreasing x); `surfaces`
are trapezoid slabs (wing, tail halves, fin and the movable surfaces); `boxes` are intakes and
plates; `gear` are legs (a box strut and a wheel); the `nozzle` is a short cone. A static surface
may set `mirror` to draw its left twin; a moving one is written once per side.

**Triangle budget.** Counted by the builder's rules, which the baker mirrors in
`present_check.rs` (16-sided fuselage rings, 12-sided canopy rings and nozzle, 12 per slab or box,
60 per gear leg). The Kestrel needs 812.

**Parts and channels.** A `rotate` part turns about an axis through `pivot_m` by
`rotate_deg_per_unit` times its channel; a `glow` part lights with the afterburner level.

| Channel | Value | Positive means |
|---|---|---|
| `elevator_deg` | surface deflection, degrees | trailing edge down (nose down) |
| `aileron_deg` | surface deflection, degrees | rolls the right wing down: the right aileron's trailing edge goes up, the left's down |
| `rudder_deg` | surface deflection, degrees | trailing edge left (nose left) |
| `flap_deg` | trailing-edge flap, degrees | down |
| `airbrake` | 0 closed to 1 open | |
| `gear_retract` | 0 down and locked to 1 up | |
| `afterburner` | 0 to 1 | |

These are the sign conventions of `crates/flight/src/actuators.rs`. Rotation is right-handed about
`axis`: about +y, a trailing edge (behind the pivot) goes down. So a surface hinged along a line
whose direction has a positive y component turns trailing-edge-down for a positive angle, which is
why the left-hand hinge axes point inboard.

**Consistency with the aircraft.** The baker checks the wing tips against `geometry.span_m`, the
nose and tail against the `nose_tip` and `tail` contact points, the length against `bbox_m`, and
that the model has a leg for each `geometry.gear` entry with its contact point within 1 cm.

**Kestrel (`kestrel.json`).** Dimensions are *targets* chosen to match the aircraft's geometry
(span 9.8 m, nose tip 7.0 m, tail -6.0 m, gear as in `aircraft/kestrel.json`); the shape is
original. Hinge angles of the gear (-90 degrees nose, 150 degrees mains) and the airbrake (-50
degrees) are *targets*; the renderer's `model-rig.test.ts` checks that a retracted wheel's hub lies
inside the fuselage section (5 cm clear of top, bottom and sides).

Tuning log:

- v0.1c WP5: the main gear's hinge angle 135 -> 150 degrees. At 135 the hub came to rest 0.13 m
  below the belly; at 150 it is inside it. The nose gear (-90) already retracted inside.
