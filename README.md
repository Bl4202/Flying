# Flying

A browser-native, real-world sandbox combat flight simulator. The design lives in
[docs/GDD.md](docs/GDD.md) (full spec), [docs/GDD_Outline.md](docs/GDD_Outline.md) (outline)
and [docs/roadmap.md](docs/roadmap.md) (v0.0 → v1.0).

**Current milestone:** v0.0a, repository, build and CI. Exit checklist: [docs/exit/v0.0a.md](docs/exit/v0.0a.md).

Staging: https://bl4202.github.io/Flying/

## Layout

| Path | Contents |
|---|---|
| `crates/` | Rust workspace: `sim-core`, `sim-wasm`, `protocol`, `content`, `cli`, `server`, `xtask` |
| `packages/` | TypeScript workspace: `platform-web`, `app-shell`, `render`, `tools` |
| `content/`, `tools/`, `server/` | Reserved for authoring data, pipelines and the dedicated server |
| `docs/` | GDD, ADRs (`docs/adr/`), open questions, runbooks, milestone exit checklists |
| `tests/` | Golden hashes, Playwright specs |

## Prerequisites

Rust (pinned by `rust-toolchain.toml`; the threads WASM build also uses the nightly named in
`rust-nightly-version` with `rust-src`), Node (`.node-version`), pnpm via Corepack, `wasm-opt`
(binaryen), `naga` (naga-cli), `cargo-deny`.

## Commands

| Command | Does |
|---|---|
| `pnpm install --frozen-lockfile` | Install JS dependencies (lifecycle scripts disabled) |
| `pnpm lint` | rustfmt, clippy, cargo-deny, layer check, ESLint, Prettier, tsc, dependency-cruiser, WGSL check |
| `pnpm test` | `cargo test` and Vitest |
| `pnpm build` | WASM (single + threads), Vite build, headers, manifest |
| `pnpm determinism` | native vs WASM state hash after 10⁶ ticks, plus golden |
| `pnpm repro` | two clean builds, byte-compare |
| `pnpm e2e` | Playwright against the production build |
| `pnpm verify` | everything above, in CI order |

## Licence

All rights reserved for now (OQ-04 is open). See [LICENSE](LICENSE).
