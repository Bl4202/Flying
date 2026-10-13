import { describe, expect, it } from 'vitest';
import type { RenderStats } from '@flying/render/messages';
import type { Quantiles, SimStats } from '@flying/sim-bridge';
import { evaluateGates, PS0_LIMITS, verdictOf } from './gates.ts';
import { describeMachine } from './machine.ts';
import { isPerfReport, type PerfMetrics } from './report.ts';
import { runScenario, type Progress, type PerfTarget } from './runner.ts';
import { findScenario, PS0 } from './scenarios.ts';

const q = (over: Partial<Quantiles> = {}): Quantiles => ({
  count: 100,
  avg: 0.5,
  p50: 0.4,
  p95: 0.9,
  p99: 1.2,
  max: 2,
  ...over,
});

interface Behaviour {
  hz: number;
  stepP99: number;
  stepAvg: number;
  renderCpuAvg: number;
  /** Simulated milliseconds dropped per second, once `dropsFrom` seconds have passed. */
  dropMsPerSecond: number;
  dropsFrom: number;
  fps: number;
}

const HEALTHY: Behaviour = {
  hz: 60,
  stepP99: 1.2,
  stepAvg: 0.5,
  renderCpuAvg: 0.2,
  dropMsPerSecond: 0,
  dropsFrom: 0,
  fps: 60,
};

/** A world that advances by sleeping: every 250 ms both threads publish a report. */
function world(b: Partial<Behaviour> = {}) {
  const beh = { ...HEALTHY, ...b };
  let now = 1_700_000_000_000;
  let elapsedS = 0;
  let tick = 0;
  let frames = 0;
  let droppedMs = 0;
  let runStart = 0;
  let sincePublishMs = 0;
  let sim: SimStats | undefined;
  let render: RenderStats | undefined;
  const resets: number[] = [];

  const publish = (): void => {
    const sinceRunS = elapsedS - runStart;
    sim = {
      atMs: now,
      tick: Math.round(tick),
      hz: beh.hz,
      droppedMs,
      droppedEvents: droppedMs > 0 ? 1 : 0,
      step: q(),
      stepRun: q({ count: Math.round(sinceRunS * beh.hz), avg: beh.stepAvg, p99: beh.stepP99 }),
      inputLatency: q(),
      inputLatencyRun: q(),
      inputsDropped: 0,
      snapshotsDropped: 0,
      coreState: 2,
      sleeper: 'atomics',
      observer: { pos: [0, 0, 0], quat: [1, 0, 0, 0], speed: 0, throttle: 0, inputSeq: 0 },
      profile: null,
    };
    render = {
      atMs: now,
      backend: 'webgl2',
      description: 'test',
      width: 1280,
      height: 720,
      fps: beh.fps,
      frameMs: q(),
      frameMsRun: q({ count: Math.round(sinceRunS * beh.fps) }),
      renderCpuMs: q(),
      renderCpuRun: q({ count: Math.round(sinceRunS * beh.fps), avg: beh.renderCpuAvg }),
      inputToDrawMs: q(),
      inputToDrawRun: q(),
      snapshotAgeMs: q(),
      frames: Math.round(frames),
      snapshotsTaken: 0,
      staleFrames: 0,
      gappedFrames: 0,
      snappedFrames: 0,
      contendedReads: 0,
    };
  };

  const target: PerfTarget = {
    stats: () => ({ sim, render }),
    resetRun: () => {
      runStart = elapsedS;
      resets.push(elapsedS);
    },
  };
  const env = {
    sharedNowMs: () => now,
    sleep: (ms: number): Promise<void> => {
      let left = ms;
      while (left > 0) {
        const step = Math.min(50, left);
        left -= step;
        now += step;
        elapsedS += step / 1000;
        tick += (beh.hz * step) / 1000;
        frames += (beh.fps * step) / 1000;
        if (elapsedS >= beh.dropsFrom) droppedMs += (beh.dropMsPerSecond * step) / 1000;
        sincePublishMs += step;
        if (sincePublishMs >= 250) {
          sincePublishMs -= 250;
          publish();
        }
      }
      return Promise.resolve();
    },
  };
  return { target, env, resets, silent: () => (sim = render = undefined) };
}

function run(w: ReturnType<typeof world>, seconds = 10, onProgress?: (p: Progress) => void) {
  return runScenario({
    scenario: PS0,
    seconds,
    target: w.target,
    env: w.env,
    build: () => ({ id: 'abc', abi: 2, contentHash: '0123456789abcdef' }),
    machine: () => describeMachine({ navigator: { userAgent: 'test' } }, '', 'webgl2 test'),
    modes: () => ({
      variant: 'threads',
      snapshotChannel: 'shared',
      inputChannel: 'shared',
      renderHost: 'worker',
      sleeper: 'atomics',
      renderer: 'webgl2',
    }),
    now: () => new Date('2026-10-13T12:00:00Z'),
    ...(onProgress ? { onProgress } : {}),
    statsTimeoutMs: 2000,
  });
}

describe('runScenario (PS-0)', () => {
  it('passes every gate for a healthy loop, and measures the rate between reports', async () => {
    const report = await run(world());
    expect(report.verdict).toBe('pass');
    expect(report.gates.every((g) => g.pass)).toBe(true);
    expect(report.metrics.simHz).toBeCloseTo(60, 6);
    expect(report.metrics.renderFps).toBeCloseTo(60, 6);
    // A window of about the requested length (the baseline is the first report after the reset).
    expect(report.metrics.windowS).toBeGreaterThan(9.5);
    expect(report.metrics.windowS).toBeLessThan(10.5);
    expect(report.metrics.ticks).toBeCloseTo(report.metrics.windowS * 60, 0);
    expect(report.scenario).toBe('PS-0');
    expect(report.startedAt).toBe('2026-10-13T12:00:00.000Z');
  });

  it('clears the run statistics exactly once, after the warm-up', async () => {
    const w = world();
    await run(w);
    expect(w.resets).toHaveLength(1);
    expect(w.resets[0]).toBeGreaterThanOrEqual(PS0.warmupS);
    expect(w.resets[0]).toBeLessThan(PS0.warmupS + 1);
  });

  it('fails the rate gate, and only that, when the sim runs at 59 Hz', async () => {
    const report = await run(world({ hz: 59 }));
    expect(report.verdict).toBe('fail');
    expect(report.gates.filter((g) => !g.pass).map((g) => g.id)).toEqual(['sim-rate']);
  });

  it('fails the dropped-time gate for time given up during the window, not before it', async () => {
    const early = await run(world({ dropMsPerSecond: 100, dropsFrom: 0 }), 10);
    // Drops that stop being counted once the warm-up is over: none happen in the window.
    expect(early.metrics.droppedMs).toBeGreaterThan(0);
    expect(early.gates.find((g) => g.id === 'dropped')?.pass).toBe(false);

    const w = world({ dropMsPerSecond: 100, dropsFrom: 1000 });
    const late = await run(w, 10);
    expect(late.metrics.droppedMs).toBe(0);
    expect(late.gates.find((g) => g.id === 'dropped')?.pass).toBe(true);
  });

  it('fails the step and render gates when those costs are over budget', async () => {
    const report = await run(world({ stepP99: 8, stepAvg: 4.5, renderCpuAvg: 7.5 }));
    expect(
      report.gates
        .filter((g) => !g.pass)
        .map((g) => g.id)
        .sort(),
    ).toEqual(['render-cpu', 'step-avg', 'step-p99']);
  });

  it('reports progress through warm-up, measurement and done, always moving forward', async () => {
    const seen: Progress[] = [];
    await run(world(), 4, (p) => seen.push(p));
    expect(seen[0]?.phase).toBe('waiting');
    expect(seen.at(-1)).toMatchObject({ phase: 'done', elapsedS: 7, totalS: 7 });
    const phases = seen.map((p) => p.phase);
    expect(phases.indexOf('warmup')).toBeLessThan(phases.indexOf('measure'));
    for (const phase of ['warmup', 'measure'] as const) {
      const elapsed = seen.filter((p) => p.phase === phase).map((p) => p.elapsedS);
      expect(elapsed).toEqual([...elapsed].sort((a, b) => a - b));
    }
  });

  it('gives up with SYS-120 when no statistics ever arrive', async () => {
    const w = world();
    // Never publishes: sleep still advances time, but there are no reports to read.
    const silent = {
      ...w,
      target: { stats: () => ({ sim: undefined, render: undefined }), resetRun: () => undefined },
    };
    await expect(run(silent)).rejects.toThrow(/SYS-120/);
  });

  it('produces JSON that survives a round trip and is recognised as a report', async () => {
    const report = await run(world());
    const copy: unknown = JSON.parse(JSON.stringify(report));
    expect(isPerfReport(copy)).toBe(true);
    expect(copy).toEqual(report);
  });
});

describe('isPerfReport', () => {
  it('rejects things that are not reports', () => {
    for (const junk of [
      null,
      5,
      'x',
      {},
      { schema: 'perf_report/1' },
      { schema: 'perf_report/2' },
    ]) {
      expect(isPerfReport(junk)).toBe(false);
    }
  });

  it('rejects a report with a damaged metric', async () => {
    const report = await run(world());
    const damaged = { ...report, metrics: { ...report.metrics, simHz: 'fast' } };
    expect(isPerfReport(damaged)).toBe(false);
  });
});

describe('gates', () => {
  const metrics = (over: Partial<PerfMetrics> = {}): PerfMetrics => ({
    windowS: 60,
    ticks: 3600,
    simHz: 60,
    droppedMs: 0,
    droppedEvents: 0,
    step: q(),
    frames: 3600,
    renderFps: 60,
    frameMs: q(),
    renderCpuMs: q({ avg: 1 }),
    staleFrames: 0,
    gappedFrames: 0,
    inputsDropped: 0,
    snapshotsDropped: 0,
    ...over,
  });

  it('accepts exactly the limits and rejects just past them', () => {
    const L = PS0_LIMITS;
    const edge = metrics({
      simHz: L.simHz + L.simHzTolerance,
      step: q({ p99: L.stepP99Ms, avg: L.stepAvgMs }),
      renderCpuMs: q({ avg: L.renderCpuAvgMs }),
    });
    expect(verdictOf(evaluateGates(edge))).toBe('pass');
    expect(verdictOf(evaluateGates(metrics({ simHz: 60.11 })))).toBe('fail');
    expect(verdictOf(evaluateGates(metrics({ simHz: 59.89 })))).toBe('fail');
    expect(verdictOf(evaluateGates(metrics({ droppedMs: 0.001 })))).toBe('fail');
    expect(verdictOf(evaluateGates(metrics({ step: q({ p99: 7.01 }) })))).toBe('fail');
    expect(verdictOf(evaluateGates(metrics({ step: q({ avg: 4.01 }) })))).toBe('fail');
    expect(verdictOf(evaluateGates(metrics({ renderCpuMs: q({ avg: 7.01 }) })))).toBe('fail');
  });

  it('does not pass on no data at all', () => {
    const none = q({ count: 0, avg: 0, p99: 0 });
    expect(verdictOf(evaluateGates(metrics({ step: none })))).toBe('fail');
    expect(verdictOf(evaluateGates(metrics({ renderCpuMs: none })))).toBe('fail');
  });
});

describe('scenarios and machine', () => {
  it('finds PS-0 by id, ignoring case', () => {
    expect(findScenario('ps-0')).toBe(PS0);
    expect(findScenario('PS-9')).toBeUndefined();
  });

  it('describes the machine from what the page can read', () => {
    const m = describeMachine(
      {
        navigator: {
          userAgent: 'UA',
          platform: 'MacIntel',
          hardwareConcurrency: 10,
          deviceMemory: 8,
        },
        screen: { width: 1728, height: 1117 },
        devicePixelRatio: 2,
      },
      'apple metal-3',
      'webgpu apple metal-3',
    );
    expect(m).toEqual({
      userAgent: 'UA',
      platform: 'MacIntel',
      hardwareConcurrency: 10,
      deviceMemoryGb: 8,
      screen: '1728x1117@2',
      adapter: 'apple metal-3',
      renderer: 'webgpu apple metal-3',
    });
  });

  it('says so when the page cannot tell', () => {
    const m = describeMachine({ navigator: { userAgent: 'UA' } }, '', '');
    expect([m.platform, m.hardwareConcurrency, m.deviceMemoryGb, m.screen]).toEqual([
      '',
      0,
      null,
      '',
    ]);
  });
});
