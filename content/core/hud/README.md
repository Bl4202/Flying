# HUD layouts (`hud/1`)

One head-up display per aircraft (GDD 54.2, 54.3, 54.13, ADR-41). The file lists the elements the
HUD draws; the renderer draws them as strokes (no glyph atlas, no render target) in the HUD's
tangent plane, so every symbol is collimated by construction.

**Frame.** `at_mrad` is `[u, v]` in milliradians of the tangent plane `(tan az, tan el)` scaled by
1000: `u` right, `v` up, boresight at the origin. `tfov_deg` is the combiner's total field of view;
the baker rejects an element that reaches outside it, so nothing is ever clipped by the glass.

**Elements.** One of each type, from the registry in `tools/content-bake/src/present_model.rs`
(`HudLayer`): `boresight`, `flight_path_marker`, `pitch_ladder`, `heading_tape`, `speed_tape`,
`altitude_tape`, `vs_caret`, `g_meter`, `aoa_bracket`, `bank_scale`, `engine_fuel_block`,
`mode_line`, `warnings`. An unknown type or channel is an error (AC-54.8). Required: `boresight`,
`flight_path_marker`, `pitch_ladder` and `warnings`. Tapes name the value they show with
`channel` (`cas`, `tas`, `ground_speed`; `altitude_msl`; `nz`, `g_lagged`).

**Kestrel (`kestrel.json`).** GDD 54.2 and C.13: 28 x 20 degrees, eye box 120 x 80 x 160 mm,
`#35ff6a` with a 0.6 px glow, text 12 mrad, stroke 1.2 mrad.

| Value | Source |
|---|---|
| `tfov_deg`, `eye_box_mm`, colour, glow, text height | GDD 54.2 |
| `cage_below_mps` 20 | GDD 54.3 (the marker is caged at low speed) |
| ladder 5 degree bars, numbers every 10, thinned above 60 degrees | GDD 54.3 |
| heading tape +-30 degrees, ticks 5, numbers 10 | GDD 54.3 |
| speed tape 100 kn window, ticks 10, numbers 20; Mach from 0.6 | GDD 54.3 |
| altitude tape 1 000 ft window, radar altitude below 1 500 ft | GDD 54.3 |
| vertical speed scale +-6 000 ft/min | design choice |
| g meter amber at 85 %, red at 100 % of the limit for the configuration | design choice (ADR-41); the meter also boxes and flashes, so colour is never the only cue |
| `aoa_bracket.on_speed_deg` **12.8** | *target*: the median angle of attack on final in the virtual tester's circuit (72 m/s calibrated, all four assist levels, `fg circuit --traces`) |
| positions of tapes, meter, blocks | *target*: first layout, tuned against screenshots in WP6; changes are logged here |

Tuning log: none yet.
