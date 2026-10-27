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

## Native tick benchmark (PS-9 skeleton)

```text
cargo xtask bench [--strict] [--update] [--ticks N]
```

Times the native core (`fg bench`, erratic pilot, core pack) and compares the median ns per tick
with `tests/perf/baseline.json`. Informational unless `--strict`, never compared across machine
classes, and the 5 % budget is tight for a noisy laptop. Re-record with `--update` after an intended
change and say why in the commit. The baseline was re-recorded at v0.1a, when the tick began to fly
the 6-DOF aircraft (about 2 000 ns per tick before, about 4 200 after, on the M4), and is due again
at v0.1b, when the erratic pilot began to fly through the Standard control system.

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
| native, M4 | about 1 800 ns | about 6 300 ns | about 33 000 ns |
| WASM single under Node, M4 | about 4 300 ns | about 16 000 to 20 000 ns (noisy) | not measured |

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
