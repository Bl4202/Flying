# Flying

A browser-native, real-world sandbox combat flight simulator. The design lives in
[docs/GDD.md](docs/GDD.md) (full spec), [docs/GDD_Outline.md](docs/GDD_Outline.md) (outline)
and [docs/roadmap.md](docs/roadmap.md) (v0.0 → v1.0).

**Current milestone:** v0.2, the geographic sandbox (M2): pick two places on Earth and fly
between them ([roadmap section 7](docs/roadmap.md)). M1, the flight prototype, closed with the tag
`v0.1d` on 2026-11-11. It built the Kestrel with its flight-control system, the cockpit and chase
views, the HUD and sound, two test arenas, a gun, an IR missile and one AI bandit. The decisions
taken before M2 are in [ADR-50](docs/adr/0050-entering-m2.md); every milestone's exit checklist is
in [docs/exit/](docs/exit/).
How it is put together: [docs/architecture/runtime.md](docs/architecture/runtime.md) and, for the
duel, [docs/architecture/combat.md](docs/architecture/combat.md).

Staging: https://bl4202.github.io/Flying/ (add `?debug=1` for the overlay; keys are listed in the
[runtime page](docs/architecture/runtime.md#url-flags-packagesplatform-websrcflagsts)).

## Layout

| Path | Contents |
|---|---|
| `crates/` | Rust workspace: `simmath`, `flight`, `flight-test`, `fcs`, `weapons`, `ai-pilot`, `sim-core`, `sim-wasm`, `protocol`, `content`, `cli`, `server`, `xtask` |
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
| `pnpm conformance` | the Kestrel's performance conformance test (`AC-3.2`) and the virtual circuit under every assist |
| `cargo xtask sortie` | the bot playtester's gates: the sortie under every assist, the AI and missile gates (ADR-45) |
| `pnpm e2e` | Playwright against the production build (bundled Chromium; `CI=1` stops it reusing stray servers) |
| `pnpm perf:smoke` | PS-0 and the first-run benchmark, lenient; `PERF_STRICT=1 PERF_SECONDS=120` for the strict gates |
| `pnpm soak` | the 30-minute soak (gates L1 to L7; `SOAK_SECONDS`, `SOAK_BASE_URL`; [reports](docs/exit/v0.1d/soak/README.md)) |
| `pnpm bench` | native ns per tick against `tests/perf/baseline.json` (informational) |
| `pnpm perf:local` | scripted reference-machine run in real Chrome ([runbook](docs/runbooks/reference-machines.md)) |
| `pnpm verify` | lint, test, build, determinism, conformance, repro, e2e, perf:smoke and bench, in CI order (CI also runs `cargo xtask sortie`; the soak runs nightly) |

## Licence

All rights reserved for now. OQ-04 (commercial or free) is open until the public beta; meanwhile
the world data uses only sources whose licence allows commercial use ([ADR-50](docs/adr/0050-entering-m2.md)).
See [LICENSE](LICENSE).
