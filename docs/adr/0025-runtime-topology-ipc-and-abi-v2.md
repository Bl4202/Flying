# ADR-25: Runtime topology, IPC formats and ABI v2

- **Status:** Accepted
- **Owner:** Bl4202
- **Date:** 2026-10-13
- **Affects:** ADR-17, ADR-10, GDD 61.4, 61.9, 61.10; `crates/sim-core`, `crates/sim-wasm`, `packages/platform-web`, `packages/sim-bridge`, `packages/sim-host`, `packages/render`

## Context

ADR-17 fixed the four roles. v0.0b has to make three of them real (main thread, Sim Worker,
Render Worker) and decide how bytes move between them, in two modes: cross-origin isolated
(shared memory) and not (messages). It also has to do this identically for the `single` and
`threads` WASM builds, and keep every byte layout defined in exactly one place.

## Decision

- **Roles.** The main thread owns the DOM, input, the compat check, the overlay and a supervisor.
  The Sim Worker owns the WASM core. The Render Worker owns the canvas. Nothing waits on another
  thread: a stall in one never blocks a different one (proved by the `independence` spec).
- **Snapshot, Sim to Render.** Rust serialises *Snapshot v1* (10 368 bytes: a 64-byte header, the
  observer, and 256 bodies as structure-of-arrays) into its own memory each tick. The Sim Worker
  copies it out and publishes it:
  - isolated: into a 3-slot seqlock in a `SharedArrayBuffer` (31 168 bytes). One `published`
    word holds both the newest slot and the publication count, so a reader that sees a count also
    sees the slot it belongs to. The writer never waits; a reader that keeps losing the race
    gives up with `CONTENDED` and draws the previous world.
  - not isolated: a pool of four transferable `ArrayBuffer`s over a direct worker-to-worker
    `MessagePort`. The reader keeps only the newest buffer and sends older ones back; if all four
    are in flight the sink holds back the newest snapshot (replacing it with each later one) and
    sends it when a buffer returns. Either way the reader ends on the newest world.
  - The host stamps `publish_time_ms` into each copy before anyone can see it.
- **Input, Main to Sim.** 96-byte `InputFrame`s (sequence, flags, a timestamp, 12 axes, 64
  buttons, 2 hats). Isolated: an SPSC ring in a `SharedArrayBuffer` (32-byte header and 64
  frames). Not isolated: one `postMessage` per frame, at about 30 Hz, queued on the worker side in
  a ring of the same capacity. Both drop the newest frame when 64 are waiting and count it. A
  neutral frame (focus lost, tab hidden) zeroes the sticks and holds the throttle.
- **Terrain.** The 465 980-byte patch (129 x 129 posts) is generated once by the core and sent
  to the Render Worker over its own `MessagePort`.
- **Control.** Typed `postMessage` commands and 4 Hz statistics (`SimCommand`, `SimEvent`,
  `RenderCommand`, `RenderEvent`). Every receiver validates what arrives; a mismatch is `SYS-030`,
  not an `undefined` three calls later.
- **Layouts are defined once**, in `crates/sim-core/src/layout.rs`. `cargo xtask gen-abi` writes
  `packages/sim-bridge/src/layout.gen.ts` and the test vectors in `tests/vectors/`; lint fails if
  they are stale. TypeScript decoders are tested against vectors the Rust side produced.
- **ABI v2** (`crates/sim-wasm/src/abi.rs`, hand-written C ABI, no `wasm-bindgen`). The core is
  created idle (`fg_init`), content is loaded (`fg_content_reserve`, `fg_load_content`), a script
  may be chosen (`fg_set_input_script`), and `fg_start` begins it. Inputs go through a staging
  buffer (`fg_input_ptr`, `fg_push_inputs`). `fg_step` runs ticks; the snapshot, profile and
  terrain buffers are exposed by pointer. The only import is `env.fg_now` (the profiler's clock),
  plus `env.memory` for the threads build; `cargo xtask wasm` checks the list exactly. The rules
  for pointer lifetimes and view refresh are in `crates/sim-wasm/UNSAFE.md`.
- **The tick allocates nothing.** `fg_step`, `fg_push_inputs` and the pointer getters never
  allocate, so views stay valid across a run of ticks. A counting global allocator test
  (`tests/no_alloc.rs`) fails the build if that changes.
- **Time.** Timestamps that cross threads use `performance.timeOrigin + performance.now()`.
  Wall-clock fields (`t_ms`, `publish_time_ms`, the profile block) never feed simulation state or
  hashes.

## Alternatives considered

- *Rust writes straight into shared memory:* ties the snapshot to the `threads` build, so the
  degraded mode would need a second path. Copying out in JavaScript works the same for both.
- *`wasm-bindgen`:* convenient, but adds a code generator and a larger surface to audit; the ABI
  here is a dozen functions.
- *Ring buffer for snapshots:* a reader that falls behind would have to read stale entries in
  order; a seqlock gives it the newest and lets it skip.

## Consequences

- Two implementations of each channel exist and must stay equal: one contract suite per channel
  is run against both.
- Any layout change means regenerating files and touching both sides in one commit.
- The degraded mode costs copies and, with timers, jitter (ADR-26), and halves the entity caps
  once there are entities (GDD 0.6.2).
