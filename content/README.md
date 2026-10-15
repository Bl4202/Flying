# content

Authoring data (JSON) and the schemas that describe it (GDD §62). Nothing here is read at
runtime: `content-bake` validates it and packs it into `content-out/core.cpack` (format v2,
ADR-30), which is what the sim loads.

| Path | What |
|---|---|
| `core/pack.json` | Pack identity: id (the namespace of every id inside), version, licence |
| `core/aircraft/*.json` | One aircraft per file (`aircraft/1`): mass and tanks, geometry, engine installations, limits, FCS data, the performance envelope. `kestrel.json` is Appendix C of the GDD |
| `core/engines/*.json` | One engine type per file (`engine/1`): thrust deck, TSFC, spool and afterburner dynamics. Derivations in its `README.md` |
| `core/aero/*.json` | One airframe's aerodynamic tables per file (`aero/1`). `kestrel.json` is **generated** by `kestrel_gen.rs` next to it (see its `README.md`) |
| `core/locales/en.json` | Display strings by key. Every key must be used, every used key must exist |
| `schemas/*.schema.json` | **Generated** from the Rust types in `tools/content-bake/src/model.rs`; do not edit |

Add an aircraft by adding files; no code changes (the steps are in
`docs/architecture/flight.md`). Ids look like `core:ac/kestrel`, `core:eng/f129`,
`core:aero/kestrel`; an aircraft names its engines (`engines[].ref`) and its tables
(`aero.tables_ref`) by id, and the baker checks that they exist and are of the right kind. Until
aircraft selection exists, the game flies the aircraft whose id sorts first, so a new aircraft
that sorts before `core:ac/kestrel` changes the player's aircraft and the determinism goldens.

**Units.** Every number carries its unit in its key (`empty_kg`, `span_m`, `vmo_mps`,
`alpha_limit_deg`, `tau_s`, `dry_kg_per_ns`, `cl_p_per_rad`) except a short list of dimensionless
ones (`tools/content-bake/src/schema.rs`). Angles are degrees in data and radians in the sim.
Positions are metres in body axes: x forward, y right, z down, from the reference point.

**Precision.** Every number must be exactly representable in single precision, because that is
how it is stored; the baker says which value to write instead.

**Tables** follow GDD Appendix A. A curve is `{ "axis": "mach", "points": [[x, y], ...] }`; a grid
is `{ "axes": ["alpha_deg", "mach"], "x": [...], "y": [...], "z": [[row for y0], ...] }`, row-major
`z[y][x]`. Grids are strictly increasing, interpolation is linear and clamped at the ends. Each
table field has a fixed axis and the baker checks the name. The two-dimensional aero tables must
cover alpha −30 to 90 deg and Mach 0 to 3; a value that sticks out of its row is reported as a
spike.

```sh
cargo xtask content             # validate and bake core/ into content-out/
cargo xtask content --schemas   # regenerate schemas/ after changing model.rs
cargo xtask content --check     # fail if schemas/ is stale (part of `pnpm lint`)
```

Errors name the file, the JSON pointer and a fix:

```text
error: aircraft/kestrel.json: /mass/max_takeoff_kg: maximum take-off mass 10000 kg is below empty mass plus internal fuel (11500 kg)
  fix: raise max_takeoff_kg, or correct the empty or fuel mass
```

Numeric ranges live in the schema (`minimum`/`maximum`, with `x-unit`), and the baker enforces
exactly those. Cross-field physics (take-off mass, tank capacities, wing loading, inertia,
thrust-to-weight, pitch stability against `sas_required`, the lift peak against `C_L,max`, the
post-stall blend) lives in `tools/content-bake/src/check.rs` and `bake.rs`.

**Performance.** The aircraft's `envelope` is the input of the Performance Conformance Test
(GDD §11.19, ADR-31). When the 6-DOF model misses a point, retune the data, not the target, and log
the change in the tuning log of the aero file's README.
