/**
 * The main thread's side of the Sim Worker: spawns it, sends commands, receives reports.
 *
 * The worker is created through `scriptUrl` (Trusted Types), initialised with one message, and
 * `ready` resolves when it has loaded the module and the content pack. Everything after that is
 * fire-and-forget commands plus the 4 Hz `stats` events; only `requestHash` has a reply.
 */
import { scriptUrl, type Variant } from '@flying/platform-web';
import {
  inputTransferables,
  parseSimEvent,
  snapshotTransferables,
  type SimCommand,
  type SimEvent,
  type SimInit,
  type SimReady,
  type SimStats,
} from '@flying/sim-bridge';
import { wakeSleeper } from './sleeper.ts';

/** The slice of `Worker` the client uses; tests supply a fake. */
export interface WorkerLike {
  postMessage(message: unknown, transfer?: Transferable[]): void;
  terminate(): void;
  onmessage: ((event: { data: unknown }) => void) | null;
  onerror: ((event: { message?: string }) => void) | null;
}

export interface SimClientOptions {
  /** URL of `sim.worker.ts` as built (from the bundler). */
  workerUrl: string;
  init: Omit<SimInit, 'type'>;
  onStats?: (stats: SimStats) => void;
  onState?: (coreState: number) => void;
  /** Called for worker errors after `ready` (before it, `ready` rejects instead). */
  onError?: (message: string) => void;
  /** Milliseconds to wait for `ready`. */
  readyTimeoutMs?: number;
  /** Replaces `new Worker(...)`, for tests. */
  createWorker?: (url: string) => WorkerLike;
}

export interface HashReport {
  tick: number;
  hashes: Record<string, string>;
}

export class SimClient {
  readonly ready: SimReady;
  /** When the last `stats` event arrived, on the main thread's `performance.now()` clock. */
  lastStatsAt = 0;
  readonly #worker: WorkerLike;
  readonly #wake: SharedArrayBuffer | null;
  readonly #pending = new Map<number, (r: HashReport) => void>();
  #nextId = 1;
  #opts: SimClientOptions;

  private constructor(worker: WorkerLike, ready: SimReady, opts: SimClientOptions) {
    this.#worker = worker;
    this.ready = ready;
    this.#opts = opts;
    this.#wake = opts.init.wake;
    worker.onmessage = (event) => {
      this.#receive(event.data);
    };
    worker.onerror = (event) => {
      opts.onError?.(`worker failed: ${event.message ?? 'unknown error'}`);
    };
  }

  /** Spawns the worker and waits for it to be ready. */
  static async create(opts: SimClientOptions): Promise<SimClient> {
    const worker =
      opts.createWorker?.(opts.workerUrl) ??
      (new Worker(scriptUrl(opts.workerUrl), { type: 'module' }) as unknown as WorkerLike);

    const ready = await new Promise<SimReady>((resolve, reject) => {
      const timer = setTimeout(() => {
        reject(new Error(`SYS-032: the Sim Worker did not become ready in time`));
      }, opts.readyTimeoutMs ?? 15_000);
      const fail = (message: string): void => {
        clearTimeout(timer);
        reject(new Error(message));
      };
      worker.onmessage = (event) => {
        try {
          const e = parseSimEvent(event.data);
          if (e.type === 'ready') {
            clearTimeout(timer);
            resolve(e);
          } else if (e.type === 'error') {
            fail(e.message);
          }
        } catch (err) {
          fail(err instanceof Error ? err.message : String(err));
        }
      };
      worker.onerror = (event) => {
        fail(`SYS-033: the Sim Worker failed to start (${event.message ?? 'unknown error'})`);
      };
      const init: SimInit = { type: 'init', ...opts.init };
      const transfer: Transferable[] = [
        ...snapshotTransferables(init.snapshot),
        ...inputTransferables(init.input),
        ...(init.terrainPort ? [init.terrainPort] : []),
      ];
      worker.postMessage(init, transfer);
    }).catch((e: unknown) => {
      worker.terminate();
      throw e;
    });
    return new SimClient(worker, ready, opts);
  }

  get variant(): Variant {
    return this.ready.variant;
  }

  #receive(data: unknown): void {
    let event: SimEvent;
    try {
      event = parseSimEvent(data);
    } catch (e) {
      this.#opts.onError?.(e instanceof Error ? e.message : String(e));
      return;
    }
    switch (event.type) {
      case 'stats':
        this.lastStatsAt = performance.now();
        this.#opts.onStats?.(event.stats);
        break;
      case 'state':
        this.#opts.onState?.(event.coreState);
        break;
      case 'hash': {
        const resolve = this.#pending.get(event.id);
        this.#pending.delete(event.id);
        resolve?.({ tick: event.tick, hashes: event.hashes });
        break;
      }
      case 'error':
        this.#opts.onError?.(event.message);
        break;
      case 'ready':
        break;
    }
  }

  /** Sends a command and wakes the worker if it is sleeping in `Atomics.wait`. */
  #send(command: SimCommand): void {
    this.#worker.postMessage(command);
    if (this.#wake !== null) {
      wakeSleeper(this.#wake);
    }
  }

  start(): void {
    this.#send({ type: 'start' });
  }
  pause(): void {
    this.#send({ type: 'pause' });
  }
  resume(): void {
    this.#send({ type: 'resume' });
  }
  setProfiling(on: boolean): void {
    this.#send({ type: 'profiling', on });
  }
  resetRun(): void {
    this.#send({ type: 'reset-run' });
  }
  /** Blocks the worker for `ms` (test hook; ignored unless the init allowed it). */
  stall(ms: number): void {
    this.#send({ type: 'stall', ms });
  }

  /** Asks for the state hashes (all columns) and the tick they were taken at. */
  requestHash(): Promise<HashReport> {
    const id = this.#nextId++;
    return new Promise<HashReport>((resolve) => {
      this.#pending.set(id, resolve);
      this.#send({ type: 'hash', id });
    });
  }

  /** Stops the loop and ends the worker. */
  stop(): void {
    this.#send({ type: 'stop' });
    this.#worker.terminate();
  }
}
