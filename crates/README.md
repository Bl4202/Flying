# crates

Rust workspace (GDD §61.2–§61.3).

| Crate | What |
|---|---|
| `sim-core` | the deterministic core: tick schedule, observer, input, snapshot, hashes, the 13 seams. Depends only on `libm` and `content` |
| `sim-wasm` | the hand-written C ABI (v2) the core is built into for the browser; the only crate with `unsafe` (see `UNSAFE.md`) |
| `content` | the dependency-free `.cpack` reader and writer and the registry |
| `protocol` | wire types and the `Transport` seam with a loopback implementation |
| `cli` | `fg`: `fg hash` (state hashes), `fg bench` (ns per tick) |
| `server` | the dedicated-server stub (`--selftest` sends a hello through the loopback transport) |
| `xtask` | `cargo xtask`: build, check and orchestration (`wasm`, `content`, `determinism`, `repro`, `bench`, `gen-abi`, `check-layers`) |

The offline content pipeline is `tools/content-bake`.

Reserved names (not created yet): `geo`, `world`, `flight`, `fcs`, `damage`, `weapons`, `sensors`,
`ew`, `ai-pilot`, `ai-squadron`, `ai-mission`, `ai-strategic`, `mission-core`, `mission-gen`,
`director`, `campaign`, `net-proto`, `net-sim`, `savegame`, `replay`.
