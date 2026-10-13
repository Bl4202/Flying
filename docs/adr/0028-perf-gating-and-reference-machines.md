# ADR-28: Performance gating and the reference-machine waiver

- **Status:** Accepted
- **Owner:** Bl4202
- **Date:** 2026-10-13
- **Affects:** GDD 0.6.3, 60.2, 60.8, 60.9, AC-60.6, AC-66.4; `packages/perf`, `crates/xtask`, `.github/workflows/`

## Context

GDD 60.9 sets performance gates and 60.8 specifies a first-run benchmark. v0.0b's exit is that the
sim runs stably at 60 Hz in a worker while the renderer runs independently, on the three
reference machines (Minimum, Recommended, High). Two facts shape how that can be shown. Shared CI
runners are far too noisy for gates as tight as 5 %, and they have no GPU. And the project owner
has two machines: a Mac with an M4 and a laptop with an RTX 5060, neither of which is a Minimum
machine (4 cores, 8 GB, integrated GPU).

## Decision

- **CI checks that the harness works, not the budgets.** The `perf-smoke` job runs PS-0 for a few
  seconds and requires the sim to keep at least 57 Hz and the report to be valid. The native tick
  benchmark (`cargo xtask bench`) is informational and never compares across machine classes.
  The strict gates run on the reference machines and in the nightly workflow, whose report is
  kept as an artifact for trend reading.
- **PS-0 strict gates** are the budgets of GDD 60.2 that already apply to a loop with nothing in
  it: sim rate 60.0 +- 0.1 Hz, no simulated time dropped after a 3 s warm-up, tick cost p99 at most
  7.0 ms and average at most 4.0 ms, render CPU average at most 7.0 ms. Changing any of them
  needs an ADR (AC-66.4).
- **Four reference runs.** A: the Mac on WebGPU (Recommended class). B: the Mac with
  `renderer=webgl2` and 4x CPU throttling (Minimum, emulated). C: the laptop on its RTX 5060
  (High). D: the laptop forced onto its integrated GPU through the Windows graphics settings
  (Minimum-class GPU; CPU and memory above the Minimum). Run A and B are scripted
  (`pnpm perf:local`); C and D are run by the project owner from the runbook. Each pass requires a
  compat verdict that is not fatal, the strict gates over 120 s, frames drawn without errors or
  device loss, and at least 30 FPS (Minimum class) or the display rate (others).
- **Minimum is approximated, not met.** Run D has a real Minimum-class GPU but a faster CPU and
  more memory than the Minimum. Run B has a Minimum-like renderer path and a slowed main thread,
  but DevTools throttling may not reach workers, so it proves the WebGL2 path under load rather
  than a slow CPU. True 4-core, 8 GB hardware is an **open item under a waiver** until such a
  machine is available; it is listed in the exit checklist and does not block the tag.
- **The benchmark's thresholds are provisional.** They are hand-set from the Mac and from a
  software renderer (`packages/perf/src/benchmark/thresholds.json`) and are to be tuned against
  the four reports. AC-60.6 (10 labelled machines, 90 % agreement) cannot be shown with four.

## Alternatives considered

- *Strict gates in CI:* they would fail at random, and a gate that fails at random gets ignored.
- *Skip Minimum until hardware exists:* leaves the lowest tier unexamined; approximating it twice
  at least exercises the paths it will use.
- *A self-hosted runner with a GPU:* more infrastructure than a solo project can keep running.

## Consequences

- A regression that only shows on slow hardware can pass CI; the nightly trend and the reference
  runs are where it would appear.
- WebGPU is exercised only on real GPUs (runs A and C); CI proves WebGL2.
- The exit checklist records which items rest on approximations, and the waiver for true Minimum
  hardware is visible there.
