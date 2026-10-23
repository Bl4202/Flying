import { describe, expect, it, vi } from 'vitest';
import {
  makeInputChannel,
  makeSnapshotChannel,
  type SimEvent,
  type SimStats,
} from '@flying/sim-bridge';
import { SimClient, type SimClientOptions, type WorkerLike } from './sim-client.ts';
import { createWakeWord } from './sleeper.ts';

class FakeWorker implements WorkerLike {
  sent: { message: unknown; transfer: Transferable[] | undefined }[] = [];
  terminated = false;
  onmessage: ((event: { data: unknown }) => void) | null = null;
  onerror: ((event: { message?: string }) => void) | null = null;
  postMessage(message: unknown, transfer?: Transferable[]): void {
    this.sent.push({ message, transfer });
  }
  terminate(): void {
    this.terminated = true;
  }
  say(event: unknown): void {
    this.onmessage?.({ data: event });
  }
  commands(): string[] {
    return this.sent.map((s) => (s.message as { type: string }).type);
  }
}

const READY: SimEvent = {
  type: 'ready',
  abi: 2,
  variant: 'single',
  sleeper: 'timer',
  contentHash: '00ff',
};

function options(extra: Partial<SimClientOptions> = {}, wake: SharedArrayBuffer | null = null) {
  const worker = new FakeWorker();
  const snapshot = makeSnapshotChannel('message');
  const input = makeInputChannel('message');
  const opts: SimClientOptions = {
    workerUrl: '/sim.worker.js',
    init: {
      variant: 'single',
      wasmUrl: '/sim.wasm',
      threadsInitialPages: 256,
      contentUrl: '/core.cpack',
      seed: '1f1e5eed00000001',
      script: 1,
      session: { spawn: 1, assist: 2, gEffects: 2 },
      snapshot: snapshot.sink,
      input: input.sink,
      terrainPort: null,
      wake,
      forceTimer: false,
      profiling: false,
      allowStall: false,
    },
    createWorker: () => worker,
    ...extra,
  };
  const close = (): void => {
    for (const e of [snapshot.sink, snapshot.source, input.sink, input.source]) {
      if (e.kind === 'message') e.port.close();
    }
  };
  return { worker, opts, close };
}

async function started(
  extra: Partial<SimClientOptions> = {},
  wake: SharedArrayBuffer | null = null,
) {
  const t = options(extra, wake);
  const pending = SimClient.create(t.opts);
  t.worker.say(READY);
  return { ...t, client: await pending };
}

describe('SimClient.create', () => {
  it('sends init with the ports to transfer, then resolves on ready', async () => {
    const t = options();
    const pending = SimClient.create(t.opts);
    expect(t.worker.sent).toHaveLength(1);
    const first = t.worker.sent[0];
    expect((first?.message as { type: string }).type).toBe('init');
    expect(first?.transfer).toHaveLength(2); // one port per message endpoint
    t.worker.say(READY);
    const client = await pending;
    expect(client.ready.sleeper).toBe('timer');
    expect(client.variant).toBe('single');
    t.close();
  });

  it('rejects, and ends the worker, if the worker reports an error before ready', async () => {
    const t = options();
    const pending = SimClient.create(t.opts);
    t.worker.say({ type: 'error', message: 'SYS-005: no wasm' });
    await expect(pending).rejects.toThrow(/SYS-005/);
    expect(t.worker.terminated).toBe(true);
    t.close();
  });

  it('rejects if the worker script itself fails to load', async () => {
    const t = options();
    const pending = SimClient.create(t.opts);
    t.worker.onerror?.({ message: 'blocked by CSP' });
    await expect(pending).rejects.toThrow(/SYS-033.*blocked by CSP/);
    expect(t.worker.terminated).toBe(true);
    t.close();
  });

  it('rejects on a malformed first message', async () => {
    const t = options();
    const pending = SimClient.create(t.opts);
    t.worker.say({ type: 'nonsense' });
    await expect(pending).rejects.toThrow(/SYS-030/);
    t.close();
  });

  it('gives up if the worker never answers', async () => {
    vi.useFakeTimers();
    try {
      const t = options({ readyTimeoutMs: 500 });
      const pending = SimClient.create(t.opts);
      const check = expect(pending).rejects.toThrow(/SYS-032/);
      await vi.advanceTimersByTimeAsync(501);
      await check;
      expect(t.worker.terminated).toBe(true);
      t.close();
    } finally {
      vi.useRealTimers();
    }
  });
});

describe('SimClient', () => {
  it('sends the lifecycle and tuning commands', async () => {
    const { worker, client, close } = await started();
    worker.sent.length = 0;
    client.start();
    client.pause();
    client.resume();
    client.setProfiling(true);
    client.resetRun();
    client.stall(300);
    expect(worker.sent.map((s) => s.message)).toEqual([
      { type: 'start' },
      { type: 'pause' },
      { type: 'resume' },
      { type: 'profiling', on: true },
      { type: 'reset-run' },
      { type: 'stall', ms: 300 },
    ]);
    close();
  });

  it('wakes a sleeping worker after every command when it has a wake word', async () => {
    const wake = createWakeWord();
    const word = new Int32Array(wake);
    const { client, close } = await started({}, wake);
    const before = Atomics.load(word, 0);
    client.pause();
    client.resume();
    expect(Atomics.load(word, 0)).toBe(before + 2);
    close();
  });

  it('does not touch a wake word it does not have', async () => {
    const { client, close } = await started();
    expect(() => {
      client.start();
    }).not.toThrow();
    close();
  });

  it('passes statistics and state changes on, and records when stats last arrived', async () => {
    const stats: SimStats[] = [];
    const states: number[] = [];
    const { worker, client, close } = await started({
      onStats: (s) => stats.push(s),
      onState: (s) => states.push(s),
    });
    expect(client.lastStatsAt).toBe(0);
    const s = { tick: 42 } as unknown as SimStats;
    worker.say({ type: 'stats', stats: s });
    worker.say({ type: 'state', coreState: 3 });
    expect(stats).toEqual([s]);
    expect(states).toEqual([3]);
    expect(client.lastStatsAt).toBeGreaterThan(0);
    close();
  });

  it('matches hash replies to requests, in any order', async () => {
    const { worker, client, close } = await started();
    worker.sent.length = 0;
    const a = client.requestHash();
    const b = client.requestHash();
    const ids = worker.sent.map((s) => (s.message as { id: number }).id);
    expect(new Set(ids).size).toBe(2);
    worker.say({ type: 'hash', id: ids[1], tick: 20, hashes: { combined: 'bb' } });
    worker.say({ type: 'hash', id: ids[0], tick: 10, hashes: { combined: 'aa' } });
    expect(await a).toEqual({ tick: 10, hashes: { combined: 'aa' } });
    expect(await b).toEqual({ tick: 20, hashes: { combined: 'bb' } });
    close();
  });

  it('reports worker errors and bad messages instead of throwing', async () => {
    const errors: string[] = [];
    const { worker, close } = await started({ onError: (m) => errors.push(m) });
    worker.say({ type: 'error', message: 'SYS-009: boom' });
    worker.say({ nope: true });
    worker.onerror?.({ message: 'crashed' });
    expect(errors).toHaveLength(3);
    expect(errors[0]).toMatch(/boom/);
    expect(errors[1]).toMatch(/SYS-030/);
    expect(errors[2]).toMatch(/crashed/);
    close();
  });

  it('stops by telling the worker and then ending it', async () => {
    const { worker, client, close } = await started();
    client.stop();
    expect(worker.commands().at(-1)).toBe('stop');
    expect(worker.terminated).toBe(true);
    close();
  });
});
