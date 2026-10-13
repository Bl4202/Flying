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
| CPU, one thread | the simulation core stepped flat out in one worker |
| CPU, scaling | the same in up to 8 workers released together (a shared flag where the page is isolated) |
| GPU | a fixed scene at 1920 x 1080 drawn with 1, 16 and 64 copies of the terrain patch; frames are submitted back to back and the GPU is waited on once |
| Memory | `navigator.deviceMemory`, the JS heap, `measureUserAgentSpecificMemory` where allowed |
| Storage | write and read throughput through the Origin Private File System |

Each part is optional: one that fails is noted and left out of the verdict. The profile is the
lowest tier any of CPU, GPU, logical processors and memory reaches; the preset comes from the GPU,
held down by the profile. Thresholds are in `packages/perf/src/benchmark/thresholds.json` and are
**provisional**: hand-set from one Mac M4 and one software renderer, to be tuned against the
reference-machine reports. Results are stored in IndexedDB and the overlay shows the last one.

## Native tick benchmark (PS-9 skeleton)

```text
cargo xtask bench [--strict] [--update] [--ticks N]
```

Times the native core (`fg bench`, erratic pilot, core pack) and compares the median ns per tick
with `tests/perf/baseline.json`. Informational unless `--strict`, never compared across machine
classes, and the 5 % budget is tight for a noisy laptop. Re-record with `--update` after an intended
change and say why in the commit.

## Reading a report

- `metrics.step` is per-tick cost in the Sim Worker (drain, step, publish). `metrics.simHz` is
  ticks over the measured window.
- `metrics.droppedMs` is simulated time given up because the worker fell more than 5 ticks behind.
  Zero is the only passing value.
- `metrics.frameMs` is the time between frames, so its `max` is the longest hitch; `renderCpuMs` is
  time spent inside `render`, not GPU time.
- Quantiles come from fixed-width histograms (0.05 ms bins for the tick, 0.25 ms for frames), so
  small values are quantised.
