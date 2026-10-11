# ADR-02: Sim Core in Rust compiled to WebAssembly

- **Status:** Accepted
- **Owner:** Bl4202
- **Source:** GDD §0.4 (imported verbatim at v0.0a)
- **Affects:** §11, §19, §33, §61

## Decision

**Sim Core is written in Rust and compiled to WebAssembly**, with no browser or renderer dependency. The same crate builds natively for the dedicated server, mission-generator fuzzing, headless tests and CLI tools. It owns all gameplay state and advances on a fixed tick.

## Alternatives considered

TypeScript-only sim (GC pauses, slower numeric loops); C++/Emscripten (viable, weaker safety); C#/.NET WASM (large runtime, startup cost)

## Rationale

No GC, predictable performance, first-class WASM SIMD and threads, strong determinism control, one code base for client-host and dedicated server. Risk: Rust talent — see Appendix F.
