/**
 * One Sim Worker iteration, independent of the worker: owed ticks, each as "drain inputs, step
 * once, publish the snapshot" (ADR-26). The worker wraps this with message handling and sleeping;
 * keeping the core here lets it run against a fake sim and a fake clock.
 */
import { Histogram } from '@flying/platform-web/metrics';
import {
  ABI,
  decodeProfile,
  INPUT_FLAG,
  INPUT_FRAME,
  newAircraftStatus,
  readAircraftStatus,
  SnapshotReader,
  stampPublishTime,
  type InputSource,
  type Quantiles,
  type SimHandle,
  type SimStats,
  type SnapshotSink,
} from '@flying/sim-bridge';
import type { FixedStepClock } from './clock.ts';

/** The part of a sim handle the loop uses. */
export type LoopSim = Pick<
  SimHandle,
  'pushInputs' | 'step' | 'tick' | 'snapshot' | 'profile' | 'coreState' | 'setProfiling'
>;

export interface SimLoopDeps {
  sim: LoopSim;
  inputs: InputSource;
  snapshots: SnapshotSink;
  clock: FixedStepClock;
  /** Monotonic milliseconds on this thread: tick scheduling and durations. */
  nowMs(): number;
  /** Milliseconds on the clock all threads share: timestamps that cross threads. */
  sharedNowMs(): number;
  sleeper: 'atomics' | 'timer';
  profiling: boolean;
}

function quantiles(h: Histogram): Quantiles {
  return {
    count: h.count,
    avg: h.mean,
    p50: h.quantile(0.5),
    p95: h.quantile(0.95),
    p99: h.quantile(0.99),
    max: h.peak,
  };
}

export class SimLoop {
  readonly #d: SimLoopDeps;
  readonly #scratch = new Uint8Array(ABI.INPUT_BATCH * INPUT_FRAME.SIZE);
  readonly #scratchView = new DataView(this.#scratch.buffer);
  // Per-tick work, 0.05 ms bins up to 20 ms; input latency, 0.25 ms bins up to 100 ms.
  readonly #step = new Histogram(20, 400);
  readonly #stepRun = new Histogram(20, 400);
  readonly #latency = new Histogram(100, 400);
  readonly #latencyRun = new Histogram(100, 400);
  // Control latency (AC-58.3): 0.25 ms bins up to 100 ms, split at the moment the frame was written.
  readonly #control = new Histogram(100, 400);
  readonly #controlRun = new Histogram(100, 400);
  readonly #poll = new Histogram(100, 400);
  readonly #pollRun = new Histogram(100, 400);
  readonly #tickWait = new Histogram(100, 400);
  readonly #tickWaitRun = new Histogram(100, 400);
  #windowStartMs: number;
  #windowStartTick = 0;
  #profiling: boolean;

  constructor(deps: SimLoopDeps) {
    this.#d = deps;
    this.#windowStartMs = deps.nowMs();
    this.#profiling = deps.profiling;
    deps.sim.setProfiling(deps.profiling);
  }

  get clock(): FixedStepClock {
    return this.#d.clock;
  }

  /** Starts ticking from now. */
  start(): void {
    this.#d.clock.start(this.#d.nowMs());
  }

  pause(): void {
    this.#d.clock.pause();
  }

  resume(): void {
    this.#d.clock.resume(this.#d.nowMs());
  }

  setProfiling(on: boolean): void {
    this.#profiling = on;
    this.#d.sim.setProfiling(on);
  }

  /** Forget run-level statistics (after warm-up). */
  resetRun(): void {
    this.#stepRun.reset();
    this.#latencyRun.reset();
    this.#controlRun.reset();
    this.#pollRun.reset();
    this.#tickWaitRun.reset();
  }

  /** Throws away queued input while nothing is stepping, so resuming starts fresh. */
  discardInputs(): void {
    while (this.#d.inputs.drain(this.#scratch, ABI.INPUT_BATCH) > 0) {
      // keep draining
    }
  }

  /** Runs whatever ticks are owed now. Returns how many ran. */
  iterate(): number {
    const owed = this.#d.clock.update(this.#d.nowMs());
    for (let i = 0; i < owed; i++) {
      this.#tick();
    }
    return owed;
  }

  #tick(): void {
    const d = this.#d;
    const t0 = d.nowMs();
    const frames = d.inputs.drain(this.#scratch, ABI.INPUT_BATCH);
    let sampledAt = Number.NaN;
    if (frames > 0) {
      sampledAt = this.#scratchView.getFloat64(
        (frames - 1) * INPUT_FRAME.SIZE + INPUT_FRAME.T_MS,
        true,
      );
      d.sim.pushInputs(this.#scratch, frames);
    }
    d.sim.step(1);
    d.snapshots.publish(d.sim.snapshot(), (copy) => {
      stampPublishTime(copy, d.sharedNowMs());
    });
    const spent = d.nowMs() - t0;
    this.#step.record(spent);
    this.#stepRun.record(spent);
    const end = d.sharedNowMs();
    if (frames > 0 && sampledAt > 0) {
      const latency = end - sampledAt;
      this.#latency.record(latency);
      this.#latencyRun.record(latency);
    }
    this.#measureControlLatency(frames, end);
  }

  /**
   * AC-58.3: for every drained frame that carries a control change, the time from the source event
   * to the end of this tick, and where it went: before the frame was written, and after.
   */
  #measureControlLatency(frames: number, end: number): void {
    const view = this.#scratchView;
    for (let i = 0; i < frames; i++) {
      const at = i * INPUT_FRAME.SIZE;
      if ((view.getUint32(at + INPUT_FRAME.FLAGS, true) & INPUT_FLAG.CHANGED) === 0) continue;
      const source = view.getFloat64(at + INPUT_FRAME.T_MS, true);
      const sent = view.getFloat64(at + INPUT_FRAME.T_SENT_MS, true);
      if (!(source > 0) || !(sent > 0)) continue; // a host that does not stamp its frames
      const control = Math.max(0, end - source);
      this.#control.record(control);
      this.#controlRun.record(control);
      const poll = Math.max(0, sent - source);
      const wait = Math.max(0, end - sent);
      this.#poll.record(poll);
      this.#pollRun.record(poll);
      this.#tickWait.record(wait);
      this.#tickWaitRun.record(wait);
    }
  }

  /** A report on the interval since the last one; starts a new interval. */
  report(): SimStats {
    const d = this.#d;
    const now = d.nowMs();
    const tick = Number(d.sim.tick());
    const seconds = Math.max((now - this.#windowStartMs) / 1000, 1e-9);
    const snap = new SnapshotReader(d.sim.snapshot());
    const pos = new Float64Array(3);
    const quat = new Float32Array(4);
    const surfaces = new Float32Array(3);
    snap.ownshipPos(pos);
    snap.ownshipQuat(quat);
    snap.ownshipSurfaces(surfaces);
    const status = newAircraftStatus();
    readAircraftStatus(snap, status);
    const stats: SimStats = {
      atMs: d.sharedNowMs(),
      tick,
      hz: (tick - this.#windowStartTick) / seconds,
      droppedMs: d.clock.droppedMs,
      droppedEvents: d.clock.dropEvents,
      step: quantiles(this.#step),
      stepRun: quantiles(this.#stepRun),
      inputLatency: quantiles(this.#latency),
      inputLatencyRun: quantiles(this.#latencyRun),
      controlLatency: quantiles(this.#control),
      controlLatencyRun: quantiles(this.#controlRun),
      pollLatency: quantiles(this.#poll),
      pollLatencyRun: quantiles(this.#pollRun),
      tickLatency: quantiles(this.#tickWait),
      tickLatencyRun: quantiles(this.#tickWaitRun),
      status,
      inputsDropped: d.inputs.dropped(),
      snapshotsDropped: d.snapshots.dropped(),
      coreState: d.sim.coreState(),
      sleeper: d.sleeper,
      aircraft: {
        pos: [pos[0] ?? 0, pos[1] ?? 0, pos[2] ?? 0],
        quat: [quat[0] ?? 1, quat[1] ?? 0, quat[2] ?? 0, quat[3] ?? 0],
        tas: snap.ownshipTas(),
        cas: snap.ownshipCas(),
        mach: snap.ownshipMach(),
        alpha: snap.ownshipAlpha(),
        beta: snap.ownshipBeta(),
        nz: snap.ownshipNz(),
        throttle: snap.ownshipThrottle(),
        thrust: snap.ownshipThrust(),
        fuel: snap.ownshipFuel(),
        mass: snap.ownshipMass(),
        surfaces: [surfaces[0] ?? 0, surfaces[1] ?? 0, surfaces[2] ?? 0],
        altitude: snap.ownshipAltitude(),
        abStage: snap.ownshipAbStage(),
        flags: snap.ownshipFlags(),
        inputSeq: snap.lastInputSeq(),
      },
      profile: null,
    };
    if (this.#profiling) {
      const p = decodeProfile(d.sim.profile());
      stats.profile = {
        tickMsLast: p.tickMsLast,
        tickMsEwma: p.tickMsEwma,
        stageMsLast: [...p.stageMsLast],
        stageMsEwma: [...p.stageMsEwma],
        counters: [...p.counters],
      };
    }
    this.#step.reset();
    this.#latency.reset();
    this.#control.reset();
    this.#poll.reset();
    this.#tickWait.reset();
    this.#windowStartMs = now;
    this.#windowStartTick = tick;
    return stats;
  }
}
