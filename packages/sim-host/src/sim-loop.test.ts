import { describe, expect, it } from 'vitest';
import {
  ABI,
  INPUT_FRAME,
  PROFILE,
  SNAPSHOT,
  SnapshotReader,
  type InputSource,
  type SnapshotSink,
} from '@flying/sim-bridge';
import { FixedStepClock } from './clock.ts';
import { MAX_CATCH_UP_TICKS, SIM_DT_MS } from './constants.ts';
import { SimLoop, type LoopSim } from './sim-loop.ts';

/** Everything the loop touches, faked, with one shared log of what happened in what order. */
function rig(opts: { profiling?: boolean } = {}) {
  const log: string[] = [];
  let mono = 0;
  let shared = 1_700_000_000_000;
  let tick = 0;
  let stepCostMs = 0;
  const queue: Uint8Array[] = [];
  const published: { bytes: Uint8Array; stampedAt: number }[] = [];
  let snapshotsDropped = 0;
  let inputsDropped = 0;

  const snapshotBytes = new Uint8Array(SNAPSHOT.SIZE);
  const sim: LoopSim = {
    pushInputs: (_frames, count) => {
      log.push(`push ${String(count)}`);
      return count;
    },
    step: (n) => {
      log.push('step');
      tick += n;
      mono += stepCostMs;
      new DataView(snapshotBytes.buffer).setBigUint64(SNAPSHOT.H_TICK, BigInt(tick), true);
      return n;
    },
    tick: () => BigInt(tick),
    snapshot: () => snapshotBytes,
    profile: () => new Uint8Array(PROFILE.SIZE),
    coreState: () => 2,
    setProfiling: (on) => {
      log.push(`profiling ${String(on)}`);
    },
  };
  const inputs: InputSource = {
    drain: (out, max) => {
      const n = Math.min(queue.length, max);
      for (let i = 0; i < n; i++) out.set(queue.shift() ?? new Uint8Array(0), i * INPUT_FRAME.SIZE);
      log.push(`drain ${String(n)}`);
      return n;
    },
    pending: () => queue.length,
    dropped: () => inputsDropped,
    close: () => undefined,
  };
  const snapshots: SnapshotSink = {
    publish: (src, prepare) => {
      const copy = src.slice();
      prepare?.(copy);
      published.push({ bytes: copy, stampedAt: new SnapshotReader(copy).publishTimeMs() });
      log.push('publish');
    },
    dropped: () => snapshotsDropped,
    close: () => undefined,
  };
  const clock = new FixedStepClock({ dtMs: SIM_DT_MS, maxTicksPerIteration: MAX_CATCH_UP_TICKS });
  const loop = new SimLoop({
    sim,
    inputs,
    snapshots,
    clock,
    nowMs: () => mono,
    sharedNowMs: () => shared,
    sleeper: 'timer',
    profiling: opts.profiling ?? false,
  });
  const frameSampledAt = (tMs: number): Uint8Array => {
    const f = new Uint8Array(INPUT_FRAME.SIZE);
    new DataView(f.buffer).setFloat64(INPUT_FRAME.T_MS, tMs, true);
    return f;
  };
  return {
    log,
    loop,
    clock,
    published,
    queue,
    frameSampledAt,
    advance: (ms: number) => {
      mono += ms;
      shared += ms;
    },
    setStepCost: (ms: number) => {
      stepCostMs = ms;
    },
    setDropped: (inputs: number, snaps: number) => {
      inputsDropped = inputs;
      snapshotsDropped = snaps;
    },
    shared: () => shared,
    snapshotBytes,
  };
}

describe('SimLoop', () => {
  it('runs each owed tick as drain, step, publish, in that order', () => {
    const r = rig();
    r.loop.start();
    r.log.length = 0;
    r.advance(SIM_DT_MS * 2 + 0.1);
    expect(r.loop.iterate()).toBe(2);
    expect(r.log).toEqual(['drain 0', 'step', 'publish', 'drain 0', 'step', 'publish']);
  });

  it('hands queued input to the core before stepping, up to a batch at a time', () => {
    const r = rig();
    for (let i = 0; i < 11; i++) r.queue.push(r.frameSampledAt(i + 1));
    r.loop.start();
    r.log.length = 0;
    r.advance(SIM_DT_MS + 0.1);
    r.loop.iterate();
    expect(r.log).toEqual([
      `drain ${String(ABI.INPUT_BATCH)}`,
      `push ${String(ABI.INPUT_BATCH)}`,
      'step',
      'publish',
    ]);
    expect(r.queue).toHaveLength(3);
  });

  it('stamps the publish time into the published copy only', () => {
    const r = rig();
    r.loop.start();
    r.advance(SIM_DT_MS + 0.1);
    r.loop.iterate();
    expect(r.published).toHaveLength(1);
    expect(r.published[0]?.stampedAt).toBe(r.shared());
    expect(new SnapshotReader(r.published[0]?.bytes ?? new Uint8Array(SNAPSHOT.SIZE)).tick()).toBe(
      1,
    );
  });

  it('measures input latency from the newest frame sampled to the snapshot published', () => {
    const r = rig();
    r.loop.start();
    r.advance(SIM_DT_MS + 0.1);
    // Two frames drained this tick; the newest was sampled 7 ms ago.
    r.queue.push(r.frameSampledAt(r.shared() - 20), r.frameSampledAt(r.shared() - 7));
    r.loop.iterate();
    const s = r.loop.report();
    expect(s.inputLatency.count).toBe(1);
    expect(s.inputLatency.max).toBeCloseTo(7, 6);
    expect(s.inputLatencyRun.count).toBe(1);
  });

  it('ignores frames with no sample time', () => {
    const r = rig();
    r.loop.start();
    r.advance(SIM_DT_MS + 0.1);
    r.queue.push(r.frameSampledAt(0));
    r.loop.iterate();
    expect(r.loop.report().inputLatency.count).toBe(0);
  });

  it('reports step cost, rate and the interval reset', () => {
    const r = rig();
    r.setStepCost(1.2);
    r.loop.start();
    // One second of perfect 60 Hz, the way the worker would run it.
    for (let i = 0; i < 60; i++) {
      r.advance(Math.max(0, r.clock.nextDue() - (r.shared() - 1_700_000_000_000 > 0 ? 0 : 0)) * 0);
      r.advance(SIM_DT_MS);
      r.loop.iterate();
    }
    const s = r.loop.report();
    expect(s.tick).toBeGreaterThanOrEqual(55);
    expect(s.step.count).toBe(s.tick);
    expect(s.step.avg).toBeCloseTo(1.2, 5);
    expect(s.step.p99).toBeGreaterThanOrEqual(1.2);
    expect(s.step.p99).toBeLessThanOrEqual(1.3);
    expect(s.stepRun.count).toBe(s.step.count);
    expect(s.hz).toBeGreaterThan(0);
    expect(s.coreState).toBe(2);
    expect(s.sleeper).toBe('timer');
    // A second report covers only what happened since.
    const again = r.loop.report();
    expect(again.step.count).toBe(0);
    expect(again.stepRun.count).toBe(s.stepRun.count);
    r.loop.resetRun();
    expect(r.loop.report().stepRun.count).toBe(0);
  });

  it('runs slow instead of spiralling after a stall and says how much time it gave up', () => {
    const r = rig();
    r.loop.start();
    r.advance(200);
    expect(r.loop.iterate()).toBe(MAX_CATCH_UP_TICKS);
    const s = r.loop.report();
    expect(s.droppedEvents).toBe(1);
    expect(s.droppedMs).toBeGreaterThan(100);
    expect(r.loop.iterate()).toBe(0);
  });

  it('does not tick while paused, and resumes without a burst', () => {
    const r = rig();
    r.loop.start();
    r.advance(SIM_DT_MS + 0.1);
    expect(r.loop.iterate()).toBe(1);
    r.loop.pause();
    r.advance(30_000);
    expect(r.loop.iterate()).toBe(0);
    r.loop.resume();
    expect(r.loop.iterate()).toBe(0);
    r.advance(SIM_DT_MS + 0.1);
    expect(r.loop.iterate()).toBe(1);
    expect(r.loop.report().droppedEvents).toBe(0);
  });

  it('can throw away queued input while idle', () => {
    const r = rig();
    for (let i = 0; i < 20; i++) r.queue.push(r.frameSampledAt(1));
    r.loop.discardInputs();
    expect(r.queue).toHaveLength(0);
  });

  it('passes queue and snapshot drop counts through', () => {
    const r = rig();
    r.setDropped(4, 9);
    const s = r.loop.report();
    expect([s.inputsDropped, s.snapshotsDropped]).toEqual([4, 9]);
  });

  it('reports the observer from the newest snapshot', () => {
    const r = rig();
    const dv = new DataView(r.snapshotBytes.buffer);
    dv.setFloat64(SNAPSHOT.O_POS_BF, 100.5, true);
    dv.setFloat64(SNAPSHOT.O_POS_BF + 8, -6000, true);
    dv.setFloat64(SNAPSHOT.O_POS_BF + 16, 1500, true);
    dv.setFloat32(SNAPSHOT.O_QUAT, 0.5, true);
    dv.setFloat32(SNAPSHOT.O_QUAT + 12, -0.5, true);
    dv.setFloat32(SNAPSHOT.O_SPEED, 180, true);
    dv.setFloat32(SNAPSHOT.O_THROTTLE, 0.5, true);
    dv.setUint32(SNAPSHOT.H_LAST_INPUT_SEQ, 77, true);
    const o = r.loop.report().observer;
    expect(o.pos).toEqual([100.5, -6000, 1500]);
    expect(o.quat).toEqual([0.5, 0, 0, -0.5]);
    expect([o.speed, o.throttle, o.inputSeq]).toEqual([180, 0.5, 77]);
  });

  it('includes the core profile only while profiling', () => {
    const on = rig({ profiling: true });
    expect(on.log).toContain('profiling true');
    const p = on.loop.report().profile;
    expect(p?.stageMsLast).toHaveLength(PROFILE.STAGE_COUNT);
    expect(p?.counters).toHaveLength(PROFILE.COUNTER_COUNT);

    const off = rig();
    expect(off.loop.report().profile).toBeNull();
    off.loop.setProfiling(true);
    expect(off.loop.report().profile).not.toBeNull();
    off.loop.setProfiling(false);
    expect(off.log.filter((l) => l.startsWith('profiling'))).toEqual([
      'profiling false',
      'profiling true',
      'profiling false',
    ]);
    expect(off.loop.report().profile).toBeNull();
  });
});
