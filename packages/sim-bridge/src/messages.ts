/**
 * Control messages between the main thread and the Sim Worker, and the Sim Worker's reports.
 *
 * Bulk data (snapshots, input frames, the terrain patch) never travels in these: it goes through
 * the channels in this package. These are the commands and the 4 Hz statistics.
 *
 * `postMessage` accepts anything, so each receiver validates what it gets (`parseSimCommand`,
 * `parseSimEvent`): a message from a mismatched build or a bug becomes a named error instead of
 * an `undefined` three calls later.
 */
import type { Variant } from '@flying/platform-web/wasm-loader';
import type { InputEndpoint } from './input-channel.ts';
import type { SnapshotEndpoint } from './snapshot-channel.ts';

export interface SimInit {
  type: 'init';
  variant: Variant;
  wasmUrl: string;
  threadsInitialPages: number;
  contentUrl: string;
  /** Hex master seed. */
  seed: string;
  /** `INPUT_SCRIPT` value. */
  script: number;
  snapshot: SnapshotEndpoint;
  input: InputEndpoint;
  /** Where to send the terrain patch once, or `null` if nobody wants it. */
  terrainPort: MessagePort | null;
  /** One shared word the main thread bumps to cut the worker's sleep short; `null` = timers. */
  wake: SharedArrayBuffer | null;
  /** Use the timer sleeper even if `Atomics.wait` is available. */
  forceTimer: boolean;
  profiling: boolean;
  /** Test hook: honour `stall` commands. */
  allowStall: boolean;
}

export type SimCommand =
  | SimInit
  | { type: 'start' }
  | { type: 'pause' }
  | { type: 'resume' }
  | { type: 'stop' }
  | { type: 'hash'; id: number }
  | { type: 'profiling'; on: boolean }
  /** Forget run-level statistics (after warm-up). */
  | { type: 'reset-run' }
  /** Block the worker for `ms` (only if the init allowed it). */
  | { type: 'stall'; ms: number };

/** Summary of a set of samples, milliseconds. */
export interface Quantiles {
  count: number;
  avg: number;
  p50: number;
  p95: number;
  p99: number;
  max: number;
}

export interface SimStats {
  /** When this report was made, on the shared clock. */
  atMs: number;
  tick: number;
  /** Ticks per second over the last report interval. */
  hz: number;
  /** Simulated time given up because the worker fell more than a few ticks behind. */
  droppedMs: number;
  droppedEvents: number;
  /** Time spent on one tick (drain, step, publish): the last interval, and since `reset-run`. */
  step: Quantiles;
  stepRun: Quantiles;
  /** From the controls being sampled to the snapshot that reflects them being published. */
  inputLatency: Quantiles;
  inputLatencyRun: Quantiles;
  inputsDropped: number;
  snapshotsDropped: number;
  coreState: number;
  sleeper: 'atomics' | 'timer';
  /**
   * The ownship as of the newest snapshot: the aircraft, or the spectator (`flags` has
   * `OWNSHIP_FLAG.SPECTATOR`, air data zero). Battlefield frame (east, north, up), SI units,
   * angles in radians.
   */
  aircraft: {
    /** Reference point, BF metres. */
    pos: [number, number, number];
    /** Body-to-BF attitude, `[w, x, y, z]`. */
    quat: [number, number, number, number];
    /** True and calibrated airspeed, m/s. */
    tas: number;
    cas: number;
    mach: number;
    alpha: number;
    beta: number;
    /** Body-axis load factor, g. */
    nz: number;
    /** Throttle lever, 0 to 1. */
    throttle: number;
    /** Net thrust, N. */
    thrust: number;
    /** Fuel and mass, kg. */
    fuel: number;
    mass: number;
    /** Elevator, aileron, rudder, radians. */
    surfaces: [number, number, number];
    /** Height above MSL, m. */
    altitude: number;
    abStage: number;
    /** `OWNSHIP_FLAG` bits. */
    flags: number;
    /** Sequence number of the newest input frame the core had applied. */
    inputSeq: number;
  };
  /** The core's own stage timings, while profiling is on. */
  profile: {
    tickMsLast: number;
    tickMsEwma: number;
    stageMsLast: number[];
    stageMsEwma: number[];
    counters: number[];
  } | null;
}

export interface SimReady {
  type: 'ready';
  abi: number;
  variant: Variant;
  sleeper: 'atomics' | 'timer';
  /** 16-digit hex of the content hash column. */
  contentHash: string;
}

export type SimEvent =
  | SimReady
  | { type: 'stats'; stats: SimStats }
  | { type: 'state'; coreState: number }
  | { type: 'hash'; id: number; tick: number; hashes: Record<string, string> }
  | { type: 'error'; message: string };

type Shape = Record<string, 'string' | 'number' | 'boolean' | 'object' | 'any'>;

const COMMAND_SHAPES: Record<SimCommand['type'], Shape> = {
  init: {
    variant: 'string',
    wasmUrl: 'string',
    threadsInitialPages: 'number',
    contentUrl: 'string',
    seed: 'string',
    script: 'number',
    snapshot: 'object',
    input: 'object',
    terrainPort: 'any',
    wake: 'any',
    forceTimer: 'boolean',
    profiling: 'boolean',
    allowStall: 'boolean',
  },
  start: {},
  pause: {},
  resume: {},
  stop: {},
  hash: { id: 'number' },
  profiling: { on: 'boolean' },
  'reset-run': {},
  stall: { ms: 'number' },
};

const EVENT_SHAPES: Record<SimEvent['type'], Shape> = {
  ready: { abi: 'number', variant: 'string', sleeper: 'string', contentHash: 'string' },
  stats: { stats: 'object' },
  state: { coreState: 'number' },
  hash: { id: 'number', tick: 'number', hashes: 'object' },
  error: { message: 'string' },
};

function check(kind: string, shapes: Record<string, Shape>, data: unknown): void {
  if (typeof data !== 'object' || data === null) {
    throw new TypeError(`SYS-030: ${kind} is not an object`);
  }
  const type = (data as { type?: unknown }).type;
  const shape = typeof type === 'string' ? shapes[type] : undefined;
  if (shape === undefined) {
    throw new TypeError(`SYS-030: unknown ${kind} type ${String(type)}`);
  }
  for (const [key, want] of Object.entries(shape)) {
    const value = (data as Record<string, unknown>)[key];
    if (want !== 'any' && typeof value !== want) {
      throw new TypeError(
        `SYS-030: ${kind} "${String(type)}" needs ${key}: ${want}, got ${typeof value}`,
      );
    }
    if (want === 'object' && value === null) {
      throw new TypeError(`SYS-030: ${kind} "${String(type)}" needs ${key}, got null`);
    }
  }
}

/** Returns `data` as a `SimCommand`, or throws `SYS-030` naming what is wrong. */
export function parseSimCommand(data: unknown): SimCommand {
  check('command', COMMAND_SHAPES, data);
  return data as SimCommand;
}

/** Returns `data` as a `SimEvent`, or throws `SYS-030` naming what is wrong. */
export function parseSimEvent(data: unknown): SimEvent {
  check('event', EVENT_SHAPES, data);
  return data as SimEvent;
}

export const SIM_COMMAND_TYPES = Object.keys(COMMAND_SHAPES) as SimCommand['type'][];
export const SIM_EVENT_TYPES = Object.keys(EVENT_SHAPES) as SimEvent['type'][];
