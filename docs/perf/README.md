# Performance

How the project measures itself, what the numbers mean, and which are enforced where
(GDD 60, ADR-28).

## PS-0, the empty loop

The only scenario in v0.0b. It measures the skeleton so later milestones can see what each system
adds: the scripted pilot flies, the sim runs at 60 Hz in its worker, and the renderer draws the
terrain patch and 32 markers in its own.

```text
https://bl4202.github.io/Flying/?perf=ps0&seconds=120        # on the Pages build
pnpm perf:smoke                                               # 8 s, on the local build (lenient)
PERF_SECONDS=120 PERF_STRICT=1 pnpm perf:smoke                # the strict gates, locally
```

A run waits for both threads to report, warms up for 3 s, clears the run-level statistics, then
measures for `seconds`. All time comes from the reports' own timestamps, so when the page happens to
look does not matter. The report is `perf_report/1` JSON in `<pre data-testid="perf-report">`, with
Copy and Download buttons.

| Gate (strict) | Limit |
|---|---|
| sim rate | 60.0 +- 0.1 Hz |
| simulated time dropped after warm-up | 0 ms |
| tick cost, p99 | 7.0 ms |
| tick cost, average | 4.0 ms |
| render CPU time, average | 7.0 ms |

CI is lenient: the sim must keep at least 57 Hz and the report must be valid. Strict gates run on
the reference machines (`docs/runbooks/reference-machines.md`) and in the nightly workflow, which
keeps a two-minute report as an artifact.

## The arena scenario (v0.1c)

PS-0 measures the bare loop. `?perf=arena` measures what the v0.1c exit is about: the circuit script flown
from the runway over the rugged arena for 120 s after a 5 s warm-up, with the sky, the four terrain
levels, the aircraft, the cockpit or chase camera, the HUD and the sound all on. Unless the URL says
otherwise it draws in the cockpit, at the High preset, in a 1920 x 1080 buffer (`?camera=`, `?quality=`,
`?res=`).

```text
pnpm perf:local --name a-arena-cockpit --class recommended --scenario arena
pnpm perf:local --name a-arena-chase --class recommended --scenario arena --camera chase
pnpm perf:local --name b-arena --class minimum --scenario arena --renderer webgl2 --throttle 4
PERF_SECONDS=120 PERF_STRICT=1 pnpm perf:smoke                # the arena spec too (tests/e2e/perf/arena.spec.ts)
```

| Gate (strict) | Limit |
|---|---|
| PS-0's five | as above |
| render CPU time, p99 | 11.0 ms |
| frame interval, average | 16.7 ms (an average of 60 FPS) |
| frame interval, p99 | 22.2 ms (a 1 % low of 45 FPS) |
| the graphics device is not lost | 0 |
| main-thread tasks over 50 ms after warm-up | 0 (a missing reading where the browser has no `longtask` entries, not a pass) |
| at least 30 FPS (Minimum class, run B) | a reading, not part of the verdict (`informational`) |

CI runs the arena spec at 480 x 270 and the lowest preset on its software rasteriser and holds the sim to 57 Hz
and the report to its shape; the frame gates are for a machine with a GPU. A headed run needs mains power.
The report adds `longTasks` and `deviceLosses` to `metrics`, and a gate can be `informational`.

## The COMBAT scenario and the soak (v0.1d)

`?perf=combat` measures the dogfight: the bot playtester's sortie from the merge (an air start, the
bandit 3 km ahead) against a Regular over the rugged arena, for 120 s after a 5 s warm-up, with the
bandit, the duel's markers, the HUD's weapon symbols and the combat sound on, in the cockpit at High in a
1920 x 1080 buffer. It is judged by the arena's gates (ADR-45 PF1 to PF4). With `--scenario combat`,
`perf:local` measures the control latency with a duel running (the merge against a Regular), so one
command is runs A and L.

```text
pnpm perf:local --name a-combat-cockpit --class recommended --scenario combat --latency 500   # PF1, PF2
pnpm perf:local --name b-combat --class minimum --scenario combat --renderer webgl2 --throttle 4  # PF3
PERF_SECONDS=120 PERF_STRICT=1 pnpm perf:smoke     # includes tests/e2e/perf/combat.spec.ts (PF4)
```

**The soak** (ADR-45 section 4.7, L1 to L7) flies the bot's duel sortie from the runway against a
Regular, again and again through the debrief's Fly again, for 30 minutes after a 2-minute warm-up, at
640 x 360 and the Low preset. It samples memory every 60 s and the DOM's counters and the GPU's live
buffers and textures at every sortie start (`tests/e2e/soak/soak.spec.ts`, Playwright project `soak`;
the verdict is `packages/perf/src/soak.ts`).

| Gate | Rule |
|---|---|
| L1 | the JavaScript heap's slope at most 1 MB per 10 minutes |
| L2 | at the last sortie start, DOM nodes at most 20 and event listeners at most 2 above the second start's (after a full garbage collection) |
| L3 | the WebAssembly memory the same in every sample from the first sortie start |
| L4 | no sim silence of a second or more, no worker error, no lost device, the same number of workers throughout |
| L5 | the GPU's live buffers and textures the same at the second and every later sortie start (so at least 3 starts) |
| L6 | the live audio nodes the same in every sample from the first sortie start |
| L7 | the sim's average at least 59.5 Hz on the reference machine (57 in CI), no console error, at least 4 sortie starts in a full run |

```text
SOAK_SECONDS=1800 PW_CHANNEL=chrome pnpm soak --headed       # the full soak, on a reference machine
SOAK_SECONDS=120 SOAK_WARMUP_SECONDS=30 SOAK_GATES=quick pnpm soak   # CI's check of the harness
```

`SOAK_BASE_URL` points it at staging (default the local headers preview) and `SOAK_REPORT_DIR` says where
`soak-report.json` goes. CI's `perf-smoke` runs the two-minute quick soak (L3, L4, L6, L7); the nightly
workflow runs the full one (the `soak` job, `soak_seconds` on a manual run). Two seeded faults prove the
gates catch a leak (debug builds only): `?fault=leak-js` keeps 2 KB of JavaScript every frame (L1 must
fail; `SOAK_FAULT=leak-js SOAK_SECONDS=300`), and `?fault=leak-gpu` makes a GPU buffer at each new
session and never frees it (L5 must fail; it needs three sortie starts, so `SOAK_FAULT=leak-gpu
SOAK_WARMUP_SECONDS=0 SOAK_SECONDS=900`). A browser sortie takes about 5.3 minutes.

## First-run benchmark (prototype)

`?bench=quick` (about 4 s) or `?bench=full` (about 25 s), or the Benchmark button in the debug
overlay. It runs on a page of its own so nothing else competes.

| Part | How |
|---|---|
| CPU, one thread | the simulation core, with the core pack loaded so the Kestrel flies hands-off, stepped flat out in one worker |
| CPU, scaling | the same in up to 8 workers released together (a shared flag where the page is isolated) |
| GPU | a fixed scene at 1920 x 1080 drawn with 1, 16 and 64 copies of the terrain patch; frames are submitted back to back and the GPU is waited on once |
| Memory | `navigator.deviceMemory`, the JS heap, `measureUserAgentSpecificMemory` where allowed |
| Storage | write and read throughput through the Origin Private File System |

Each part is optional: one that fails is noted and left out of the verdict. The profile is the
lowest tier any of CPU, GPU, logical processors and memory reaches; the preset comes from the GPU,
held down by the profile. Thresholds are in `packages/perf/src/benchmark/thresholds.json` and are
**provisional**: hand-set from one Mac M4 and one software renderer, to be tuned against the
reference-machine reports. Results are stored in IndexedDB and the overlay shows the last one.

*Re-based at v0.1a.* Since v0.1a the CPU kernel flies the real aircraft (crashes and respawns
included), which ticks at 0.41 of the v0.0b spectator kernel's rate on the same machine (WASM
single under Node on the M4: about 137 000 against 333 000 ticks per second). The three CPU
thresholds were scaled by that ratio (100 000 / 180 000 / 260 000 became 41 000 / 74 000 / 107 000
ticks per second), so every machine keeps the tier it had. They stay provisional.

*Re-based at v0.1c (ADR-28 amendment 1).* The kernel changed again in v0.1b (the control system, the gear
and physiology), and the M4 read Minimum (58 177 ticks per second against 74 000). The thresholds were scaled
by 0.420, the ratio of the v0.1c kernel to the v0.1a tag's measured alternately under Node on the M4
(`node packages/tools/src/kernel-rate.ts --root DIR`: 56 934 against 135 436 ticks per second; the browser
reports give 0.391, 7 % apart), and are now 17 200 / 31 100 / 44 900. The M4 reads High again. A kernel change
that moves the score by more than 10 % is re-based the same way in the same milestone.

## Native tick benchmark (PS-9 skeleton)

```text
cargo xtask bench [--strict] [--update] [--ticks N]
```

Times the native core (`fg bench`, erratic pilot, core pack) and compares the median ns per tick
with `tests/perf/baseline.json`. Informational unless `--strict`, never compared across machine
classes, and the 5 % budget is tight for a noisy laptop. Re-record with `--update` after an intended
change and say why in the commit. The baseline was re-recorded at v0.1a, when the tick began to fly
the 6-DOF aircraft (about 2 000 ns per tick before, about 4 200 after, on the M4), and again at
v0.1b (about 10 160 ns), when the erratic pilot began to fly through the Standard control system,
the gear model and the physiology (the benchmark's session is air spawn, Standard assist).

### Per-aircraft cost (T0)

```text
fg bench --aircraft N --content content-out/core.cpack [--ticks N] [--assist full|standard|reduced|manual] [--ground] [--json]
```

Times N copies of the spawned aircraft through the control system and the flight model, in the
live world (procedural geo, still air), and reports ns per aircraft-tick: the cost of one T0
aircraft (GDD 60.2). Since v0.1b each tick runs `FcsModel::step` (default `--assist standard`)
before `step_tick`, so the number includes the control laws and the wheels' contact search. The
aircraft fly gentle inputs on every axis and restart from the trimmed spawn every 20 s, so they
stay in normal flight. `--ground` starts them on the runway with a taxi throttle (0.45), the brakes
released and a light touch of both brakes (0.3) from 15 s to 16.7 s, which exercises the gear model, the friction
and the ground law instead; it fails if an aircraft leaves the ground, since the number would then
not be the ground's.

| Where | v0.1a (manual law, no wheels) | v0.1b air, Standard | v0.1b ground |
|---|---|---|---|
| native, M4 | about 1 800 ns | about 6 600 ns | about 34 000 ns |
| WASM single under Node, M4 | about 4 300 ns | about 16 000 to 20 000 ns (noisy) | not measured |

In the browser (reference run A, 120 s) the whole sim step averaged 22 us a tick with the one
aircraft, against a gate of 4 ms.

The informational budget is 20 us per aircraft-tick in WASM. The air case is inside it with
little margin, and the ground case is outside it natively, which does not matter for one player
(it is 0.2 % of a 16.7 ms tick) but matters for the AI fleets of v0.1d. Where the time goes: the
ground case evaluates eight terrain samples per wheel per RK4 stage plus the contact points
(ADR-35); a cheaper pad normal would change the hashes, so it is deferred to a milestone that
re-blesses anyway. A first version of the control system added about 7 us because every
telemetry call took eight full surface samples for the wheels' height; `sense_light` and
`telemetry_light` read the height only where it is used, which removed the cost without
changing a single hash (`6c1e851`).

## Control latency (AC-58.3)

The definition is in ADR-33 section 7 and the stack in
[controls.md](../architecture/controls.md): source event to the end of the sim tick that acted on
it, on frames that carry a change, display latency excluded. Two ways to read it:

```text
tests/e2e/latency.spec.ts                       # CI, headless: at least 200 changes, p95 <= 33.3 ms
pnpm perf:local --latency 500 --name m4-latency --class recommended
                                                # reference machine, headed Chrome, on power:
                                                # at least 500 changes, p95 <= 25 ms
```

The sim worker keeps three histograms per changed frame (control, poll, tick), per report
interval and since `resetRun`; the overlay's `ctl` row shows the run-scoped ones. Headless CI
measured p95 between 16.5 and 21 ms. `perf:local` records the display rate beside the result,
since a 60 Hz display bounds how soon a frame can be written. Reports go to
`docs/exit/v0.1b/machines/` (`docs/runbooks/reference-machines.md`).

## Reading a report

- `metrics.step` is per-tick cost in the Sim Worker (drain, step, publish). `metrics.simHz` is
  ticks over the measured window.
- `metrics.droppedMs` is simulated time given up because the worker fell more than 5 ticks behind.
  Zero is the only passing value.
- `metrics.frameMs` is the time between frames, so its `max` is the longest hitch; `renderCpuMs` is
  time spent inside `render`, not GPU time.
- Quantiles come from fixed-width histograms (0.05 ms bins for the tick, 0.25 ms for frames), so
  small values are quantised.
