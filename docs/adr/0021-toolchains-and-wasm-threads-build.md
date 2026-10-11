# ADR-21: Toolchains and the WASM threads build

- **Status:** Accepted
- **Owner:** Bl4202
- **Date:** 2026-10-10
- **Affects:** GDD 61.3, ADR-02, ADR-10, ADR-17; `rust-toolchain.toml`, `rust-nightly-version`, `crates/xtask`

## Context

GDD 61.3 asks for two WASM builds: `single` and `threads` (`+atomics,+bulk-memory,+simd128`, shared
memory). The standard library shipped for `wasm32-unknown-unknown` is built without atomics, so a
threads build has to rebuild `std` (`-Z build-std`), which needs a nightly compiler and the
`rust-src` component.

## Decision

- **Everything is stable except one artifact.** Rust is pinned by `rust-toolchain.toml` (1.99.0).
  Only `sim.threads.wasm` is built on a nightly, pinned by date in `rust-nightly-version`
  (`nightly-2026-10-08` at v0.0a). Linting, tests, the native build, the `single` module and the
  determinism reference all use stable.
- **Nightly bumps are deliberate.** Changing the date is a one-line commit that must pass the
  determinism and repro jobs. If `build-std` breaks on a date, try an adjacent date; the stable-only
  jobs are unaffected.
- **Dedicated `wasm` cargo profile** (`inherits = "release"`, `strip = "debuginfo"`). `strip = true`
  removes the `target_features` custom section, which `xtask` reads to derive `wasm-opt`'s
  `--enable-*` flags and to assert that `relaxed-simd` is absent. `wasm-opt --strip-debug` removes
  the names section afterwards.
- **No `wasm-bindgen` on the hot path.** The ABI is hand-written `extern "C"` (GDD 61.3); 64-bit
  values cross as two `u32` halves so JavaScript needs no `BigInt` in hot calls.
- **`libm` pinned exactly (`=0.2.16`) with default features off.** Its `arch` feature lets it use
  target intrinsics, which would break cross-architecture bit-exactness (ADR-10).
- **JS toolchain:** Node 24 (`.node-version`), pnpm 12 via Corepack (`packageManager`), exact
  dependency versions, lifecycle scripts disabled (`allowBuilds` denies esbuild's, which is not
  needed because its binary ships as an optional dependency).
- **TypeScript is held at 6.0.x.** `typescript-eslint` 8.71 declares support for TypeScript below
  6.1; TypeScript 7 is not yet supported by it. Revisit when it is.
- **Build tools are pinned and checksummed** in `tools/ci/install-tools.sh`: binaryen
  `version_133`, cargo-deny `0.20.2`, naga-cli `30.0.1`.
- **Licence allow-list gains `Zlib`** (permissive, BSD-like). It arrived via `foldhash`, a
  transitive dependency of `wasmparser`, which only `xtask` uses (build tooling, never shipped).
  This extends the list in GDD 61.7 by one permissive licence.

## Alternatives considered

- *Nightly for everything:* simpler, but exposes tests, lints and the determinism reference to
  nightly churn for the sake of one artifact.
- *Skip the threads build until needed:* postpones discovering `build-std`, shared-memory import and
  cross-origin-isolation problems to a milestone where they would block real work.
- *`wasm-bindgen` for the ABI:* adds a JS glue layer and generated code to a boundary that has nine
  functions.

## Consequences

- CI installs two Rust toolchains for jobs that build WASM.
- The threads build's shared memory is created by the loader with `initial = 256` pages (16 MiB),
  `maximum = 32768` pages (2 GiB); `cargo xtask wasm` asserts the module's import matches.
