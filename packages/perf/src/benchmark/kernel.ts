/**
 * Runs the CPU kernel in one or several workers and adds up what they managed. The workers load
 * and warm up first and then start together, so the measured windows overlap.
 */
import type { Variant } from '@flying/platform-web/wasm-loader';

export interface KernelInit {
  type: 'init';
  variant: Variant;
  wasmUrl: string;
  threadsInitialPages: number;
  /** The content pack, so the kernel flies the real aircraft (a real tick). */
  contentUrl: string;
  /** How long to step, milliseconds. */
  ms: number;
  /** A shared word the workers wait on, or null to wait for a `go` message. */
  startFlag: SharedArrayBuffer | null;
}

export type KernelReply =
  | { type: 'ready' }
  | { type: 'result'; ticks: number; elapsedMs: number }
  | { type: 'error'; message: string };

export interface KernelWorkerLike {
  postMessage(message: unknown): void;
  terminate(): void;
  onmessage: ((event: { data: unknown }) => void) | null;
  onerror: ((event: { message?: string }) => void) | null;
}

export interface KernelRun {
  /** Ticks per second, summed over the workers. */
  ticksPerSecond: number;
  perWorker: number[];
}

export interface KernelOptions {
  workers: number;
  ms: number;
  variant: Variant;
  wasmUrl: string;
  threadsInitialPages: number;
  contentUrl: string;
  spawn(): KernelWorkerLike;
  /** Use a `SharedArrayBuffer` for the common start (only where the page is isolated). */
  sharedStart: boolean;
  /** Milliseconds to wait for the workers to load, and then for them to finish. */
  loadTimeoutMs?: number;
}

const DEFAULT_LOAD_TIMEOUT_MS = 20_000;

interface Deferred<T> {
  promise: Promise<T>;
  resolve(value: T): void;
  reject(error: Error): void;
}

function deferred<T>(): Deferred<T> {
  let resolve!: (value: T) => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  // Whichever of a worker's two promises nobody is waiting on any more must not become an
  // unhandled rejection; the one that is awaited still sees the failure.
  promise.catch(() => undefined);
  return { promise, resolve, reject };
}

export async function runKernel(opts: KernelOptions): Promise<KernelRun> {
  const flagBuffer = opts.sharedStart ? new SharedArrayBuffer(4) : null;
  const workers: KernelWorkerLike[] = [];
  try {
    const results = Array.from({ length: opts.workers }, () => ({
      ready: deferred<undefined>(),
      done: deferred<KernelReply & { type: 'result' }>(),
    }));
    const timeout = opts.loadTimeoutMs ?? DEFAULT_LOAD_TIMEOUT_MS;
    const fail = (i: number, message: string): void => {
      const err = new Error(message);
      results[i]?.ready.reject(err);
      results[i]?.done.reject(err);
    };

    for (let i = 0; i < opts.workers; i++) {
      const worker = opts.spawn();
      workers.push(worker);
      worker.onmessage = (event) => {
        const reply = event.data as KernelReply;
        if (reply.type === 'ready') results[i]?.ready.resolve(undefined);
        else if (reply.type === 'result') results[i]?.done.resolve(reply);
        else fail(i, reply.message);
      };
      worker.onerror = (event) => {
        fail(i, `SYS-033: a benchmark worker failed (${event.message ?? 'unknown error'})`);
      };
      const init: KernelInit = {
        type: 'init',
        variant: opts.variant,
        wasmUrl: opts.wasmUrl,
        contentUrl: opts.contentUrl,
        threadsInitialPages: opts.threadsInitialPages,
        ms: opts.ms,
        startFlag: flagBuffer,
      };
      worker.postMessage(init);
    }

    await withTimeout(
      Promise.all(results.map((r) => r.ready.promise)),
      timeout,
      'SYS-032: the benchmark workers did not become ready in time',
    );

    // Everyone is loaded: release them together.
    if (flagBuffer !== null) {
      const flag = new Int32Array(flagBuffer);
      Atomics.store(flag, 0, 1);
      Atomics.notify(flag, 0);
    } else {
      for (const w of workers) w.postMessage({ type: 'go' });
    }

    const replies = await withTimeout(
      Promise.all(results.map((r) => r.done.promise)),
      opts.ms + timeout,
      'SYS-032: the benchmark workers did not finish in time',
    );
    const perWorker = replies.map((r) => (r.ticks * 1000) / Math.max(r.elapsedMs, 1e-6));
    return { ticksPerSecond: perWorker.reduce((a, b) => a + b, 0), perWorker };
  } finally {
    for (const w of workers) w.terminate();
  }
}

function withTimeout<T>(promise: Promise<T>, ms: number, message: string): Promise<T> {
  return new Promise<T>((resolve, reject) => {
    const timer = setTimeout(() => {
      reject(new Error(message));
    }, ms);
    promise.then(
      (v) => {
        clearTimeout(timer);
        resolve(v);
      },
      (e: unknown) => {
        clearTimeout(timer);
        reject(e instanceof Error ? e : new Error(String(e)));
      },
    );
  });
}
