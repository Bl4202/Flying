# content

Authoring data (JSON) and the schemas that describe it (GDD §62). Nothing here is read at
runtime: `content-bake` validates it and packs it into `content-out/core.cpack`, which is what the
sim loads.

| Path | What |
|---|---|
| `core/pack.json` | Pack identity: id (the namespace of every id inside), version, licence |
| `core/aircraft/*.json` | One aircraft per file. `kestrel.json` is Appendix C of the GDD |
| `core/locales/en.json` | Display strings by key. Every key must be used, every used key must exist |
| `schemas/*.schema.json` | **Generated** from the Rust types in `tools/content-bake/src/model.rs`; do not edit |

Add an aircraft by adding a file; no code changes. Ids look like `core:ac/kestrel`. Every number
carries its unit in its key (`empty_kg`, `span_m`, `vmo_ms`) except a short list of dimensionless
ones, and must be exactly representable in single precision, because that is how it is stored.

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
exactly those. Cross-field physics (take-off mass, wing loading, inertia, thrust-to-weight) lives in
`tools/content-bake/src/check.rs`.
