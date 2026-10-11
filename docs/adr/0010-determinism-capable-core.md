# ADR-10: Determinism-capable core

- **Status:** Accepted
- **Owner:** Bl4202
- **Source:** GDD §0.4 (imported verbatim at v0.0a)
- **Affects:** §26, §34, §51, §61

## Decision

**The Sim Core is determinism-capable; procedural generation is deterministic by requirement.** WebAssembly specifies IEEE-754 results for `+ − × ÷ √` (only NaN payloads may differ), so the Rust core is built with: no fast-math, no `relaxed-simd`, software `libm` for all transcendentals (`sin`, `cos`, `atan2`, `exp`, `pow`), integer RNG, sorted iteration. A CI job compares state hashes between the WASM build and the native x86-64 build after N ticks. Generators additionally use `Fixed32` where ordering matters (§26.3).

## Alternatives considered

Global bit-exact guarantee

## Rationale

Seeds must reproduce "the same basic mission configuration" (outline §56) across machines and browsers. Runtime play does not depend on bit-exactness (server is authoritative), so a hash mismatch in play is a bug report, not a desync.
