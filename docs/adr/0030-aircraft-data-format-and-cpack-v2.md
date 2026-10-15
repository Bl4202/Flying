# ADR-30: Aircraft data format, cpack v2 and the post-stall blend

- **Status:** Accepted
- **Owner:** Bl4202
- **Date:** 2026-10-14
- **Affects:** GDD 11.6.3, 12.2, 12.3, 62.2 to 62.6, Appendix A.2, A.3, C; `crates/content`, `tools/content-bake`, `content/`

## Context

v0.0b's content pack held one fixed 144-byte record per aircraft with scalar data only (ADR-13
keeps content as data). The v0.1a flight model needs tables (lift and moment against angle of
attack and Mach, drag, thrust against Mach), engines as their own entities (GDD A.3), and the
authored performance envelope (C.9). GDD 11.6.3 also defines a post-stall blend that, read
literally, cannot reproduce the Kestrel's stall speed.

## Decision

**Three entity kinds.** `aircraft/1` (airframe, mass and tanks, geometry, engine installations,
limits, FCS data, envelope), `engine/1` (thrust deck, TSFC, dynamics) and `aero/1` (the coefficient
tables). An aircraft refers to its engines and its tables by id; the baker resolves them to
handles and checks their kinds, and so does the pack reader. Ids: `core:ac/…`, `core:eng/…`,
`core:aero/…`.

**cpack v2.** The 80-byte header keeps its fields (version 2). Table-of-contents entries grow to
24 bytes and carry a record length; records are variable-length, laid out field by field in a
fixed order per kind, and zero-padded to a multiple of 8. Tables are stored inline: a curve as a
count and its `x` and `y` arrays, a grid as two counts and its `x`, `y` and row-major `z`. The
encoding stays canonical: the reader refuses unused bytes, non-zero padding, overlong records,
unsorted ids and references to missing or wrong-kind entries. The plan's separate blob section
was dropped as unnecessary: inline tables are simpler and keep a record self-contained.

**Schema versions stay at `/1`.** `aircraft/1` changed shape (limits moved under `aero`, engines
became references, `vmo_ms` became `vmo_mps`, tanks, FCS and envelope were added). The game is
pre-release and nothing persists a pack, so there is nothing to migrate; schemas freeze at the
first public build (v0.3, First Playable) and change by major version after that (GDD A).

**Tables in JSON.** Appendix A's `Table1D` and `Table2D`, with the axis named in the data
(`"axis": "mach"`, `"axes": ["alpha_deg", "mach"]`) and checked by the baker. Angles are degrees
in data. Values are `f32` and must be exactly representable. Generated tables are produced by a
generator kept next to the data (`content/core/aero/kestrel_gen.rs`), and a test keeps the two
identical; the JSON is what the baker reads.

**Post-stall blend.** GDD 11.6.3 centres a logistic blend on the stall angle with a width of
2 deg. At the stall that gives a blend weight of 0.5, so the maximum blended lift is 1.444 instead
of 1.60 and the stall speed is 5.3 % high. The blend is now a C1 smoothstep that is exactly 0 below
`post_stall.blend_start_deg(M)` and exactly 1 above `blend_end_deg(M)`, both data. The stall
break and the post-stall fall live in `C_L,base`; the blend only reshapes the region beyond it.
The baker requires the blend to start at least 5 deg past each Mach row's lift peak.

**Trimmed C_L,max.** C.4's `C_L,max` is the whole aircraft's trimmed value. `C_L,base` is the
untrimmed lift, so its peak is raised by the elevator's trim loss (3.5 % subsonic, up to about
15 % supersonic where the static margin is larger); the baker checks the ratio is within 1.0 to 1.2.

## Alternatives considered

- **Keep fixed-size records with a blob section for tables.** More offsets to validate and no
  benefit over inline tables with lengths.
- **Bump the schemas to `/2`.** Nothing reads `/1` packs and no migration exists; a bump now would
  only add noise.
- **CSV tables converted at bake time (GDD 62.5).** Useful for spreadsheet tuning later; JSON keeps
  one format and one validator for now.
- **Keep the GDD blend and lower the stall speed target.** Rejected by ADR-31: targets are not
  changed to fit the model.

## Consequences

- Adding an aircraft means three files (or two, sharing an engine), with no code change.
- The cpack is about 5 KB for the Kestrel; tables dominate.
- The content hash changed, so the determinism golden is re-blessed in v0.1a (ADR-23).
- GDD 11.6.3 and A.2 are clarified to match (Appendix I change process).
