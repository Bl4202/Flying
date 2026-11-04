# ADR-39: v0.1c gates: screenshots, cameras, HUD, audio, frame rate and app flow

- **Status:** Accepted
- **Owner:** Bl4202
- **Date:** 2026-11-03
- **Affects:** GDD 7.4.2, 44, 53, 54.2, 54.3, 54.11, 56, 57.3, 60.10, 61.8, 66.3 (M1 exit);
  `packages/render`, `packages/audio`, `packages/ui`, `packages/app-shell`, `packages/platform-web`,
  `packages/perf`, `crates/sim-core`, `crates/xtask`, `tests/`, `.github/workflows/ci.yml`

## Context

The v0.1c exit (docs/roadmap.md) is: "60 FPS on the Recommended profile in the arena; horizon dip
and curvature correct in screenshot tests". The milestone also brings cockpit and chase cameras, a
core HUD, procedural test arenas with basic terrain, engine, wind and warning-tone audio, and a
first-run flow with settings, PWA install and an offline shell. This ADR fixes every pass/fail
threshold **before** any render, HUD or audio code exists, so that none can be adjusted to fit what
the code happens to do (AC-66.4).

Four points needed the owner (decisions of 2026-11-02):

- **"The Recommended profile".** The only machines are a Mac M4 and a laptop. The M4 benchmarked as
  High at v0.1a and as Minimum at v0.1b, because v0.1b's slower kernel was not followed by a
  re-base of the CPU thresholds. The owner chose: the Mac M4 gates (run A, WebGPU, as ADR-28
  designates), and the CPU thresholds are re-based by v0.1a's method (ADR-28 amendment 1), so the
  M4 reads High again. No machine of the GDD's Recommended spec has been tested; the exit records it.
- **The renderer.** It stays custom (ADR-40).
- **Art.** The aircraft and cockpit are procedural low-poly, original, generated in code.
- **Sign-off.** The automated gates below decide the tag. The owner's look-and-listen checklist on
  staging is recorded and does not block.

## Decision

### 1. Screenshot suite (exit: horizon dip and curvature)

Runs in CI (`e2e` job, headless Chromium, WebGL2 on the software rasteriser) and on the reference
machine on WebGPU. Scenes come from `?selftest=screens&case=<id>`: the page fills the four terrain
levels from the real WASM sim for the chosen arena and centre, then renders fixture poses with no
flight and no clock, so every case is deterministic. Images are 1280 x 720 in CI (one pixel is
0.097 deg at a 70 deg vertical FOV, so 0.05 deg needs about half a pixel) and 1920 x 1080 on the
reference run (GDD 60.10's size). The horizon is found per column as the sub-pixel position of the
largest luminance step (parabolic fit over three rows) and turned into an angle through the frame's
own projection.

| # | Test | Gate |
|---|---|---|
| S1 | **Horizon dip.** Level camera (pitch 0, roll 0, FOV 70 deg) over `plain` at 100 m, 1 km, 5 km, 10 km and 15 km above its 40 m base | Dip at the centre column within **+-0.05 deg** of `acos((R + b)/(R + b + h))`, R from `WORLD`, b = 40 m (3.21 deg at 10 km, GDD 7.4.2) |
| S2 | **Curvature.** The same at 10 km and 15 km over 33 columns; at FOV 100 deg; at roll 30 deg | The horizon profile matches the analytic image of the tangent circle within 0.5 px RMS and 1 px max; under roll the fitted angle within 0.1 deg; at FOV 100 deg the sagitta (centre against edges) within 1 px |
| S3 | **Seeded flat Earth** (AC-60.7). A debug-only render fault (`?fault=flat-earth`, honoured only with `debug=1`) adds the sphere drop back in the terrain vertex shader and sets R = infinity in the sky pass | S1 at 10 km **must fail**; the suite asserts the failure |
| S4 | **HUD conformity** (AC-54.1, as clarified). 10 pitch/bank/heading combinations at 30 m over `plain`, cockpit camera, classify view | The HUD 0 deg line lies above the rendered horizon by the analytic dip (about 0.18 deg, 2 px) within **0.1 deg**; the flight-path marker within 0.1 deg of a debug marker along the velocity vector |
| S5 | **Terrain cracks** (GDD 60.10 "LOD ring"). 24 views low over `rugged` across level boundaries, classify view | No globe-key pixel where the ray meets the globe inside the coarsest level's footprint (the globe lies below the arena's lowest terrain, so only a hole can show it): 0 allowed |
| S6 | **SSIM regression** (GDD 60.10). 8 scenes at 1280 x 720 (cockpit and chase x `plain` and `rugged` x low and high) | SSIM >= 0.985 against `tests/golden/screens/linux-swiftshader/*.png`; gating on Linux CI only, informational elsewhere. The spec uploads its images as a CI artifact; a bless commit copies them from a named CI run, with the run id and the reason |

Unit twins in Node (the same matrices, no GPU): HUD symbol directions against world directions
within 0.01 deg over 10^4 random states (S4); **collimation** (AC-54.4): 100 eye positions inside the
eye box move no conformal symbol by more than 0.05 deg.

### 2. Cameras

| # | Gate |
|---|---|
| K1 | AC-54.6: 1 000 seeded random low-level paths over a committed `rugged` height grid (`tests/vectors/rugged_heights.bin`, 513 x 513 posts at 62.5 m around the airfield, written by `cargo xtask gen-abi` and checked by `gen-abi --check`): neither camera goes below terrain + 1.5 m; the chase camera's arm never crosses the terrain |
| K2 | Head limits hold; the smoothing step response reaches 63 % at 60 +- 10 ms |
| K3 | AC-57.3: with reduced motion, the G offset and the shake stay <= 0.5 deg |
| K4 | Chase lag time constant within 0.6 to 1.2 s; FOV +8 deg at Vmax within 0.5 deg; at rest the chase camera sits behind the nose |

### 3. HUD

| # | Gate |
|---|---|
| U1 | Every element of the v0.1c set has a worked-example unit test (caging below 20 m/s, ladder steps, tape windows and labels, `R` below 1 500 ft, VS clamp, G thresholds, AoA bracket only with the gear down); the stroke font covers every HUD string |
| U2 | AC-54.8: the shipped `hud/1` passes; a corpus of **40 invalid layouts** fails, each with file, JSON pointer and hint |
| U3 | AC-54.3: HUD build on the CPU <= 0.3 ms on average in the arena run; the GPU HUD pass <= 0.35 ms where `timestamp-query` exists (informational otherwise); **no per-frame allocation** (a counting harness like `pump.garbage.test.ts`: 0 B at rest, a stated small bound under load) |

### 4. Audio

DSP kernels are pure TypeScript and are tested in Node by rendering blocks offline.

| # | Gate |
|---|---|
| A1 | Blade-pass peak at rpm / 60 x 24 within 1 % at idle, military and afterburner (FFT) |
| A2 | Engine level rises monotonically with core thrust; afterburner adds 6 +- 1 dB |
| A3 | Wind level rises with dynamic pressure by 10 +- 2 dB per decade |
| A4 | AC-56.5: in the browser, a warning tone starts within **100 ms** of the snapshot that raises its flag (p95 over 20 events; onset reported through the worklet's port); the aircraft bus ducks 6 dB and the warnings bus never ducks (kernel test) |
| A5 | AC-56.6: a 60 s mix rendered offline from a recorded circuit trace (committed as `packages/audio/test/fixtures/circuit-60s.csv`) measures **-16 +- 1.5 LUFS** integrated and <= -1 dBTP (BS.1770 K-weighting and gating, 4x oversampled peak) |
| A6 | Kernel cost per 128-frame quantum <= 0.21 ms (8 % of a core) on the M4 in Node (informational in CI) |
| A7 | No allocation in `process()` (counting harness) |
| A8 | The context resumes on Fly; a suspended context shows the menu prompt; the null backend passes the audio contract in Node and the Web Audio backend in the browser (`?selftest=audio`) |

### 5. Performance (exit: 60 FPS on the Recommended profile in the arena)

| # | Gate |
|---|---|
| P1 | **Arena scenario, run A** (Mac M4, headed stable Chrome, WebGPU, mains power, High preset, drawing buffer 1920 x 1080 by `?res=`): the circuit script from the runway in `rugged`, 120 s after a 5 s warm-up, cockpit camera, HUD and audio on. **Average >= 60 FPS** (frame interval average <= 16.7 ms), **1 % low >= 45 FPS** (interval p99 <= 22.2 ms), render CPU average <= 7.0 ms and p99 <= 11.0 ms, the PS-0 strict sim gates (ADR-28), no device loss, no main-thread long task over 50 ms after the warm-up |
| P2 | The same in the **chase** camera |
| P3 | Run B (WebGL2, 4x throttle, Minimum emulated): arena, cockpit camera, >= 30 FPS |
| P4 | Control latency (run L, ADR-33 section 7): p95 <= 25 ms with the HUD and audio running |
| P5 | CI `perf-smoke`: the arena scenario's report is valid and the sim keeps >= 57 Hz (a harness check, ADR-28) |

Informational: the `plain` arena; JS heap growth; GPU pass times where available; AC-53.4's
main-thread UI scripting per frame (p95 of the strip, status-feed and audio-feed callbacks) against
0.3 ms.

### 6. App flow, PWA and offline

| # | Gate |
|---|---|
| F1 | First run: every step reachable and operable by keyboard alone with visible focus (AC-53.2, keyboard half); shown once; "Run setup again" brings it back |
| F2 | AC-53.6: settings export then import round-trips byte-exactly; a `settings/1` fixture migrates to `settings/2` with every field kept |
| F3 | AC-53.4: in flight, with the HUD on and the overlay open, DOM <= 150 nodes |
| F4 | PWA: the manifest parses with `id`, `start_url`, `scope`, `display` and three icons, each fetched (200) at its declared size; the page is controlled by a worker with a fetch handler; a synthetic `beforeinstallprompt` shows Install and calls `prompt()` |
| F5 | Offline (AC-61.8, app-shell row): after one online visit, go offline and reload: compat (acknowledged), main menu, Fly, the sim ticks, frames are drawn, the audio worklet loads. `staging-smoke` adds an offline reload (AC-66.3) |
| F6 | Update flow (unit tests with a fake registration and caches): installing build B under build A and then navigating offline serves A's shell and assets; the update banner shows only in the menu; `skip-waiting` is posted; the page reloads on `controllerchange`; never during flight |
| F7 | AC-53.1 precursor: from a cached cold start, menu then Fly with `script=circuit` reaches wheels-up within 60 s |

### 7. Arenas and determinism

| # | Gate |
|---|---|
| D1 | `cargo xtask circuit --arena all`: all four assists pass C1 to C9 (ADR-33) in both arenas |
| D2 | erratic, flight60 and circuit equal their **unchanged** goldens; the new `plain-circuit` suite (runway, Standard, g Full, circuit script, `plain`, 7 200 ticks, 13 checkpoints) equals its golden; native release = debug = WASM single = WASM threads; arm64 |
| D3 | `fg_reset`, configure and start equal a fresh core (every subsystem hash, both arenas); the snapshot pointer survives a reset |

### 8. What the tag needs

Every gate above passes, plus the existing ones (determinism, no allocation in the tick, layering
and content ids, staging, a green pipeline on the final commit). The owner's checklist (views, HUD,
sound, first run, install, offline) is requested, recorded in the exit doc, and does not block.

## Alternatives considered

- *Image goldens only (SSIM) for the horizon:* a golden captures whatever the code drew; the dip and
  curvature need a measured angle against the analytic value, and a seeded fault (S3) to prove the
  measurement can fail.
- *Gate the frame rate in CI:* shared runners have no GPU and draw WebGL2 in software, so a frame-rate
  gate there would fail at random (ADR-28). CI keeps the harness check.
- *Wait for a GDD-spec Recommended machine:* none is available; the M4 gates with the gap recorded,
  as the owner decided.
- *Loudness by ear only:* the owner is the only listener; LUFS and true-peak measurement give an
  objective floor and the checklist covers taste.

## Consequences

- The screenshot gates rest on a software rasteriser in CI and a real GPU (WebGPU) on run A; either
  can expose a backend-specific error.
- The 60 FPS gate runs on a machine above the GDD's Recommended spec; the exit doc says so.
- Changing any threshold here needs an amendment to this ADR (AC-66.4).
