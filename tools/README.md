# tools

Offline pipelines (GDD §61.3). Build-time scripts for the web app live in `packages/tools`.

| Tool | State |
|---|---|
| `content-bake` | v0.0b: validates authoring JSON, writes `.cpack` v1, exports a pack back to JSON, generates the JSON Schemas. Usage is in the header of `content-bake/src/main.rs`; run it through `cargo xtask content` |
| `geo-pipeline` | v0.2a: the offline geodata pipeline (ADR-54): Copernicus DEM tiles to `GeoPack` v1 cell archives and the base pack, the QA report, the licence audit, `datapack.json`, and the synthetic fixture (`synth`). Usage is in the header of `geo-pipeline/src/main.rs`; `cargo xtask geo-fixture` builds the fixture. Depends on `geo`, `geo-pack`, `miniz_oxide`, `serde`, `serde_json`, `sha2` |
| `model-optimiser`, `audio-pack` | reserved |

Scripts for the web build (shader generation, manifests) and the scripted reference-machine runs
(`pnpm perf:local`) are in `packages/tools`.

`content-bake` is the only crate allowed to depend on `schemars`, `serde` and friends alongside
`content` (see `crates/xtask/src/layers.rs`); the sim reads packs through the dependency-free
`content` crate.
