// @vitest-environment happy-dom
import { describe, expect, it, vi } from 'vitest';
import {
  BENCHMARK_SCHEMA,
  classify,
  PERF_REPORT_SCHEMA,
  type BenchmarkReport,
  type ClassifyInput,
  type PerfReport,
} from '@flying/perf';
import { explainClassification, mountBenchmarkPanel } from './benchmark-panel.ts';
import { mountPerfPanel } from './perf-panel.ts';
import { reportFileName, type ReportIo } from './report-io.ts';

const q = { count: 100, avg: 0.5, p50: 0.4, p95: 0.9, p99: 1.2, max: 2 };

const byId = (el: ParentNode, id: string): HTMLElement | null =>
  el.querySelector<HTMLElement>(`[data-testid="${id}"]`);

function io(copyResult = true): ReportIo & { copied: string[]; downloads: [string, string][] } {
  const copied: string[] = [];
  const downloads: [string, string][] = [];
  return {
    copied,
    downloads,
    copy: (text) => {
      copied.push(text);
      return Promise.resolve(copyResult);
    },
    download: (name, text) => {
      downloads.push([name, text]);
    },
  };
}

function perfReport(over: Partial<PerfReport> = {}): PerfReport {
  return {
    schema: PERF_REPORT_SCHEMA,
    scenario: 'PS-0',
    startedAt: '2026-10-13T12:00:00.000Z',
    warmupS: 3,
    seconds: 60,
    build: { id: 'abc', abi: 2, contentHash: '0123456789abcdef' },
    machine: {
      userAgent: 'UA',
      platform: '',
      hardwareConcurrency: 10,
      deviceMemoryGb: 8,
      screen: '',
      adapter: '',
      renderer: 'webgl2',
    },
    modes: {
      variant: 'threads',
      snapshotChannel: 'shared',
      inputChannel: 'shared',
      renderHost: 'worker',
      sleeper: 'atomics',
      renderer: 'webgl2',
    },
    metrics: {
      windowS: 60,
      ticks: 3600,
      simHz: 60,
      droppedMs: 0,
      droppedEvents: 0,
      step: q,
      frames: 3600,
      renderFps: 60,
      frameMs: q,
      renderCpuMs: q,
      staleFrames: 0,
      gappedFrames: 0,
      inputsDropped: 0,
      snapshotsDropped: 0,
    },
    gates: [
      { id: 'sim-rate', rule: 'sim rate 60 ± 0.1 Hz', value: 60, limit: 60, pass: true },
      { id: 'dropped', rule: 'no time dropped', value: 0, limit: 0, pass: true },
    ],
    verdict: 'pass',
    ...over,
  };
}

function root(): HTMLElement {
  const el = document.createElement('div');
  document.body.append(el);
  return el;
}

describe('perf panel', () => {
  it('shows progress through the phases', () => {
    const r = root();
    const panel = mountPerfPanel(r, { scenario: 'PS-0', io: io() });
    expect(byId(r, 'perf-status')?.textContent).toContain('Waiting');
    panel.progress({ phase: 'warmup', elapsedS: 2, totalS: 63 });
    expect(byId(r, 'perf-status')?.textContent).toBe('Warm-up: 2 of 63 s');
    panel.progress({ phase: 'measure', elapsedS: 10, totalS: 63 });
    expect(byId(r, 'perf-status')?.textContent).toBe('Measuring: 10 of 63 s');
    expect(byId(r, 'perf-status')?.dataset['state']).toBe('running');
    expect((byId(r, 'perf-copy') as HTMLButtonElement).disabled).toBe(true);
  });

  it('shows the report as JSON with its gates, and enables Copy and Download', async () => {
    const r = root();
    const sink = io();
    const panel = mountPerfPanel(r, { scenario: 'PS-0', io: sink });
    const report = perfReport();
    panel.done(report);

    expect(byId(r, 'perf-status')?.dataset['state']).toBe('done');
    expect(byId(r, 'perf-verdict')?.textContent).toBe('PASS: every gate holds.');
    expect(r.querySelectorAll('[data-gate]')).toHaveLength(2);
    expect(JSON.parse(byId(r, 'perf-report')?.textContent ?? '')).toEqual(report);

    byId(r, 'perf-copy')?.click();
    await vi.waitFor(() => {
      expect(byId(r, 'perf-feedback')?.textContent).toBe('Copied.');
    });
    expect(sink.copied[0]).toBe(JSON.stringify(report, null, 2));

    byId(r, 'perf-download')?.click();
    expect(sink.downloads[0]?.[0]).toBe('perf-ps-0-2026-10-13t12-00-00.json');
  });

  it('says how many gates failed', () => {
    const r = root();
    const panel = mountPerfPanel(r, { scenario: 'PS-0', io: io() });
    panel.done(
      perfReport({
        verdict: 'fail',
        gates: [
          { id: 'sim-rate', rule: 'sim rate', value: 59, limit: 60, pass: false },
          { id: 'dropped', rule: 'dropped', value: 0, limit: 0, pass: true },
        ],
      }),
    );
    expect(byId(r, 'perf-verdict')?.textContent).toBe('FAIL: 1 of 2 gates do not hold.');
    expect(r.querySelector('[data-gate="sim-rate"]')?.getAttribute('data-pass')).toBe('false');
  });

  it('tells the person to copy by hand when the clipboard refuses', async () => {
    const r = root();
    const panel = mountPerfPanel(r, { scenario: 'PS-0', io: io(false) });
    panel.done(perfReport());
    byId(r, 'perf-copy')?.click();
    await vi.waitFor(() => {
      expect(byId(r, 'perf-feedback')?.textContent).toContain('by hand');
    });
  });

  it('shows a failed run, and a Back button only when there is somewhere to go back to', () => {
    const r = root();
    const onBack = vi.fn();
    const panel = mountPerfPanel(r, { scenario: 'PS-0', io: io(), onBack });
    panel.fail('SYS-120: no statistics arrived');
    expect(byId(r, 'perf-status')?.dataset['state']).toBe('error');
    expect(byId(r, 'perf-status')?.textContent).toContain('SYS-120');
    byId(r, 'perf-back')?.click();
    expect(onBack).toHaveBeenCalledTimes(1);

    const r2 = root();
    mountPerfPanel(r2, { scenario: 'PS-0', io: io() });
    expect(byId(r2, 'perf-back')).toBeNull();
  });

  it('puts report text in as text, never as markup', () => {
    const r = root();
    const panel = mountPerfPanel(r, { scenario: 'PS-0', io: io() });
    const report = perfReport();
    report.machine.userAgent = '<img src=x onerror=alert(1)>';
    panel.done(report);
    expect(r.querySelector('img')).toBeNull();
  });
});

const STRONG: ClassifyInput = {
  cpu: { singleTicksPerSecond: 350_000, workers: 8, multiTicksPerSecond: 2_100_000, speedup: 6 },
  gpuHeavyMs: 0.8,
  gpuBackend: 'webgpu',
  cores: 10,
  memory: { deviceMemoryGb: 8 },
};

function benchReport(input: ClassifyInput = STRONG, notes: string[] = []): BenchmarkReport {
  return {
    schema: BENCHMARK_SCHEMA,
    mode: 'quick',
    startedAt: '2026-10-13T12:00:00.000Z',
    durationS: 9,
    build: { id: 'abc', abi: 2, contentHash: '0123456789abcdef' },
    machine: {
      userAgent: 'UA',
      platform: '',
      hardwareConcurrency: 10,
      deviceMemoryGb: 8,
      screen: '',
      adapter: '',
      renderer: 'webgpu',
    },
    cpu: input.cpu,
    gpu: null,
    memory: { deviceMemoryGb: 8, jsHeapLimitMb: null, jsHeapUsedMb: null, measuredMb: null },
    storage: { available: true, writeMBps: 900, readMBps: 1500, note: '' },
    classification: classify(input),
    notes,
  };
}

describe('explainClassification', () => {
  it('says nothing held a top machine back, with the figures', () => {
    const lines = explainClassification(classify(STRONG));
    expect(lines[0]).toBe('Ultra preset on High hardware.');
    expect(lines[1]).toContain('Nothing held it back');
    expect(lines[1]).toContain('0.8 ms');
    expect(lines[1]).toContain('350 thousand');
    expect(lines[1]).toContain('10 logical processors');
    expect(lines).toHaveLength(2);
  });

  it('names the weakest part', () => {
    const slowGpu = explainClassification(classify({ ...STRONG, gpuHeavyMs: 11.2 }));
    expect(slowGpu[1]).toContain('11.2 ms');
    expect(slowGpu[1]).toContain('weakest part');
    const fewCores = explainClassification(classify({ ...STRONG, cores: 4 }));
    expect(fewCores[1]).toBe('The 4 logical processors are the weakest part.');
    const lowMemory = explainClassification(classify({ ...STRONG, memory: { deviceMemoryGb: 4 } }));
    expect(lowMemory[1]).toBe('4 GB of memory is the weakest part.');
    const slowCpu = explainClassification(
      classify({
        ...STRONG,
        cpu: {
          ...(STRONG.cpu as NonNullable<ClassifyInput['cpu']>),
          singleTicksPerSecond: 150_000,
        },
      }),
    );
    expect(slowCpu[1]).toContain('150 thousand');
  });

  it('warns when a part was not measured', () => {
    const lines = explainClassification(classify({ ...STRONG, gpuHeavyMs: null }));
    expect(lines.at(-1)).toContain('too generous');
  });
});

describe('benchmark panel', () => {
  it('shows progress, naming the step and its detail', () => {
    const r = root();
    const panel = mountBenchmarkPanel(r, {
      seconds: 25,
      io: io(),
      onAgain: vi.fn(),
      onBack: vi.fn(),
    });
    expect(byId(r, 'bench-status')?.textContent).toContain('25 seconds');
    panel.step('gpu', '16 patches: 0.22 ms');
    expect(byId(r, 'bench-status')?.textContent).toBe('Graphics… 16 patches: 0.22 ms');
    panel.step('cpu-single', '');
    expect(byId(r, 'bench-status')?.textContent).toBe('One processor thread…');
  });

  it('shows the profile, preset, reason and parts, in words', () => {
    const r = root();
    const panel = mountBenchmarkPanel(r, {
      seconds: 25,
      io: io(),
      onAgain: vi.fn(),
      onBack: vi.fn(),
    });
    panel.done(benchReport({ ...STRONG, gpuHeavyMs: 3 }));
    expect(byId(r, 'bench-profile')?.textContent).toBe('Hardware profile: Recommended');
    expect(byId(r, 'bench-profile')?.dataset['profile']).toBe('recommended');
    expect(byId(r, 'bench-preset')?.textContent).toBe('Suggested preset: High');
    expect(byId(r, 'bench-explanation')?.textContent).toContain('weakest part');
    const parts = [...r.querySelectorAll('[data-part]')].map((e) => e.textContent);
    expect(parts).toEqual([
      'Processor: High',
      'Graphics: Recommended',
      'Logical processors: High',
      'Memory: High',
    ]);
  });

  it('shows unmeasured parts and the notes about them', () => {
    const r = root();
    const panel = mountBenchmarkPanel(r, {
      seconds: 25,
      io: io(),
      onAgain: vi.fn(),
      onBack: vi.fn(),
    });
    panel.done(benchReport({ ...STRONG, gpuHeavyMs: null }, ['GPU measurement skipped: SYS-103']));
    expect(r.querySelector('[data-part="gpu"]')?.textContent).toBe('Graphics: not measured');
    expect(byId(r, 'bench-notes')?.textContent).toContain('SYS-103');
  });

  it('marks a result loaded from storage, and offers Copy, Download, Run again and Back', async () => {
    const r = root();
    const sink = io();
    const onAgain = vi.fn();
    const onBack = vi.fn();
    const panel = mountBenchmarkPanel(r, { seconds: 25, io: sink, onAgain, onBack });
    expect((byId(r, 'bench-again') as HTMLButtonElement).disabled).toBe(true);
    const report = benchReport();
    panel.done(report, '13 Oct 2026');
    expect(byId(r, 'bench-status')?.textContent).toBe(
      'Showing the result stored from 13 Oct 2026.',
    );
    expect(JSON.parse(byId(r, 'bench-report')?.textContent ?? '')).toEqual(report);

    byId(r, 'bench-copy')?.click();
    await vi.waitFor(() => {
      expect(byId(r, 'bench-feedback')?.textContent).toBe('Copied.');
    });
    byId(r, 'bench-download')?.click();
    expect(sink.downloads[0]?.[0]).toBe('benchmark-2026-10-13t12-00-00.json');
    byId(r, 'bench-again')?.click();
    byId(r, 'bench-back')?.click();
    expect([onAgain.mock.calls.length, onBack.mock.calls.length]).toEqual([1, 1]);
  });

  it('shows a failure and lets the person run again', () => {
    const r = root();
    const panel = mountBenchmarkPanel(r, {
      seconds: 25,
      io: io(),
      onAgain: vi.fn(),
      onBack: vi.fn(),
    });
    panel.fail('workers are blocked');
    expect(byId(r, 'bench-status')?.dataset['state']).toBe('error');
    expect(byId(r, 'bench-status')?.textContent).toBe('The benchmark failed: workers are blocked');
    expect((byId(r, 'bench-again') as HTMLButtonElement).disabled).toBe(false);
  });
});

describe('reportFileName', () => {
  it('is lower case and safe in every file system', () => {
    expect(reportFileName('perf-PS-0', '2026-10-13T12:00:00.123Z')).toBe(
      'perf-ps-0-2026-10-13t12-00-00.json',
    );
    expect(reportFileName('benchmark', '2026-10-13T12:00:00.000Z')).not.toMatch(/[:/\\ ]/);
  });
});
