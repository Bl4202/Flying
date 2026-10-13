/**
 * How the Sim Worker waits for the next tick.
 *
 * * `AtomicsSleeper` blocks the worker in `Atomics.wait` with a timeout, which is precise (a
 *   fraction of a millisecond) and costs nothing while waiting. It needs a `SharedArrayBuffer`, so
 *   only the cross-origin isolated mode has it. Blocking means control messages wait until the
 *   sleep ends (at most one tick), unless the main thread bumps the shared word and wakes it.
 * * `TimerSleeper` uses `scheduler.postTask` with a delay where it exists, else `setTimeout`.
 *   Browsers round these to a millisecond or more, so ticks start a little late; the fixed-step
 *   clock corrects the rate, and what remains is jitter.
 */

export interface Sleeper {
  readonly kind: 'atomics' | 'timer';
  /** Waits about `ms` milliseconds (returns at once for zero or less). */
  sleep(ms: number): Promise<void> | void;
}

/** One 32-bit word the main thread bumps to wake the worker. */
export function createWakeWord(): SharedArrayBuffer {
  return new SharedArrayBuffer(4);
}

/** Wakes a worker sleeping in an `AtomicsSleeper` on `wake`. Safe to call from any thread. */
export function wakeSleeper(wake: SharedArrayBuffer): void {
  const word = new Int32Array(wake);
  Atomics.add(word, 0, 1);
  Atomics.notify(word, 0);
}

export class AtomicsSleeper implements Sleeper {
  readonly kind = 'atomics';
  readonly #word: Int32Array;
  #seen: number;

  constructor(wake: SharedArrayBuffer) {
    this.#word = new Int32Array(wake);
    this.#seen = Atomics.load(this.#word, 0);
  }

  sleep(ms: number): void {
    if (ms <= 0) {
      return;
    }
    // Waiting on the value we last saw means a wake that arrived since returns at once, so a
    // wake can never be lost between the loop deciding to sleep and sleeping.
    Atomics.wait(this.#word, 0, this.#seen, ms);
    this.#seen = Atomics.load(this.#word, 0);
  }
}

interface SchedulerLike {
  postTask(callback: () => void, options: { delay: number }): Promise<void>;
}

export class TimerSleeper implements Sleeper {
  readonly kind = 'timer';
  readonly #scheduler: SchedulerLike | undefined;

  constructor(scope: { scheduler?: SchedulerLike } = globalThis) {
    this.#scheduler = typeof scope.scheduler?.postTask === 'function' ? scope.scheduler : undefined;
  }

  sleep(ms: number): Promise<void> {
    if (ms <= 0) {
      return Promise.resolve();
    }
    return new Promise<void>((resolve) => {
      if (this.#scheduler !== undefined) {
        void this.#scheduler.postTask(resolve, { delay: ms });
      } else {
        setTimeout(resolve, ms);
      }
    });
  }
}

/**
 * The best sleeper available: atomics if there is a wake word and `Atomics.wait` is allowed here
 * (it is not on the browser's main thread, and not when the page is not isolated), else timers.
 */
export function createSleeper(opts: {
  wake: SharedArrayBuffer | null;
  forceTimer: boolean;
}): Sleeper {
  if (!opts.forceTimer && opts.wake !== null) {
    try {
      // A zero-timeout wait on a value that differs throws on threads that may not block and
      // returns "not-equal" on those that may.
      Atomics.wait(new Int32Array(opts.wake), 0, 1 << 30, 0);
      return new AtomicsSleeper(opts.wake);
    } catch {
      // fall through to timers
    }
  }
  return new TimerSleeper();
}
