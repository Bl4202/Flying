/**
 * The Sim Worker: owns the WebAssembly simulation and runs it at a fixed 60 Hz, independent of
 * the main thread and the renderer (ADR-17, ADR-26).
 *
 * Each loop iteration runs the ticks that are owed (see `SimLoop`), then reports statistics if
 * 250 ms have passed, then yields one macrotask so control messages are handled, then sleeps
 * until the next tick falls due. Nothing in the loop waits on another thread.
 */
import { monotonicMs, sharedNowMs } from '@flying/platform-web';
import {
  HASH_KIND,
  hex64,
  loadSim,
  openInputSource,
  openSnapshotSink,
  parseSimCommand,
  SnapshotReader,
  type SimEvent,
  type SimHandle,
  type SimInit,
} from '@flying/sim-bridge';
import { FixedStepClock } from './clock.ts';
import {
  IDLE_POLL_MS,
  MAX_CATCH_UP_TICKS,
  SIM_DT_MS,
  STATS_INTERVAL_MS,
  TERRAIN_CHECK_MS,
  TERRAIN_FILL_WARN_MS,
  TERRAIN_SPACING_M,
} from './constants.ts';
import { SimLoop } from './sim-loop.ts';
import { nextPatchCentre } from './terrain-follow.ts';
import { createSleeper, type Sleeper } from './sleeper.ts';

interface WorkerScope {
  onmessage: ((event: MessageEvent<unknown>) => void) | null;
  postMessage(message: SimEvent): void;
  close(): void;
}
const scope = self as unknown as WorkerScope;

let sim: SimHandle | undefined;
let loop: SimLoop | undefined;
let sleeper: Sleeper | undefined;
let allowStall = false;
let stopped = false;
/** Where the renderer's terrain patch comes from, and the BF east/north of its centre. */
let terrainPort: MessagePort | null = null;
let patchCentre: [number, number] = [0, 0];
const closers: (() => void)[] = [];

function post(event: SimEvent): void {
  scope.postMessage(event);
}

scope.onmessage = (event) => {
  let command;
  try {
    command = parseSimCommand(event.data);
  } catch (e) {
    post({ type: 'error', message: e instanceof Error ? e.message : String(e) });
    return;
  }
  void handle(command).catch((e: unknown) => {
    post({ type: 'error', message: e instanceof Error ? e.message : String(e) });
  });
};

async function handle(command: ReturnType<typeof parseSimCommand>): Promise<void> {
  if (command.type === 'init') {
    await init(command);
    return;
  }
  if (sim === undefined || loop === undefined) {
    throw new Error(`SYS-031: "${command.type}" before init`);
  }
  switch (command.type) {
    case 'start':
      sim.start();
      loop.start();
      post({ type: 'state', coreState: sim.coreState() });
      break;
    case 'pause':
      sim.pause();
      loop.pause();
      post({ type: 'state', coreState: sim.coreState() });
      break;
    case 'resume':
      sim.resume();
      loop.resume();
      post({ type: 'state', coreState: sim.coreState() });
      break;
    case 'stop':
      stopped = true;
      break;
    case 'hash':
      post({
        type: 'hash',
        id: command.id,
        tick: Number(sim.tick()),
        hashes: Object.fromEntries(
          Object.entries(HASH_KIND).map(([name, kind]) => [
            name.toLowerCase(),
            hex64(sim?.hash(kind) ?? 0n),
          ]),
        ),
      });
      break;
    case 'profiling':
      loop.setProfiling(command.on);
      break;
    case 'reset-run':
      loop.resetRun();
      break;
    case 'stall': {
      if (allowStall) {
        const until = monotonicMs() + command.ms;
        while (monotonicMs() < until) {
          // Deliberately busy: this is the stall the independence tests inject.
        }
      }
      break;
    }
  }
}

async function init(cmd: SimInit): Promise<void> {
  allowStall = cmd.allowStall;
  const handle = await loadSim({
    variant: cmd.variant,
    url: cmd.wasmUrl,
    threadsInitialPages: cmd.threadsInitialPages,
  });
  const response = await fetch(cmd.contentUrl);
  if (!response.ok) {
    throw new Error(`SYS-005: ${cmd.contentUrl} returned ${String(response.status)}`);
  }
  const pack = new Uint8Array(await response.arrayBuffer());

  // Same order as `fg hash`: content first (only an idle core takes it), then the session, then
  // the script.
  handle.init(BigInt(`0x${cmd.seed}`));
  handle.loadContent(pack);
  handle.configure(cmd.session);
  if (!handle.setInputScript(cmd.script)) {
    throw new Error(`SYS-004: unknown input script ${String(cmd.script)}`);
  }

  if (cmd.terrainPort !== null) {
    terrainPort = cmd.terrainPort;
    sendPatch(handle, [0, 0]);
    closers.push(() => {
      terrainPort?.close();
      terrainPort = null;
    });
  }

  const inputs = openInputSource(cmd.input);
  const snapshots = openSnapshotSink(cmd.snapshot);
  closers.push(() => {
    inputs.close();
    snapshots.close();
  });
  sleeper = createSleeper({ wake: cmd.wake, forceTimer: cmd.forceTimer });
  sim = handle;
  loop = new SimLoop({
    sim: handle,
    inputs,
    snapshots,
    clock: new FixedStepClock({ dtMs: SIM_DT_MS, maxTicksPerIteration: MAX_CATCH_UP_TICKS }),
    nowMs: monotonicMs,
    sharedNowMs,
    sleeper: sleeper.kind,
    profiling: cmd.profiling,
  });

  post({
    type: 'ready',
    abi: handle.abiVersion(),
    variant: handle.variant,
    sleeper: sleeper.kind,
    contentHash: hex64(handle.hash(HASH_KIND.CONTENT)),
  });
  void run();
}

/** Generates the terrain patch around `centre` and sends it to the renderer. */
function sendPatch(handle: SimHandle, centre: [number, number]): void {
  if (terrainPort === null) return;
  const t0 = monotonicMs();
  const patch = handle.fillTerrain(centre[0], centre[1], TERRAIN_SPACING_M);
  if (patch === undefined) {
    throw new Error('SYS-008: terrain patch could not be generated');
  }
  const spent = monotonicMs() - t0;
  if (spent > TERRAIN_FILL_WARN_MS) {
    console.warn(`terrain patch took ${spent.toFixed(1)} ms to generate`);
  }
  patchCentre = centre;
  const copy = patch.slice().buffer;
  terrainPort.postMessage({ type: 'terrain', patch: copy }, [copy]);
}

/** Moves the terrain patch under the aircraft when it has flown away from the middle. */
function followTerrain(handle: SimHandle): void {
  if (terrainPort === null) return;
  const pos = new Float64Array(3);
  new SnapshotReader(handle.snapshot()).ownshipPos(pos);
  const next = nextPatchCentre(patchCentre, [pos[0] ?? 0, pos[1] ?? 0]);
  if (next !== undefined) sendPatch(handle, next);
}

/** Lets the worker's message events run: a message-port round trip is one macrotask. */
const yielder = new MessageChannel();
function yieldToMessages(): Promise<void> {
  return new Promise<void>((resolve) => {
    yielder.port1.onmessage = () => {
      resolve();
    };
    yielder.port2.postMessage(0);
  });
}

async function run(): Promise<void> {
  if (loop === undefined || sleeper === undefined || sim === undefined) {
    return;
  }
  let nextReport = monotonicMs() + STATS_INTERVAL_MS;
  let nextTerrainCheck = monotonicMs() + TERRAIN_CHECK_MS;
  try {
    while (!stopped) {
      if (loop.clock.running) {
        loop.iterate();
      } else {
        loop.discardInputs();
      }
      const now = monotonicMs();
      if (now >= nextReport) {
        post({ type: 'stats', stats: loop.report() });
        nextReport = now + STATS_INTERVAL_MS;
      }
      if (now >= nextTerrainCheck) {
        followTerrain(sim);
        nextTerrainCheck = now + TERRAIN_CHECK_MS;
      }
      await yieldToMessages();
      const wait = loop.clock.running ? loop.clock.nextDue() - monotonicMs() : IDLE_POLL_MS;
      await sleeper.sleep(wait);
    }
  } catch (e) {
    post({ type: 'error', message: e instanceof Error ? e.message : String(e) });
  } finally {
    for (const close of closers.splice(0)) close();
    scope.close();
  }
}
