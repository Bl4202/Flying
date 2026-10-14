# ADR-27: Seams policy and the project's own vector types

- **Status:** Accepted
- **Owner:** Bl4202
- **Date:** 2026-10-13
- **Affects:** GDD 61.5, `AC-61.5`, `AC-66.2`, ADR-10, ADR-23; `crates/sim-core`, `crates/protocol`, `packages/render`, `packages/audio`

## Context

GDD 61.5 lists 16 extension seams: capabilities the first build cannot afford, each behind an
interface with a simple implementation and a contract test, so that replacing the stub later
changes no caller. Three are dated M0 (`GeoService`, `StrategicContext`, `Transport`) and
`Renderer` and `AudioBackend` start in M1. Separately, the simulation needs a few vector and
quaternion operations in `f64`, and ADR-10 forbids anything that could differ between targets.

## Decision

**Seams.** All 16 exist now, each with at least one implementation and a contract function that
any implementation must pass:

- *Real first implementations*, used by the tick or the app: `GeoService` (`ProceduralGeo`, a
  spherical Earth with value-noise terrain, and `FlatGeo`, which passes the same contract),
  `WeatherProvider`, `ReplaySink`, `Transport` (`LoopbackTransport`) and `Renderer` (WebGPU and
  WebGL2, ADR-24).
- *Held by the services bundle, not yet called by the tick:* `LosService`, `StrategicContext`.
- *Provisional*, with null or minimal stubs: `AeroModel`, `DamageModel`, `RadarModel`,
  `MissileModel`, `PilotBrain`, `MissionProvider`, `PersistenceSink`, `SurfaceSim`, and
  `AudioBackend` (`NullAudioBackend`). "Provisional" means the trait exists so later work has a
  place to land; nothing depends on its exact signature until its first real user arrives, and
  that user may change it. The table in `crates/sim-core/src/seams/mod.rs` is the list.
- *Contract functions* are ordinary public functions (behind `feature = "contract"` or
  `cfg(test)` in Rust) that take a factory and return or assert results. Each seam's tests run
  its stubs through its contract; a new implementation adds one line.
- *Replacing a stub* must not touch a caller. If it does, the seam was wrong and is fixed first.

**Vector types.** `sim-core::math` has its own `DVec3` and `DQuat` (add, subtract, scale, dot,
cross, length, normalise, quaternion product, rotate, body-rate integration) and
`sim-core::frame_math` has the battlefield-to-render conversion. They use only `dmath`
(ADR-10), so no transcendental function can come from a different place than the rest of the
core. `glam` is deferred.

*Amended at v0.1a (ADR-29):* `math` and `dmath` moved unchanged into the crate `simmath`, which
depends only on `libm`, so the flight model in its own crate can share them. `sim-core` re-exports
both (`sim_core::math`, `sim_core::dmath`), so no caller changed. `simmath::math` gained `DMat3`
(inertia tensors and their inverse) and the quaternion helpers RK4 needs. The move was proven
behaviour-neutral: native, WASM single and WASM threads still equal the v0.0b golden.

## Alternatives considered

- *Seams only when a milestone needs them:* cheaper now, but "replace the stub, change no
  caller" cannot be shown for a seam that does not exist.
- *`glam` for vectors:* better tested and complete, but its `libm` dependency would enable the
  `arch` feature that v0.0a deliberately switched off, letting target intrinsics replace the
  portable functions and breaking the cross-build hash test. Revisit if `glam` gains a way to keep
  that off, or when the operation set outgrows a small file.

## Consequences

- Eleven seams (the nine provisional ones and the two held but not yet called) have signatures
  that are guesses. They are cheap to change, and the policy says how: the first real user
  decides.
- A small amount of maths code is the project's to maintain, tested against vectors produced by
  `cargo xtask gen-abi` and compared with the TypeScript side (`frame-math`).
- `AC-61.5` and `AC-66.2` are checked by 16 contract suites in CI.
