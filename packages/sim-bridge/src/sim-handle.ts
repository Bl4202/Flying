/**
 * A typed handle on one instantiated simulation module (ABI in `crates/sim-wasm/src/abi.rs`).
 *
 * 64-bit values travel as two u32 halves so JavaScript never needs `BigInt` on a hot path.
 * Blocks the module owns (snapshot, profile, terrain) are handed out as views onto its memory.
 * Growing the memory replaces the `ArrayBuffer` behind every view, so each accessor rebuilds its
 * view when the buffer's identity changed; a view is good until the next call into the module that
 * can allocate (`init`, `loadContent`, `fillTerrain`). Copy what must outlive that.
 */
import {
  hex64,
  instantiateBytes,
  loadWasm,
  type HostImports,
  type LoadedWasm,
  type Variant,
} from '@flying/platform-web/wasm-loader';
import { ABI, CONTENT_STATUS, HASH_KIND, INPUT_FRAME, TERRAIN } from './layout.gen.ts';

export { HASH_KIND, hex64 };

/** Must equal `ABI_VERSION` in `crates/sim-wasm/src/abi.rs`. */
export const EXPECTED_ABI_VERSION = ABI.VERSION;

export interface SimHandle {
  readonly variant: Variant;
  readonly memory: WebAssembly.Memory;
  abiVersion(): number;
  /** Creates the core, idle. Load content and choose a script, then `start`. */
  init(seed: bigint): void;
  /** Loads a `.cpack` into an idle core. Throws if the core refuses it. */
  loadContent(bytes: Uint8Array): void;
  /** Selects the scripted pilot (`INPUT_SCRIPT`). Returns false for an unknown script. */
  setInputScript(script: number): boolean;
  /** `Idle -> Running`. Returns false if the core was not idle. */
  start(): boolean;
  /** `Running -> Paused`. */
  pause(): boolean;
  /** `Paused -> Running`. */
  resume(): boolean;
  /** `CORE_STATE` of the core. */
  coreState(): number;
  /**
   * Hands `count` frames (96 bytes each, back to back in `frames`) to the core for its next tick,
   * at most `ABI.INPUT_BATCH` per call. Returns how many it accepted.
   */
  pushInputs(frames: Uint8Array, count: number): number;
  setProfiling(on: boolean): void;
  /** Runs up to `maxTicks` ticks; returns how many ran. */
  step(maxTicks: number): number;
  tick(): bigint;
  /** The core's newest snapshot, in place. */
  snapshot(): Uint8Array;
  /** The profile block, refreshed. */
  profile(): Uint8Array;
  /** Generates the terrain patch around a BF point and returns it in place, or `undefined`. */
  fillTerrain(eastM: number, northM: number, spacingM: number): Uint8Array | undefined;
  /** A state hash (`HASH_KIND`), 64 bits. */
  hash(kind?: number): bigint;
}

type Fn = (...args: number[]) => number;

function requireFn(exports: WebAssembly.Exports, name: string): Fn {
  const f = exports[name];
  if (typeof f !== 'function') {
    throw new Error(`SYS-002: sim module is missing export ${name}`);
  }
  return f as Fn;
}

function join(lo: number, hi: number): bigint {
  return (BigInt(hi >>> 0) << 32n) | BigInt(lo >>> 0);
}

/** A view of `len` bytes at `ptr` that is rebuilt only when the memory buffer or the range moves. */
class Window {
  readonly #memory: WebAssembly.Memory;
  #buffer: ArrayBufferLike | undefined;
  #ptr = -1;
  #len = -1;
  #view = new Uint8Array(0);

  constructor(memory: WebAssembly.Memory) {
    this.#memory = memory;
  }

  at(ptr: number, len: number): Uint8Array {
    const buffer = this.#memory.buffer;
    if (buffer !== this.#buffer || ptr !== this.#ptr || len !== this.#len) {
      this.#view = new Uint8Array(buffer, ptr, len);
      this.#buffer = buffer;
      this.#ptr = ptr;
      this.#len = len;
    }
    return this.#view;
  }
}

/** Wraps an instantiated module. Throws if the ABI version or any export does not match. */
export function wrapSim({ variant, instance, memory }: LoadedWasm): SimHandle {
  const x = instance.exports;
  const abi = requireFn(x, 'fg_abi_version');
  const version = abi();
  if (version !== EXPECTED_ABI_VERSION) {
    throw new Error(
      `SYS-003: sim ABI ${String(version)}, expected ${String(EXPECTED_ABI_VERSION)}`,
    );
  }
  const init = requireFn(x, 'fg_init');
  const coreState = requireFn(x, 'fg_core_state');
  const reserve = requireFn(x, 'fg_content_reserve');
  const load = requireFn(x, 'fg_load_content');
  const start = requireFn(x, 'fg_start');
  const pause = requireFn(x, 'fg_pause');
  const resume = requireFn(x, 'fg_resume');
  const setScript = requireFn(x, 'fg_set_input_script');
  const inputPtr = requireFn(x, 'fg_input_ptr');
  const inputCapacity = requireFn(x, 'fg_input_capacity');
  const pushInputs = requireFn(x, 'fg_push_inputs');
  const setProfiling = requireFn(x, 'fg_set_profiling');
  const step = requireFn(x, 'fg_step');
  const snapshotPtr = requireFn(x, 'fg_snapshot_ptr');
  const snapshotLen = requireFn(x, 'fg_snapshot_len');
  const profilePtr = requireFn(x, 'fg_profile_ptr');
  const profileLen = requireFn(x, 'fg_profile_len');
  const terrainPtr = requireFn(x, 'fg_terrain_ptr');
  const terrainFill = requireFn(x, 'fg_terrain_fill');
  const tickLo = requireFn(x, 'fg_tick_lo');
  const tickHi = requireFn(x, 'fg_tick_hi');
  const subLo = requireFn(x, 'fg_subsystem_hash_lo');
  const subHi = requireFn(x, 'fg_subsystem_hash_hi');

  if (inputCapacity() !== ABI.INPUT_BATCH) {
    throw new Error(
      `SYS-003: sim input batch is ${String(inputCapacity())}, expected ${String(ABI.INPUT_BATCH)}`,
    );
  }

  const snapshotWindow = new Window(memory);
  const profileWindow = new Window(memory);
  const terrainWindow = new Window(memory);
  const inputWindow = new Window(memory);
  const contentWindow = new Window(memory);

  return {
    variant,
    memory,
    abiVersion: () => abi(),
    init(seed) {
      const status = init(Number(seed & 0xffff_ffffn), Number(seed >> 32n));
      if (status !== 0) {
        throw new Error(`SYS-004: fg_init returned ${String(status)}`);
      }
    },
    loadContent(bytes) {
      const ptr = reserve(bytes.length);
      if (ptr === 0) {
        throw new Error(`SYS-006: cannot reserve ${String(bytes.length)} bytes for content`);
      }
      // Reserving may have grown memory, so take the view only now.
      contentWindow.at(ptr, bytes.length).set(bytes);
      const status = load(bytes.length);
      if (status !== CONTENT_STATUS.OK) {
        throw new Error(`SYS-007: content rejected (status ${String(status)})`);
      }
    },
    setInputScript: (script) => setScript(script) === 1,
    start: () => start() === 1,
    pause: () => pause() === 1,
    resume: () => resume() === 1,
    coreState: () => coreState(),
    pushInputs(frames, count) {
      const n = Math.min(count, ABI.INPUT_BATCH);
      if (frames.byteLength < n * INPUT_FRAME.SIZE) {
        throw new RangeError('frames holds fewer bytes than count frames');
      }
      inputWindow
        .at(inputPtr(), ABI.INPUT_BATCH * INPUT_FRAME.SIZE)
        .set(frames.subarray(0, n * INPUT_FRAME.SIZE));
      return pushInputs(n);
    },
    setProfiling: (on) => {
      setProfiling(on ? 1 : 0);
    },
    step: (n) => step(n),
    tick: () => join(tickLo(), tickHi()),
    snapshot: () => snapshotWindow.at(snapshotPtr(), snapshotLen()),
    profile() {
      const ptr = profilePtr(); // refreshes the block
      return profileWindow.at(ptr, profileLen());
    },
    fillTerrain(eastM, northM, spacingM) {
      const bytes = terrainFill(eastM, northM, spacingM);
      if (bytes === 0) {
        return undefined;
      }
      if (bytes !== TERRAIN.SIZE) {
        throw new Error(
          `SYS-008: terrain patch is ${String(bytes)} bytes, expected ${String(TERRAIN.SIZE)}`,
        );
      }
      return terrainWindow.at(terrainPtr(), bytes);
    },
    hash: (kind = HASH_KIND.COMBINED) => join(subLo(kind), subHi(kind)),
  };
}

/** Instantiates from bytes (Node runner, tests). */
export async function instantiateSim(
  bytes: BufferSource,
  variant: Variant,
  threadsInitialPages: number,
  host?: HostImports,
): Promise<SimHandle> {
  return wrapSim(await instantiateBytes(bytes, variant, threadsInitialPages, host));
}

/** Fetches and instantiates. */
export async function loadSim(opts: {
  variant: Variant;
  url: string;
  threadsInitialPages: number;
  host?: HostImports;
}): Promise<SimHandle> {
  return wrapSim(await loadWasm(opts));
}
