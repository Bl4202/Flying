import { describe, expect, it } from 'vitest';
import { memoryKv } from '@flying/platform-web/kv';
import { NullRenderer } from '@flying/render/null-renderer';
import {
  BENCHMARK_PLAN,
  GPU_HEIGHT,
  GPU_LEVELS,
  GPU_WIDTH,
  runBenchmark,
  type BenchmarkDeps,
  type BenchmarkStep,
} from './benchmark.ts';
import type { KernelInit, KernelWorkerLike } from './kernel.ts';
import { loadBenchmark, saveBenchmark, BENCHMARK_KEY } from './store.ts';
import { BENCHMARK_SCHEMA, isBenchmarkReport } from './types.ts';

/** A worker that achieves `rate` ticks per second and starts on the `go` message. */
function kernelWorker(rate: number, log: KernelInit[]): KernelWorkerLike {
  let init: KernelInit | undefined;
  const w: KernelWorkerLike = {
    onmessage: null,
    onerror: null,
    terminate: () => undefined,
    postMessage: (message) => {
      const m = message as { type: string };
      if (m.type === 'init') {
        init = message as KernelInit;
        log.push(init);
        queueMicrotask(() => w.onmessage?.({ data: { type: 'ready' } }));
      } else if (init !== undefined) {
        const ms = init.ms;
        w.onmessage?.({ data: { type: 'result', ticks: (rate * ms) / 1000, elapsedMs: ms } });
      }
    },
  };
  return w;
}

/** A renderer that costs a fixed fake time per patch, so the heavy level is 64 x `costPerPatch`. */
class CostedRenderer extends NullRenderer {
  disposedTimes = 0;
  pending = 0;
  readonly clock: { now: number };
  readonly costPerPatch: number;
  constructor(clock: { now: number }, costPerPatch: number) {
    super();
    this.clock = clock;
    this.costPerPatch = costPerPatch;
  }
  override render(frame: Parameters<NullRenderer['render']>[0]): void {
    super.render(frame);
    this.pending += frame.patchInstances * this.costPerPatch;
  }
  override finish(): Promise<void> {
    this.clock.now += this.pending;
    this.pending = 0;
    return Promise.resolve();
  }
  override dispose(): void {
    this.disposedTimes += 1;
    super.dispose();
  }
}

function deps(over: Partial<BenchmarkDeps> = {}) {
  const clock = { now: 0 };
  const kernelInits: KernelInit[] = [];
  const renderers: CostedRenderer[] = [];
  const steps: [BenchmarkStep, string][] = [];
  const base: BenchmarkDeps = {
    mode: 'quick',
    build: { id: 'abc', abi: 2, contentHash: '0123456789abcdef' },
    machine: {
      userAgent: 'test',
      platform: '',
      hardwareConcurrency: 10,
      deviceMemoryGb: 8,
      screen: '',
      adapter: '',
      renderer: 'null',
    },
    cores: 10,
    isolated: false,
    nowMs: () => clock.now,
    now: () => new Date('2026-10-13T12:00:00Z'),
    kernel: {
      variant: 'single',
      wasmUrl: 'sim.single.wasm',
      threadsInitialPages: 0,
      spawn: () => kernelWorker(300_000, kernelInits),
    },
    makeRenderer: () => {
      const r = new CostedRenderer(clock, 0.0125);
      renderers.push(r);
      return Promise.resolve(r);
    },
    memory: () =>
      Promise.resolve({
        deviceMemoryGb: 8,
        jsHeapLimitMb: 4096,
        jsHeapUsedMb: 12,
        measuredMb: null,
      }),
    storage: () => Promise.resolve({ available: true, writeMBps: 900, readMBps: 1500, note: '' }),
    onProgress: (step, detail) => steps.push([step, detail]),
    ...over,
  };
  return { deps: base, clock, kernelInits, renderers, steps };
}

describe('runBenchmark', () => {
  it('measures every part and classifies the machine', async () => {
    const t = deps();
    const report = await runBenchmark(t.deps);
    expect(isBenchmarkReport(report)).toBe(true);
    expect(report.schema).toBe(BENCHMARK_SCHEMA);
    expect(report.cpu).toMatchObject({ singleTicksPerSecond: 300_000, workers: 8, speedup: 8 });
    expect(report.gpu?.levels.map((l) => l.patches)).toEqual([...GPU_LEVELS]);
    expect(report.gpu?.levels.at(-1)?.msPerFrame).toBeCloseTo(0.8, 6);
    expect(report.gpu).toMatchObject({ width: GPU_WIDTH, height: GPU_HEIGHT });
    expect(report.storage.writeMBps).toBe(900);
    expect(report.notes).toEqual([]);
    expect(report.classification).toMatchObject({
      profile: 'high',
      preset: 'ultra',
      complete: true,
    });
    expect(report.startedAt).toBe('2026-10-13T12:00:00.000Z');
  });

  it('runs one worker and then as many as there are cores, up to eight', async () => {
    const t = deps({ cores: 10 });
    await runBenchmark(t.deps);
    // One worker, then eight (10 cores, capped).
    expect(t.kernelInits).toHaveLength(1 + 8);
    const few = deps({ cores: 2 });
    await runBenchmark(few.deps);
    expect(few.kernelInits).toHaveLength(1 + 2);
    const one = deps({ cores: 1 });
    const report = await runBenchmark(one.deps);
    expect(one.kernelInits).toHaveLength(1);
    expect(report.cpu?.speedup).toBe(1);
  });

  it('uses the time budgets of the chosen mode', async () => {
    const quick = deps({ mode: 'quick' });
    await runBenchmark(quick.deps);
    expect(quick.kernelInits[0]?.ms).toBe(BENCHMARK_PLAN.quick.kernelMs);
    const full = deps({ mode: 'full' });
    await runBenchmark(full.deps);
    expect(full.kernelInits[0]?.ms).toBe(BENCHMARK_PLAN.full.kernelMs);
    expect(BENCHMARK_PLAN.full.kernelMs).toBeGreaterThan(BENCHMARK_PLAN.quick.kernelMs);
  });

  it('reports progress through each part in order, ending with done', async () => {
    const t = deps();
    await runBenchmark(t.deps);
    const order = t.steps.map(([s]) => s).filter((s, i, a) => s !== a[i - 1]);
    expect(order).toEqual(['cpu-single', 'cpu-multi', 'gpu', 'memory', 'storage', 'done']);
    expect(t.steps.some(([s, d]) => s === 'gpu' && d.startsWith('64 patches'))).toBe(true);
  });

  it('disposes the renderer it made', async () => {
    const t = deps();
    await runBenchmark(t.deps);
    expect(t.renderers).toHaveLength(1);
    expect(t.renderers[0]?.disposedTimes).toBe(1);
  });

  it('carries on when a part fails: it is noted and left out of the verdict', async () => {
    const t = deps({
      makeRenderer: () => Promise.reject(new Error('SYS-103: no renderer could start')),
      storage: () => Promise.reject(new Error('disk on fire')),
      kernel: {
        variant: 'single',
        wasmUrl: 'x',
        threadsInitialPages: 0,
        spawn: () => {
          throw new Error('workers are blocked');
        },
      },
    });
    const report = await runBenchmark(t.deps);
    expect(report.cpu).toBeNull();
    expect(report.gpu).toBeNull();
    expect(report.notes).toHaveLength(3);
    expect(report.notes.join('\n')).toMatch(/CPU kernel skipped: workers are blocked/);
    expect(report.notes.join('\n')).toMatch(/GPU measurement skipped: SYS-103/);
    expect(report.notes.join('\n')).toMatch(/storage measurement skipped: disk on fire/);
    expect(report.classification.complete).toBe(false);
    expect(isBenchmarkReport(report)).toBe(true);
  });

  it('puts a storage note into the report notes', async () => {
    const t = deps({
      storage: () =>
        Promise.resolve({ available: false, writeMBps: null, readMBps: null, note: 'no OPFS' }),
    });
    const report = await runBenchmark(t.deps);
    expect(report.notes).toEqual(['storage: no OPFS']);
  });

  it('disposes the renderer even if measuring fails', async () => {
    const t = deps();
    const broken = new CostedRenderer({ now: 0 }, 1);
    broken.finish = () => Promise.reject(new Error('device lost'));
    t.deps.makeRenderer = () => Promise.resolve(broken);
    const report = await runBenchmark(t.deps);
    expect(report.gpu).toBeNull();
    expect(broken.disposedTimes).toBe(1);
    expect(report.notes[0]).toMatch(/device lost/);
  });

  it('classifies a slow GPU as holding the machine back', async () => {
    const t = deps({
      makeRenderer: () => Promise.resolve(new CostedRenderer({ now: 0 }, 0)),
    });
    // 64 patches at 1.05 ms each = 67 ms per frame: a software renderer.
    const clock = { now: 0 };
    t.deps.nowMs = () => clock.now;
    t.deps.makeRenderer = () => Promise.resolve(new CostedRenderer(clock, 1.05));
    const report = await runBenchmark(t.deps);
    expect(report.classification).toMatchObject({
      profile: 'below-minimum',
      limitedBy: 'gpu',
      preset: 'low',
    });
  });
});

describe('the stored report', () => {
  it('survives the key-value store', async () => {
    const kv = memoryKv();
    expect(await loadBenchmark(kv)).toBeUndefined();
    const report = await runBenchmark(deps().deps);
    await saveBenchmark(kv, report);
    expect(await loadBenchmark(kv)).toEqual(report);
  });

  it('ignores a stored value that is not a report', async () => {
    const kv = memoryKv();
    await kv.set(BENCHMARK_KEY, { schema: 'something-else' });
    expect(await loadBenchmark(kv)).toBeUndefined();
  });

  it('survives a store that throws', async () => {
    const broken = {
      backend: 'memory' as const,
      get: () => Promise.reject(new Error('no')),
      set: () => Promise.reject(new Error('no')),
      delete: () => Promise.reject(new Error('no')),
    };
    expect(await loadBenchmark(broken)).toBeUndefined();
    const report = await runBenchmark(deps().deps);
    await expect(saveBenchmark(broken, report)).resolves.toBeUndefined();
  });
});
