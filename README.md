# Flying

A browser-native, real-world sandbox combat flight simulator. The design lives in
[docs/GDD.md](docs/GDD.md) (full spec), [docs/GDD_Outline.md](docs/GDD_Outline.md) (outline)
and [docs/roadmap.md](docs/roadmap.md) (v0.0 → v1.0).

**Current milestone:** v0.0b, the empty loop: a sim at 60 Hz in one worker, a renderer in another,
keyboard and gamepad input, a compat check and a debug overlay. Exit checklist:
[docs/exit/v0.0b.md](docs/exit/v0.0b.md) (v0.0a: [docs/exit/v0.0a.md](docs/exit/v0.0a.md)).
How it is put together: [docs/architecture/runtime.md](docs/architecture/runtime.md).

Staging: https://bl4202.github.io/Flying/ (add `?debug=1` for the overlay; keys are listed in the
[runtime page](docs/architecture/runtime.md#url-flags-packagesplatform-websrcflagsts)).

## Layout

| Path | Contents |
|---|---|
| `crates/` | Rust workspace: `sim-core`, `sim-wasm`, `protocol`, `content`, `cli`, `server`, `xtask` |
| `packages/` | TypeScript workspace: `platform-web`, `sim-bridge`, `sim-host`, `render`, `input`, `audio`, `perf`, `ui`, `app-shell`, `tools` |
| `content/`, `tools/`, `server/` | Authoring data, offline pipelines (`content-bake`) and the reserved dedicated server |
| `docs/` | GDD, ADRs (`docs/adr/`), architecture, performance notes, open questions, runbooks, milestone exit checklists |
| `tests/` | Golden hashes, shared test vectors, the perf baseline, Playwright specs |

## Prerequisites

Rust (pinned by `rust-toolchain.toml`; the threads WASM build also uses the nightly named in
`rust-nightly-version` with `rust-src`), Node (`.node-version`), pnpm via Corepack, `wasm-opt`
(binaryen), `naga` (naga-cli), `cargo-deny`.

## Commands

| Command | Does |
|---|---|
| `pnpm install --frozen-lockfile` | Install JS dependencies (lifecycle scripts disabled) |
| `pnpm lint` | rustfmt, clippy, cargo-deny, layer check, generated-file checks (ABI layouts, schemas, GLSL), ESLint, Prettier, tsc, dependency-cruiser, WGSL check |
| `pnpm test` | `cargo test` and Vitest |
| `pnpm build` | WASM (single + threads), the content pack, Vite build, headers, manifest |
| `pnpm determinism` | native vs WASM state hash after 10⁶ ticks, plus golden |
| `pnpm repro` | two clean builds, byte-compare |
| `pnpm e2e` | Playwright against the production build (bundled Chromium; `CI=1` stops it reusing stray servers) |
| `pnpm perf:smoke` | PS-0 and the first-run benchmark, lenient; `PERF_STRICT=1 PERF_SECONDS=120` for the strict gates |
| `pnpm bench` | native ns per tick against `tests/perf/baseline.json` (informational) |
| `pnpm perf:local` | scripted reference-machine run in real Chrome ([runbook](docs/runbooks/reference-machines.md)) |
| `pnpm verify` | everything above, in CI order |

## Licence

All rights reserved for now (OQ-04 is open). See [LICENSE](LICENSE).
