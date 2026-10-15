# Runtime architecture, as built in v0.1a

What runs where, how bytes move, and which switches change it. The decisions behind it are
ADR-17 (roles), ADR-24 (renderer), ADR-25 (topology, IPC, ABI), ADR-26 (loop pacing); this page
is the map. Code references are to files in the repository. The aircraft inside the core is
described in [flight.md](flight.md).

## Topology

```text
Main thread   boot -> Service Worker isolation -> flags -> features -> GameFlow
              (BOOT -> COMPAT_CHECK -> DEMO | FATAL)
              input pump (rAF + 4 ms timer) - compat screen - debug overlay - perf and
              benchmark panels - supervisor
   |  Input channel (96 B frames)                          |  control: typed postMessage
   v                                                       v
Sim Worker    WASM core - FixedStepClock - step(1) x n (n <= 5) - stats at 4 Hz
   |  Snapshot channel (10 432 B per tick, latest wins)
   |  Terrain patch over a direct MessagePort: at start, then again around the aircraft
   |  whenever it is more than 8 km from the patch centre (checked every 500 ms)
   v
Render Worker OffscreenCanvas - interpolate two snapshots - Renderer (WebGPU | WebGL2 | Null)
              - stats at 4 Hz
```

Nothing waits on another thread. A stalled thread costs its own work and nothing else; the
`independence` end-to-end spec injects a busy main thread, a render stall, a sim stall and a pause
and checks each.

## Modes

The mode is chosen once, by `evaluateCompat` (`packages/ui/src/compat.ts`), from what the browser
has and the URL flags. The compat screen prints it.

| Mode | When | Sim build | Snapshots | Inputs | Sim sleep |
|---|---|---|---|---|---|
| Full | cross-origin isolated | `threads` | seqlock in a `SharedArrayBuffer` | SPSC ring in a `SharedArrayBuffer` | `Atomics.wait` with a timeout |
| Degraded | not isolated | `single` | four transferable buffers over a `MessagePort` | one `postMessage` per frame, about 30 Hz | `scheduler.postTask` with a delay, else `setTimeout` |

Isolation without server headers comes from the Service Worker (ADR-22): the first visit installs
it and reloads once. Where that cannot happen, the page runs in Degraded mode and says so.

The renderer is chosen in order: WebGPU (adapter and device obtained before the canvas is
touched), then WebGL2, otherwise the compat screen is fatal (`SYS-103`). The render host is the
Render Worker when the browser has `transferControlToOffscreen` and workers have
`requestAnimationFrame`, otherwise the main thread.

## Byte layouts

Little-endian, defined in `crates/sim-core/src/layout.rs`, generated into
`packages/sim-bridge/src/layout.gen.ts` by `cargo xtask gen-abi`.

| Block | Size | Contents |
|---|---|---|
| `InputFrame` | 96 B | `seq` u32, `flags` u32 (bit 0: neutral), `t_ms` f64, 12 axes f32 (pitch, roll, yaw, throttle, throttle2, brake L and R, trim P, R and Y, cursor X and Y), 64 buttons, 2 hats |
| Snapshot v2 | 10 432 B | 64 B header (magic `FGSN`, version 2, tick, `publish_time_ms`, last input `seq`, body count, core state, `input_t_ms`); 128 B ownship (reference-point position f64x3, attitude, velocity, TAS, CAS, Mach, α, β, n_z, throttle, thrust, fuel, mass, body rates, surface positions, altitude, AB stage, flags: spectator, crashed, flameout, stall warning, numerical fault); 256 bodies as structure of arrays from byte 192 |
| Snapshot SAB | 31 360 B | 64 B header (magic, version, slot size and count, the `published` word, one sequence counter per slot) and 3 slots |
| Input ring SAB | 6 176 B | 32 B header (record size, capacity 64, write and read positions, dropped count) and 64 frames |
| Profile block | 144 B | ticks, tick time (last, EWMA), 11 stage times (last, EWMA), 9 counters (inputs applied, non-finite, stale and overflowed; FDM faults, crashes, respawns, snapshots, trim failures) |
| Terrain patch | 465 980 B | origin f64x3, posts, spacing, then 129 x 129 vertices of position, normal and height |

Battlefield frame (BF) is east, north, up; render axes are `(x, y, z) -> (x, z, -y)`. Attitudes are
`[w, x, y, z]`. Body (FRD) to BF for the ownship; the camera applies the BF-to-render rotation.

The **ownship** is the player's aircraft: the core pack's lowest-handle aircraft, spawned trimmed
when the core starts. With no aircraft content it is the v0.0b observer, and the snapshot flags it
`spectator`.

## Sim ABI v2 (`crates/sim-wasm/src/abi.rs`)

```text
fg_abi_version() = 2         fg_init(seed_lo, seed_hi)        fg_core_state()
fg_content_reserve(len)      fg_load_content(len) -> status
fg_start()  fg_pause()  fg_resume()                           fg_set_input_script(kind)
fg_input_ptr()  fg_input_capacity() = 8  fg_push_inputs(count) -> accepted
fg_set_profiling(on)         fg_step(max_ticks) -> ticks_run
fg_snapshot_ptr()  fg_snapshot_len()  fg_profile_ptr()  fg_profile_len()
fg_terrain_ptr()             fg_terrain_fill(east_m, north_m, spacing_m) -> bytes
fg_tick_lo/hi()              fg_state_hash_lo/hi()            fg_subsystem_hash_lo/hi(kind)
import: env.fg_now() -> f64  (profiling clock; the threads build also imports env.memory)
```

Hash kinds: 0 combined, 1 tick, 2 rng, 3 bodies, 4 ownship, 5 input, 6 content. Input scripts: 0
none (live input), 1 erratic (the 10⁶-tick determinism run), 2 flight60 (the 60 s scripted flight
of `AC-11.7`). Buttons: 0 reset, 1 pitch trim nose up, 2 pitch trim nose down. `fg_init` leaves
the core idle; callers load content, choose a script, then `fg_start`. The pointer-lifetime and
view-refresh rules are in `crates/sim-wasm/UNSAFE.md`. v0.1a changed the snapshot layout and these
constants, not the exports, so the ABI is still version 2.

## Messages

Typed in `packages/sim-bridge/src/messages.ts` (Sim Worker) and
`packages/render/src/messages.ts` (Render Worker); each receiver validates what it gets.

| To the Sim Worker | Effect |
|---|---|
| `init` | load the module and the content pack, create the core, open the channels |
| `start`, `pause`, `resume`, `stop` | lifecycle; `pause` and `resume` re-anchor the clock |
| `profiling` | time the core's 11 stages (the overlay turns it on while it is open) |
| `reset-run` | forget run-level statistics (after a warm-up) |
| `hash` | reply with every hash column and the tick it was taken at |
| `stall` | block for N ms (honoured only with `?flags=stall`) |

| From the Sim Worker | Contents |
|---|---|
| `ready` | ABI version, build, sleeper, content hash |
| `stats` (4 Hz) | tick, rate, dropped time, tick-cost and input-latency quantiles (interval and run), queue drops, core state, the aircraft block of the latest snapshot, stage timings when profiling |
| `state`, `hash`, `error` | as named |

The Render Worker takes `init` (the canvas, snapshot channel, terrain port), `canvas` (a fresh one),
`resize`, `read-pixels`, `reset-run`, `stall` and `stop`, and reports `ready`, `stats`,
`need-canvas`, `need-main-thread`, `lost`, `pixels` and `error`.

## URL flags (`packages/platform-web/src/flags.ts`)

Unknown keys and bad values are ignored with a warning in the log.

| Flag | Values | Meaning |
|---|---|---|
| `debug` | `1`, `perf` | open the overlay at start (and expose `window.__flying`, the test hooks) |
| `renderer` | `auto`, `webgpu`, `webgl2`, `null` | choose the backend |
| `flags` | `transfer`, `mainrender`, `timer`, `stall` | force copies instead of shared memory; render on the main thread; timers instead of `Atomics.wait`; allow the stall hooks |
| `compat` | `1` | show the compat screen even if it was acknowledged |
| `selftest` | `1`, `renderer` | the determinism self-test table; the renderer contract in this browser |
| `perf` | `ps0` | run PS-0 and show the report |
| `seconds` | 1 to 3600 | length of the measured window of a perf run |
| `bench` | `quick`, `full` | run the first-run benchmark and show the report |

Keys and toggles: `Ctrl+Shift+D` opens the debug overlay. Flying: `W`/`S` pitch (`W` pushes the
nose down), `A`/`D` roll, `Q`/`E` yaw, `Shift` and `Page Up` raise the throttle, `Ctrl` and `Page
Down` lower it, `]` and `[` trim the nose up and down while held (on a pad, d-pad down and up),
`R` respawns. The stick moves the control surfaces directly; there is no flight-control system
before v0.1b. The throttle's military detent is at 80 % of the lever, with four afterburner
stages above it. Browsers keep `Ctrl+W`, `Ctrl+R` and similar for themselves, so `Page Up` and
`Page Down` are the dependable throttle keys.

## Error codes

| Code | Meaning |
|---|---|
| `SYS-002..008` | the sim module is unusable: missing memory or export, ABI mismatch, load or content failure, terrain patch |
| `SYS-010` | a required element is missing from the page |
| `SYS-020`, `021` | a shared channel's header does not match (not initialised, or a different layout) |
| `SYS-030`, `031` | a control message was malformed, or arrived before `init` |
| `SYS-032`, `033` | a worker did not become ready in time, or failed to start |
| `SYS-040` | a terrain patch is the wrong size |
| `SYS-050..053` | a WebGPU or WebGL2 backend could not start (no adapter, pipeline, context) |
| `SYS-101` | no WebAssembly |
| `SYS-102` | no WebAssembly SIMD |
| `SYS-103` | no renderer could start |
| `SYS-104..108` | the renderer could not be recovered; a canvas, worker or shader source was missing |
| `SYS-109` | the benchmark scene could not be built |
| `SYS-120`, `121` | a perf run received no statistics, or named a scenario that does not exist |

## Where things live

| Concern | Place |
|---|---|
| Fixed-step clock, sleepers, loop, worker, client | `packages/sim-host/src/` |
| Channels, layouts, messages, handle on the module | `packages/sim-bridge/src/` |
| Ring and seqlock primitives, flags, kv store, metrics, wasm loader | `packages/platform-web/src/` |
| Renderer seam, backends, camera, interpolation, session, worker | `packages/render/src/`, `packages/render/shaders/` |
| Keyboard, gamepad, shaping, the input pump | `packages/input/src/` |
| Compat check, screens, overlay, panels, game-flow reducer | `packages/ui/src/` |
| PS-0, gates, the first-run benchmark | `packages/perf/src/` |
| Audio seam | `packages/audio/src/` |
| Composition root, demo wiring, self-tests, Vite config | `packages/app-shell/` |
| Core, seams, ownship, spawn, snapshot | `crates/sim-core/src/` |
| Flight model; the conformance test | `crates/flight/src/`; `crates/flight-test/src/` ([flight.md](flight.md)) |
| Terrain patch re-centring | `packages/sim-host/src/terrain-follow.ts`, `sim.worker.ts` |
| C ABI | `crates/sim-wasm/src/abi.rs` |
