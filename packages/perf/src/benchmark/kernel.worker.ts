/**
 * The CPU kernel of the first-run benchmark: a private simulation core, with the core content
 * loaded so the player's aircraft flies the 6-DOF model hands-off, stepped flat out for a set
 * time. Several of these at once, released together, show how well the machine scales across
 * cores (GDD 60.8). Each worker owns its own module instance, so there is no shared state to
 * contend on; what scales (or does not) is the machine.
 */
import { loadSim, SCRIPTED_SESSION } from '@flying/sim-bridge/sim-handle';
import type { KernelInit, KernelReply } from './kernel.ts';

interface WorkerScope {
  onmessage: ((event: MessageEvent<KernelInit | { type: 'go' }>) => void) | null;
  postMessage(message: KernelReply): void;
}
const scope = self as unknown as WorkerScope;

/** Ticks per call into the module: long enough to be mostly kernel, short enough to check the time. */
const CHUNK_TICKS = 200;

let release: (() => void) | undefined;

scope.onmessage = (event) => {
  const msg = event.data;
  if (msg.type === 'init') {
    void prepare(msg).catch((e: unknown) => {
      scope.postMessage({ type: 'error', message: e instanceof Error ? e.message : String(e) });
    });
  } else {
    release?.();
  }
};

async function prepare(cmd: KernelInit): Promise<void> {
  const sim = await loadSim({
    variant: cmd.variant,
    url: cmd.wasmUrl,
    threadsInitialPages: cmd.threadsInitialPages,
  });
  const response = await fetch(cmd.contentUrl);
  if (!response.ok) {
    throw new Error(`SYS-005: ${cmd.contentUrl} returned ${String(response.status)}`);
  }
  sim.init(0x1f1e5eed00000001n);
  sim.loadContent(new Uint8Array(await response.arrayBuffer()));
  sim.configure(SCRIPTED_SESSION);
  if (!sim.start()) throw new Error('SYS-004: the benchmark core would not start');
  scope.postMessage({ type: 'ready' });

  // Wait for the common start: a flag in shared memory where there is one (precise), else a message.
  if (cmd.startFlag !== null) {
    const flag = new Int32Array(cmd.startFlag);
    while (Atomics.load(flag, 0) === 0) {
      Atomics.wait(flag, 0, 0, 100);
    }
  } else {
    await new Promise<void>((resolve) => {
      release = resolve;
    });
  }

  const t0 = performance.now();
  let ticks = 0;
  let now = t0;
  while (now - t0 < cmd.ms) {
    ticks += sim.step(CHUNK_TICKS);
    now = performance.now();
  }
  scope.postMessage({ type: 'result', ticks, elapsedMs: now - t0 });
}
