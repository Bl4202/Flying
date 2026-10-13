import { Worker } from 'node:worker_threads';
import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  AtomicsSleeper,
  createSleeper,
  createWakeWord,
  TimerSleeper,
  wakeSleeper,
} from './sleeper.ts';

afterEach(() => {
  vi.restoreAllMocks();
  vi.useRealTimers();
});

function spawn(
  wake: SharedArrayBuffer,
  ms: number,
  sleeps = 1,
): {
  ready: Promise<void>;
  slept: Promise<number>;
  worker: Worker;
} {
  const worker = new Worker(new URL('./sleeper.fixture.worker.ts', import.meta.url), {
    workerData: { wake, ms, sleepsBeforeReport: sleeps },
  });
  const ready = new Promise<void>((resolve, reject) => {
    worker.once('message', () => {
      resolve();
    });
    worker.once('error', reject);
  });
  const slept = new Promise<number>((resolve, reject) => {
    worker.on('message', (m: { slept?: number }) => {
      if (m.slept !== undefined) resolve(m.slept);
    });
    worker.once('error', reject);
  });
  return { ready, slept, worker };
}

describe('AtomicsSleeper', () => {
  it('sleeps about as long as asked', () => {
    const s = new AtomicsSleeper(createWakeWord());
    const t0 = performance.now();
    s.sleep(20);
    const spent = performance.now() - t0;
    expect(spent).toBeGreaterThanOrEqual(18);
    expect(spent).toBeLessThan(80);
  });

  it('returns at once for zero or negative durations', () => {
    const s = new AtomicsSleeper(createWakeWord());
    const t0 = performance.now();
    s.sleep(0);
    s.sleep(-5);
    expect(performance.now() - t0).toBeLessThan(5);
  });

  it('is woken early by another thread', async () => {
    const wake = createWakeWord();
    const { ready, slept, worker } = spawn(wake, 5000);
    await ready;
    await new Promise<void>((r) => setTimeout(r, 60));
    wakeSleeper(wake);
    const spent = await slept;
    await worker.terminate();
    expect(spent).toBeLessThan(2000);
    expect(spent).toBeGreaterThan(20);
  });

  it('does not lose a wake that arrives before the sleep starts', async () => {
    const wake = createWakeWord();
    const { ready, slept, worker } = spawn(wake, 5000);
    await ready;
    // The worker may or may not have reached its sleep yet; either way the bump must cut it short.
    wakeSleeper(wake);
    const spent = await slept;
    await worker.terminate();
    expect(spent).toBeLessThan(2000);
  });

  it('a wake cuts only one sleep short, not the ones after', async () => {
    const wake = createWakeWord();
    const { ready, slept, worker } = spawn(wake, 150, 2);
    await ready;
    wakeSleeper(wake);
    const spent = await slept;
    await worker.terminate();
    // One sleep was cut short; the other ran its full 150 ms.
    expect(spent).toBeGreaterThan(120);
    expect(spent).toBeLessThan(450);
  });
});

describe('TimerSleeper', () => {
  it('uses scheduler.postTask with a delay when it exists', async () => {
    const calls: number[] = [];
    const s = new TimerSleeper({
      scheduler: {
        postTask: (cb, opts) => {
          calls.push(opts.delay);
          cb();
          return Promise.resolve();
        },
      },
    });
    await s.sleep(7.5);
    expect(calls).toEqual([7.5]);
  });

  it('falls back to setTimeout', async () => {
    vi.useFakeTimers();
    const s = new TimerSleeper({});
    let done = false;
    const p = Promise.resolve(s.sleep(10)).then(() => {
      done = true;
    });
    await vi.advanceTimersByTimeAsync(9);
    expect(done).toBe(false);
    await vi.advanceTimersByTimeAsync(2);
    await p;
    expect(done).toBe(true);
  });

  it('resolves at once for zero or negative durations', async () => {
    vi.useFakeTimers();
    const s = new TimerSleeper({});
    await s.sleep(0);
    await s.sleep(-1);
  });
});

describe('createSleeper', () => {
  it('prefers atomics when it can block here', () => {
    expect(createSleeper({ wake: createWakeWord(), forceTimer: false }).kind).toBe('atomics');
  });
  it('uses timers when asked, when there is no wake word, or when blocking is not allowed', () => {
    expect(createSleeper({ wake: createWakeWord(), forceTimer: true }).kind).toBe('timer');
    expect(createSleeper({ wake: null, forceTimer: false }).kind).toBe('timer');
    vi.spyOn(Atomics, 'wait').mockImplementation(() => {
      throw new TypeError('Atomics.wait cannot be called in this context');
    });
    expect(createSleeper({ wake: createWakeWord(), forceTimer: false }).kind).toBe('timer');
  });
});
