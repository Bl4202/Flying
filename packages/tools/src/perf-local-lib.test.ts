import { describe, expect, it } from 'vitest';
import {
  DEFAULT_BASE,
  firstPageQuery,
  judgeRun,
  parseLocalArgs,
  renderSummary,
  reportFile,
  scenarioQuery,
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
    scenario: 'PS-0',
    compatVerdict: 'ok',
    compatCodes: [],
    adapter: 'apple metal-3',
    renderer: 'webgpu apple metal-3',
    displayHz: 120,
    flight,
    perf: { verdict: 'pass', simHz: 60, renderFps: 119.9, failedGates: [], deviceLosses: 0 },
    benchmark: { profile: 'high', preset: 'ultra' },
    consoleErrors: [],
    flightWanted: true,
    latencyWanted: 0,
    latency: null,
    soak: null,
    ...over,
  };
}

describe('the dogfight run and the soak (v0.1d)', () => {
  it('reads --scenario combat (no scripted minute by default) and --soak', () => {
    const o = parseLocalArgs([
      '--name',
      'm4-combat',
      '--class',
      'recommended',
      '--scenario',
      'combat',
      '--soak',
    ]);
    expect([o.scenario, o.fly, o.soak]).toEqual(['combat', 0, true]);
    expect(parseLocalArgs(['--name', 'x', '--class', 'high']).soak).toBe(false);
    expect(() => parseLocalArgs(['--name', 'x', '--class', 'high', '--scenario', 'war'])).toThrow(
      /combat/,
    );
  });

  it('measures the control latency of the dogfight run with a duel running (PF2)', () => {
    const args = ['--name', 'x', '--class', 'recommended'];
    expect(firstPageQuery(parseLocalArgs([...args, '--scenario', 'combat']))).toBe(
      'debug=1&fly=1&spawn=air&scenario=merge&skill=2',
    );
    expect(firstPageQuery(parseLocalArgs([...args, '--scenario', 'arena']))).toBe(
      'debug=1&fly=1&spawn=air',
    );
    expect(firstPageQuery(parseLocalArgs([...args, '--spawn', 'runway']))).toBe(
      'debug=1&fly=1&spawn=runway',
    );
  });

  it('judges the soak when it ran, and says so in the summary', () => {
    expect(judgeRun(facts({ soak: 'fail' }), 'high').pass).toBe(false);
    expect(judgeRun(facts({ soak: 'pass' }), 'high').checks.map((c) => c.id)).toContain('soak');
    expect(judgeRun(facts(), 'high').checks.map((c) => c.id)).not.toContain('soak');
  });
});

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
      scenario: 'ps0',
      camera: undefined,
      res: undefined,
      quality: undefined,
      soak: false,
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
      out: 'docs/exit/v0.1d/machines',
      scenario: 'ps0',
      spawn: 'air',
      latency: 0,
    });
  });

  it('takes the arena scenario, its view, and skips the hands-on flight unless asked', () => {
    const base = ['--name', 'a', '--class', 'recommended', '--scenario', 'arena'];
    expect(parseLocalArgs(base)).toMatchObject({
      scenario: 'arena',
      fly: 0,
      camera: undefined,
      res: undefined,
      quality: undefined,
    });
    expect(
      parseLocalArgs([...base, '--camera', 'chase', '--res', '1920x1080', '--quality', 'medium']),
    ).toMatchObject({ camera: 'chase', res: '1920x1080', quality: 'medium' });
    expect(parseLocalArgs([...base, '--fly', '30']).fly).toBe(30);
    expect(() => parseLocalArgs([...base, '--camera', 'orbit'])).toThrow(/--camera/);
    expect(() => parseLocalArgs([...base, '--res', 'big'])).toThrow(/--res/);
    expect(() => parseLocalArgs([...base, '--quality', 'ultra'])).toThrow(/--quality/);
    expect(() => parseLocalArgs(['--name', 'a', '--class', 'high', '--scenario', 'ps9'])).toThrow(
      /--scenario/,
    );
  });

  it('builds the page query and the report file of the measured run from the scenario', () => {
    const ps0 = parseLocalArgs(['--name', 'a', '--class', 'high', '--seconds', '30']);
    expect(scenarioQuery(ps0)).toBe('perf=ps0&seconds=30');
    expect(reportFile(ps0)).toBe('perf-ps0.json');
    const arena = parseLocalArgs([
      '--name',
      'a',
      '--class',
      'high',
      '--scenario',
      'arena',
      '--camera',
      'chase',
      '--res',
      '1280x720',
    ]);
    expect(scenarioQuery(arena)).toBe('perf=arena&seconds=120&camera=chase&res=1280x720');
    expect(reportFile(arena)).toBe('perf-arena.json');
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
      facts({
        perf: {
          verdict: 'fail',
          simHz: 59.5,
          renderFps: 119,
          failedGates: ['sim-rate'],
          deviceLosses: 0,
        },
      }),
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
      perf: { verdict: 'pass', simHz: 60, renderFps: 34, failedGates: [], deviceLosses: 0 },
    });
    expect(judgeRun(slow, 'minimum').pass).toBe(true);
    expect(judgeRun(slow, 'recommended').pass).toBe(false);
    const at = facts({
      displayHz: 60,
      perf: { verdict: 'pass', simHz: 60, renderFps: 57, failedGates: [], deviceLosses: 0 },
    });
    expect(judgeRun(at, 'high').pass).toBe(true);
    expect(
      judgeRun(
        facts({
          displayHz: 60,
          perf: { verdict: 'pass', simHz: 60, renderFps: 29, failedGates: [], deviceLosses: 0 },
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

  it('names the arena scenario, what it looked through and its failed gates', () => {
    const arena = parseLocalArgs([
      '--name',
      'a-arena-chase',
      '--class',
      'recommended',
      '--scenario',
      'arena',
      '--camera',
      'chase',
    ]);
    const f = facts({
      scenario: 'ARENA',
      flight: null,
      flightWanted: false,
      perf: {
        verdict: 'fail',
        simHz: 60,
        renderFps: 52.4,
        failedGates: ['frame-avg', 'frame-p99'],
        deviceLosses: 0,
      },
    });
    const text = renderSummary(arena, f, judgeRun(f, 'recommended'), '2026-11-09');
    expect(text).toContain(
      'ARENA (120 s, chase, 1920x1080, high): FAIL, sim 60.000 Hz, render 52.4 FPS',
    );
    expect(text).toContain('ARENA strict gates hold (failed: frame-avg, frame-p99)');
    expect(text).toContain('`perf-arena.json`');
  });

  it('counts an arena run with no scripted minute as clean, and a lost device as not', () => {
    const arena = facts({ scenario: 'ARENA', flight: null, flightWanted: false });
    const j = judgeRun(arena, 'recommended');
    expect(j.checks.find((c) => c.id === 'clean')?.pass).toBe(true);
    const lost = facts({
      scenario: 'ARENA',
      flight: null,
      flightWanted: false,
      perf: { verdict: 'pass', simHz: 60, renderFps: 119, failedGates: [], deviceLosses: 1 },
    });
    expect(judgeRun(lost, 'recommended').checks.find((c) => c.id === 'clean')?.pass).toBe(false);
    // A scripted minute that was asked for and did not happen is not clean.
    expect(
      judgeRun(facts({ flight: null }), 'recommended').checks.find((c) => c.id === 'clean')?.pass,
    ).toBe(false);
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
