# ADR-43: Test arenas, session reset (ABI v4), snapshot v4 and the presentation bundle

- **Status:** Accepted
- **Owner:** Bl4202
- **Date:** 2026-11-05
- **Affects:** ADR-10, ADR-23, ADR-36, ADR-38; GDD 7.4, 44, 62, A.2, A.11, A.12; `crates/sim-core`,
  `crates/sim-wasm`, `crates/xtask`, `tools/content-bake`, `packages/sim-bridge`

## Context

v0.1c needs two things the simulation could not do. A game with a main menu must start a second sortie
without reloading the page, and a test arena must be chosen per sortie: the horizon, curvature and
terrain-crack tests are easier to read over a flat plain, while the ground-contact work wants the
v0.1b valley. At the same time, the presentation (HUD, cockpit, exterior model, audio profile, stroke
font) must be data, but **the SHA-256 of the sim's content pack is part of the state hash**, so anything
added to that pack moves every determinism golden. The three existing goldens (erratic, flight60,
circuit) are not to change in v0.1c.

## Decision

### Two arenas

`Arena` is `Rugged` (code 0) or `Plain` (code 1), chosen per session.

- **`rugged`** is the v0.1b world unchanged (ridged noise, the 40 m basin, the runway pad), so every
  existing hash is the same.
- **`plain`** is a 40 m plain with gentle noise of +-3 m everywhere and the same runway pad. It is not
  called "flat": `FlatGeo`, a world without curvature, already exists, and the flat-Earth fault that the
  horizon tests must catch is a different thing.

Both share the airfield constants, the sphere (R = 6 371 km) and the anchor. Each arena exports a proven
lower bound of its terrain height (`terrain_floor_m`: 0 m for `rugged`, 37 m for `plain`), which the sky
pass uses for the analytic globe (ADR-40). The parameters are code constants marked as test-arena data
(GDD NN-1 permits static arenas as test content), like the basin. `rugged` keeps
`TERRAIN_DATA_VERSION` 2; `plain` has its own version 1.

### ABI v4

- `fg_configure(spawn, assist, g_effects, arena) -> u32` (idle core only) gains the arena.
- **`fg_reset() -> u32`** takes any state back to `Idle`, rebuilding the world through `with_services`
  and keeping the content, the models and the snapshot buffer: **its pointer stays valid across a
  reset**, which a test checks. A reset core, configured and started, equals a fresh one in every
  subsystem hash, in both arenas (D3). The host sets the input script again after configuring.
- `SessionConfig::hash_word` puts the arena in the **fourth byte**; `rugged` is 0, so a v0.1b session
  hashes exactly as before. The word stays an `|` of bytes, so the three existing suites' session flag
  lists are byte-identical to what they were.

### Snapshot v4

Same size (10 528 bytes): the reserved bytes 258 to 287 are used, and `SNAPSHOT_LAYOUT_VERSION` is 4
(both TypeScript readers check it). The fields are published from state that is **already hashed**, so no
hash changes: `arena` (258), `engine_bits` (259: running 1, afterburner lit 2, compressor stalled 4,
relighting 8), then `qbar_pa`, `fuel_flow_kgps`, `engine_core` (core thrust as a fraction of
military), `ab_level`, `max_nz` (highest load factor since the last spawn), `n_max_cfg` (the limit for the
current configuration) and `alpha_warn_rad` (the angle where the stall warning starts), each an `f32` at
260, 264, 268, 272, 276, 280 and 284. The HUD, the warning tones and the audio read these instead of
recomputing them.

The world's constants (`EARTH_RADIUS_M`, the anchor's latitude and longitude) are `f64`. They live in a
separate `f64` table in `layout.rs`, which `cargo xtask gen-abi` prints exactly (round-trip formatting)
into a `WORLD` object in `layout.gen.ts`, with a vector test; the layout macro takes `usize` values only.

### Goldens

The three existing goldens are compared after every sim commit and **did not change**. A new suite,
**`plain-circuit`** (runway spawn, Standard assist, full g-effects, the circuit script, `--arena plain`,
7 200 ticks), covers the second arena with ground contact, and is blessed alone with `--suite`.
`cargo xtask circuit --arena all` flies the circuit in both arenas under every assist level (D1). The
re-bless policy (ADR-23) is unchanged.

### The presentation bundle

The HUD layout, the cockpit, the exterior model, the audio profile and the HUD stroke font are content:
validated by `content-bake` (schemas, lints, file + JSON pointer + hint diagnostics), but baked into a
**separate bundle**, `content-out/core.present.json` (canonical JSON, listed with its SHA-256 in
`content-manifest.json`), **never into the sim pack**. Presentation data therefore cannot move the state
hash. The kinds are `hud/1`, `cockpit_layout/1`, `audio_profile/1`, `model/1` and `stroke_font/1`, with
ids following GDD 62.1 in the repo's form (`core:hud/kestrel`, `core:cpt/kestrel`, `core:aud/kestrel`,
`core:mdl/kestrel`, `core:fnt/hudstroke`); the bundle maps `core:ac/kestrel` to its references. It records
the SHA-256 of the sim pack it was baked with, and the app checks that against the pack it loaded. The
pack's SHA-256 is `191d750d85f5682d0e662bd9237a4df76ab1120a3790a638000c39d9cddd4762`, the same as before
any presentation content existed; `the_sim_pack_is_byte_identical_to_v0_1b` pins it, so a presentation file that
reached the pack would fail that test and move the goldens.

## Alternatives considered

- *Presentation data in the sim pack.* Simple, and it moves all three goldens for data the simulation
  never reads.
- *A flat arena (no sphere).* Already exists as `FlatGeo`, and removes the thing the horizon tests check.
- *A new snapshot size for the new fields.* The reserved bytes were set aside for exactly this, and a
  fixed size keeps both readers and the shared-memory slots unchanged.
- *Reloading the page to start another sortie.* Loses the warmed-up workers and the player's place in
  the menu, and cannot be tested as a session lifecycle.

## Consequences

- A TypeScript reader of the snapshot must be version 4; a v3 reader refuses it by version.
- A second arena doubles the circuit gate's runtime; it is one extra step in the existing `conformance`
  job, not a new required check.
- The presentation bundle is a second file to fetch and to precache; the app compares its recorded pack
  hash with the pack it loaded.
- The tuning of the Kestrel's audio levels (ADR-42) edits the bundle, not the pack, and moves no golden.
