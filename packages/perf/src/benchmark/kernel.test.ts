import { describe, expect, it } from 'vitest';
import { runKernel, type KernelInit, type KernelWorkerLike } from './kernel.ts';

interface FakeOptions {
  /** Ticks per second each worker achieves. */
  rate: number;
  /** Never says `ready`. */
  silent?: boolean;
  /** Fails instead of becoming ready. */
  failWith?: string;
}

class FakeWorker implements KernelWorkerLike {
  onmessage: ((event: { data: unknown }) => void) | null = null;
  onerror: ((event: { message?: string }) => void) | null = null;
  terminated = false;
  init: KernelInit | undefined;
  readonly #opts: FakeOptions;
  readonly #log: string[];
  readonly id: number;
  constructor(opts: FakeOptions, log: string[], id: number) {
    this.#opts = opts;
    this.#log = log;
    this.id = id;
  }
  postMessage(message: unknown): void {
    const m = message as KernelInit | { type: 'go' };
    if (m.type === 'init') {
      this.init = m;
      queueMicrotask(() => {
        if (this.#opts.failWith !== undefined) {
          this.onmessage?.({ data: { type: 'error', message: this.#opts.failWith } });
        } else if (!this.#opts.silent) {
          this.#log.push(`ready ${String(this.id)}`);
          this.onmessage?.({ data: { type: 'ready' } });
        }
      });
    } else {
      this.#log.push(`go ${String(this.id)}`);
      this.finish();
    }
  }
  /** What a worker does on a shared flag: poll it, then run. */
  watchFlag(): void {
    const flag = new Int32Array(this.init?.startFlag ?? new SharedArrayBuffer(4));
    const poll = (): void => {
      if (Atomics.load(flag, 0) === 1) {
        this.#log.push(`flag ${String(this.id)}`);
        this.finish();
      } else {
        setTimeout(poll, 1);
      }
    };
    poll();
  }
  finish(): void {
    const ms = this.init?.ms ?? 0;
    this.onmessage?.({
      data: { type: 'result', ticks: (this.#opts.rate * ms) / 1000, elapsedMs: ms },
    });
  }
  terminate(): void {
    this.terminated = true;
  }
}

function rig(opts: FakeOptions) {
  const log: string[] = [];
  const workers: FakeWorker[] = [];
  const spawn = (): KernelWorkerLike => {
    const w = new FakeWorker(opts, log, workers.length);
    workers.push(w);
    return w;
  };
  return { log, workers, spawn };
}

const base = { variant: 'single', wasmUrl: 'x.wasm', threadsInitialPages: 0, ms: 1000 } as const;

describe('runKernel', () => {
  it('adds up the rates of the workers', async () => {
    const r = rig({ rate: 300_000 });
    const run = await runKernel({ ...base, workers: 4, spawn: r.spawn, sharedStart: false });
    expect(run.ticksPerSecond).toBe(1_200_000);
    expect(run.perWorker).toEqual([300_000, 300_000, 300_000, 300_000]);
  });

  it('releases the workers only once every one is ready (message start)', async () => {
    const r = rig({ rate: 1000 });
    await runKernel({ ...base, workers: 3, spawn: r.spawn, sharedStart: false });
    const firstGo = r.log.findIndex((l) => l.startsWith('go'));
    const lastReady = r.log.map((l) => l.startsWith('ready')).lastIndexOf(true);
    expect(lastReady).toBeLessThan(firstGo);
    expect(r.log.filter((l) => l.startsWith('go'))).toHaveLength(3);
  });

  it('starts on a shared flag when asked, with no go messages', async () => {
    const r = rig({ rate: 1000 });
    const pending = runKernel({ ...base, workers: 2, spawn: r.spawn, sharedStart: true });
    // Fake workers watch the flag the way a real worker waits on it.
    await new Promise((resolve) => setTimeout(resolve, 5));
    for (const w of r.workers) w.watchFlag();
    const run = await pending;
    expect(run.perWorker).toEqual([1000, 1000]);
    expect(r.log.some((l) => l.startsWith('go'))).toBe(false);
    expect(r.workers.every((w) => w.init?.startFlag instanceof SharedArrayBuffer)).toBe(true);
  });

  it('gives no flag to workers when the start is by message', async () => {
    const r = rig({ rate: 1000 });
    await runKernel({ ...base, workers: 1, spawn: r.spawn, sharedStart: false });
    expect(r.workers[0]?.init?.startFlag).toBeNull();
  });

  it('terminates every worker, on success and on failure', async () => {
    const ok = rig({ rate: 1000 });
    await runKernel({ ...base, workers: 3, spawn: ok.spawn, sharedStart: false });
    expect(ok.workers.every((w) => w.terminated)).toBe(true);

    const bad = rig({ rate: 1000, failWith: 'SYS-002: no memory' });
    await expect(
      runKernel({ ...base, workers: 3, spawn: bad.spawn, sharedStart: false }),
    ).rejects.toThrow(/SYS-002/);
    expect(bad.workers.every((w) => w.terminated)).toBe(true);
  });

  it('gives up with SYS-032 when a worker never becomes ready', async () => {
    const r = rig({ rate: 1000, silent: true });
    await expect(
      runKernel({ ...base, workers: 2, spawn: r.spawn, sharedStart: false, loadTimeoutMs: 20 }),
    ).rejects.toThrow(/SYS-032/);
    expect(r.workers.every((w) => w.terminated)).toBe(true);
  });

  it('reports a worker that fails to start', async () => {
    const r = rig({ rate: 1000, silent: true });
    const pending = runKernel({ ...base, workers: 1, spawn: r.spawn, sharedStart: false });
    queueMicrotask(() => {
      r.workers[0]?.onerror?.({ message: 'boom' });
    });
    await expect(pending).rejects.toThrow(/SYS-033.*boom/);
  });
});
