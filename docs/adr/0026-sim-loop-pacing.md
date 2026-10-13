# ADR-26: Simulation loop pacing

- **Status:** Accepted
- **Owner:** Bl4202
- **Date:** 2026-10-13
- **Affects:** ADR-07, GDD 0.6.1, 5.6; `packages/sim-host`

## Context

ADR-07 fixes the simulation at 60 Hz. The Sim Worker has to keep that rate against a wall clock
that is jittery (timers are rounded to a millisecond or more), may stall (garbage collection, a
busy machine), and must never spiral: catching up must not make the next stall worse.

## Decision

- **`FixedStepClock`** is pure and tested with a fake clock. Ticks owed =
  `floor((now - anchor) / dt + 1e-9) - done`. Counting from an anchor rather than adding `dt` each
  time means rounding never accumulates; the epsilon makes an exact edge count as reached.
- **At most 5 ticks per iteration.** If more are owed, the excess is dropped by re-anchoring and
  added to `droppedMs` and `droppedEvents`: the sim runs slow instead of spiralling. A 300 ms
  stall therefore gives five catch-up ticks and about 13 dropped (the `independence` spec).
- **Each iteration**: for every owed tick, drain inputs, `step(1)`, publish the snapshot; then
  report statistics if 250 ms have passed; then yield one macrotask through a `MessageChannel`
  (so control messages are handled); then sleep until the next tick falls due.
- **Sleeping.** Isolated: `Atomics.wait` with a timeout, precise to a fraction of a millisecond
  and free while waiting. Blocking means control messages wait until the sleep ends, at most one
  tick, so the main thread bumps a shared word and calls `Atomics.notify` after each command.
  Not isolated: `scheduler.postTask` with a delay where it exists, else `setTimeout`. The clock
  corrects the rate, so what degrades is jitter, not the average.
- **Pause and resume** stop stepping and re-anchor on resume: no catch-up burst for the time
  spent paused. While paused the loop polls every 8 ms and discards queued input.
- **A hidden tab pauses the sim** (GDD 5.6, single-player rule) and showing it resumes it, unless
  the person had paused it themselves.
- **Measurement.** The worker times every tick (drain, step, publish) and keeps run-level
  statistics that `reset-run` clears after warm-up; the core times its 11 stages while profiling
  is on (off by default, one branch).

## Alternatives considered

- *`setInterval(16.67)`:* drifts, and a late tick is simply lost rather than made up.
- *Driving the sim from `requestAnimationFrame`:* throttled in background tabs and tied to the
  display rate (ADR-17 exists to avoid this).
- *Unbounded catch-up:* a long stall followed by a burst of ticks is the spiral this design
  avoids.

## Consequences

- Simulated time can fall behind wall time after a stall; `droppedMs` says by how much. The
  PS-0 gate requires zero after warm-up on the reference machines.
- `Atomics.wait` blocks the worker, so everything the worker does between sleeps must be short.
- Timer granularity in the degraded mode is browser-dependent; PS-0 measures what it costs.
