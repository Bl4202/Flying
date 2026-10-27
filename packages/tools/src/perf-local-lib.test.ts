import { describe, expect, it } from 'vitest';
import {
  DEFAULT_BASE,
  judgeRun,
  parseLocalArgs,
  renderSummary,
  type FlightSummary,
  type LatencySummary,
  type LocalOptions,
  type RunFacts,
} from './perf-local-lib.ts';

const flight: FlightSummary = {
  seconds: 60,
  fpsAvg: 119.9,
  fpsMin: 110,
  frameP99Ms: 9,
  frameMaxMs: 14,
  simHzAvg: 60,
  toasts: [],
};

function facts(over: Partial<RunFacts> = {}): RunFacts {
  return {
    compatVerdict: 'ok',
    compatCodes: [],
    adapter: 'apple metal-3',
    renderer: 'webgpu apple metal-3',
    displayHz: 120,
    flight,
    perf: { verdict: 'pass', simHz: 60, renderFps: 119.9, failedGates: [] },
    benchmark: { profile: 'high', preset: 'ultra' },
    consoleErrors: [],
    latencyWanted: 0,
    latency: null,
    ...over,
  };
}

describe('parseLocalArgs', () => {
  it('reads a full command line', () => {
    const o = parseLocalArgs([
      '--name',
      'm4-webgl2-throttled',
      '--class',
      'minimum',
      '--renderer',
      'webgl2',
      '--throttle',
      '4',
      '--seconds',
      '120',
      '--fly',
      '30',
      '--headless',
      '--base',
      'http://127.0.0.1:4173/Flying',
      '--out',
      'somewhere',
      '--spawn',
      'runway',
      '--latency',
      '500',
    ]);
    expect(o).toEqual({
      name: 'm4-webgl2-throttled',
      base: 'http://127.0.0.1:4173/Flying/',
      renderer: 'webgl2',
      throttle: 4,
      seconds: 120,
      fly: 30,
      headless: true,
      out: 'somewhere',
      machineClass: 'minimum',
      spawn: 'runway',
      latency: 500,
    });
  });

  it('has defaults for everything but the name and the class', () => {
    const o = parseLocalArgs(['--name', 'm4', '--class', 'recommended']);
    expect(o).toMatchObject({
      base: DEFAULT_BASE,
      renderer: 'auto',
      throttle: 1,
      seconds: 120,
      fly: 60,
      headless: false,
      out: 'docs/exit/v0.1b/machines',
      spawn: 'air',
      latency: 0,
    });
  });

  it('refuses what it does not understand', () => {
    expect(() => parseLocalArgs([])).toThrow(/--name is required/);
    expect(() => parseLocalArgs(['--name', 'x'])).toThrow(/--class is required/);
    expect(() => parseLocalArgs(['--name', 'Bad Name', '--class', 'high'])).toThrow(/--name/);
    expect(() => parseLocalArgs(['--name', 'x', '--class', 'huge'])).toThrow(/--class/);
    expect(() =>
      parseLocalArgs(['--name', 'x', '--class', 'high', '--renderer', 'vulkan']),
    ).toThrow(/--renderer/);
    expect(() => parseLocalArgs(['--name', 'x', '--class', 'high', '--throttle', '0'])).toThrow(
      /--throttle/,
    );
    expect(() => parseLocalArgs(['--name', 'x', '--class', 'high', '--seconds'])).toThrow(
      /needs a value/,
    );
    expect(() => parseLocalArgs(['--name', 'x', '--class', 'high', '--bogus'])).toThrow(/unknown/);
    expect(() => parseLocalArgs(['--name', 'x', '--class', 'high', '--spawn', 'sea'])).toThrow(
      /--spawn/,
    );
    expect(() => parseLocalArgs(['--name', 'x', '--class', 'high', '--latency', '0'])).toThrow(
      /--latency/,
    );
  });
});

describe('judgeRun', () => {
  it('passes a clean run at the display rate', () => {
    const j = judgeRun(facts(), 'recommended');
    expect(j.pass).toBe(true);
    expect(j.checks.map((c) => c.id)).toEqual(['compat', 'ps0', 'clean', 'fps']);
  });

  it('fails a fatal compat verdict', () => {
    expect(judgeRun(facts({ compatVerdict: 'fatal' }), 'high').pass).toBe(false);
    // A degraded verdict is allowed, and so is no screen at all (already acknowledged).
    expect(judgeRun(facts({ compatVerdict: 'degraded' }), 'high').pass).toBe(true);
    expect(judgeRun(facts({ compatVerdict: null }), 'high').pass).toBe(true);
  });

  it('fails when a PS-0 gate fails, and names it', () => {
    const j = judgeRun(
      facts({ perf: { verdict: 'fail', simHz: 59.5, renderFps: 119, failedGates: ['sim-rate'] } }),
      'high',
    );
    expect(j.pass).toBe(false);
    expect(j.checks.find((c) => c.id === 'ps0')?.text).toContain('sim-rate');
    expect(judgeRun(facts({ perf: null }), 'high').pass).toBe(false);
  });

  it('fails on console errors, a lost device, or a flight that did not happen', () => {
    expect(judgeRun(facts({ consoleErrors: ['boom'] }), 'high').pass).toBe(false);
    expect(
      judgeRun(
        facts({ flight: { ...flight, toasts: ['Warning: The graphics device was lost: x'] } }),
        'high',
      ).pass,
    ).toBe(false);
    expect(judgeRun(facts({ flight: null }), 'high').pass).toBe(false);
    // Other notices are fine.
    expect(
      judgeRun(facts({ flight: { ...flight, toasts: ['Notice: Paused'] } }), 'high').pass,
    ).toBe(true);
  });

  it('judges the control latency only when it was asked for, strictly at 25 ms p95', () => {
    expect(judgeRun(facts(), 'high').checks.map((c) => c.id)).not.toContain('latency');
    const measured = (over: Partial<LatencySummary> = {}): RunFacts =>
      facts({
        latencyWanted: 500,
        latency: { count: 612, avg: 9.3, p95: 21, p99: 22.8, max: 24.5, ...over },
      });
    const ok = judgeRun(measured(), 'high');
    expect(ok.pass).toBe(true);
    expect(ok.checks.find((c) => c.id === 'latency')?.text).toContain('p95 21.00 ms');
    expect(judgeRun(measured({ p95: 25 }), 'high').pass).toBe(true);
    expect(judgeRun(measured({ p95: 25.01 }), 'high').pass).toBe(false);
    expect(judgeRun(measured({ count: 499 }), 'high').pass).toBe(false);
    expect(judgeRun(facts({ latencyWanted: 500, latency: null }), 'high').pass).toBe(false);
  });

  it('asks 30 FPS of a Minimum-class run and the display rate of the others', () => {
    const slow = facts({
      displayHz: 60,
      perf: { verdict: 'pass', simHz: 60, renderFps: 34, failedGates: [] },
    });
    expect(judgeRun(slow, 'minimum').pass).toBe(true);
    expect(judgeRun(slow, 'recommended').pass).toBe(false);
    const at = facts({
      displayHz: 60,
      perf: { verdict: 'pass', simHz: 60, renderFps: 57, failedGates: [] },
    });
    expect(judgeRun(at, 'high').pass).toBe(true);
    expect(
      judgeRun(
        facts({
          displayHz: 60,
          perf: { verdict: 'pass', simHz: 60, renderFps: 29, failedGates: [] },
        }),
        'minimum',
      ).pass,
    ).toBe(false);
  });
});

describe('renderSummary', () => {
  const opts: LocalOptions = parseLocalArgs(['--name', 'm4-webgpu', '--class', 'recommended']);

  it('states the verdict, the figures and each criterion', () => {
    const f = facts();
    const text = renderSummary(opts, f, judgeRun(f, 'recommended'), '2026-10-14');
    expect(text).toContain('# Run m4-webgpu');
    expect(text).toContain('## Pass criteria: PASS');
    expect(text).toContain('PS-0 (120 s): PASS, sim 60.000 Hz');
    expect(text).toContain('profile high, preset ultra');
    expect(text).toContain('- [x] compat verdict is not fatal (ok)');
    expect(text).toContain('Adapter: apple metal-3');
  });

  it('lists failures, errors and notices', () => {
    const f = facts({
      consoleErrors: ['pageerror: boom'],
      flight: { ...flight, toasts: ['Warning: The graphics device was lost: x'] },
      compatCodes: ['SYS-103'],
    });
    const text = renderSummary(opts, f, judgeRun(f, 'recommended'), '2026-10-14');
    expect(text).toContain('## Pass criteria: FAIL');
    expect(text).toContain('- [ ] frames render without errors or device loss');
    expect(text).toContain('## Console errors');
    expect(text).toContain('pageerror: boom');
    expect(text).toContain('## Notices shown during the flight');
    expect(text).toContain('SYS-103');
  });

  it('reports the control latency, or that it was not measured', () => {
    const asked = parseLocalArgs(['--name', 'l', '--class', 'high', '--latency', '500']);
    const f = facts({
      latencyWanted: 500,
      latency: { count: 612, avg: 9.3, p95: 21, p99: 22.8, max: 24.5 },
    });
    const text = renderSummary(asked, f, judgeRun(f, 'high'), 'x');
    expect(text).toContain(
      'Control latency (612 changes): avg 9.30 ms, p95 21.00 ms, p99 22.80 ms, max 24.50 ms, at 120 Hz',
    );
    expect(text).toContain('- [x] control latency p95 21.00 ms within 25 ms, over 612 changes');
    expect(text).toContain('air start');
    const g = facts({ latencyWanted: 500 });
    expect(renderSummary(asked, g, judgeRun(g, 'high'), 'x')).toContain(
      'Control latency: did not finish',
    );
    expect(renderSummary(opts, facts(), judgeRun(facts(), 'high'), 'x')).toContain(
      'Control latency: not measured',
    );
  });

  it('says so when a part did not run, and notes throttling', () => {
    const o = parseLocalArgs(['--name', 'b', '--class', 'minimum', '--throttle', '4']);
    const f = facts({ flight: null, perf: null, benchmark: null });
    const text = renderSummary(o, f, judgeRun(f, 'minimum'), 'x');
    expect(text).toContain('Flight: not run');
    expect(text).toContain('PS-0: did not finish');
    expect(text).toContain('Benchmark: not run');
    expect(text).toContain('4x (main thread only');
  });
});
