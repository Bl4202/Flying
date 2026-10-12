# tools

Offline pipelines (GDD §61.3). Build-time scripts for the web app live in `packages/tools`.

| Tool | State |
|---|---|
| `content-bake` | v0.0b: validates authoring JSON, writes `.cpack` v1, exports a pack back to JSON, generates the JSON Schemas. Usage is in the header of `content-bake/src/main.rs`; run it through `cargo xtask content` |
| `geo-pipeline` | reserved for v0.2 |
| `model-optimiser`, `audio-pack` | reserved |

`content-bake` is the only crate allowed to depend on `schemars`, `serde` and friends alongside
`content` (see `crates/xtask/src/layers.rs`); the sim reads packs through the dependency-free
`content` crate.
