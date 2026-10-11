import type { Features } from './features.ts';

/** `single` owns its memory; `threads` imports a shared memory (needs cross-origin isolation). */
export type Variant = 'single' | 'threads';

/** Must equal `ABI_VERSION` in `crates/sim-wasm/src/abi.rs`. */
export const EXPECTED_ABI_VERSION = 1;

/** 2 GiB in 64 KiB pages (GDD 0.6.3). */
export const MAX_PAGES = 32768;

/** Which subsystem hash to read. Values are the `kind` argument of `fg_subsystem_hash_*`. */
export const HASH_KINDS = { combined: 0, tick: 1, rng: 2, bodies: 3 } as const;
export type HashKind = keyof typeof HASH_KINDS;

/** Picks the best variant the environment can run, or `undefined` if neither can run. */
export function chooseVariant(f: Features): Variant | undefined {
  if (!f.wasm || !f.wasmSimd) {
    return undefined;
  }
  return f.crossOriginIsolated && f.sharedArrayBuffer && f.wasmThreads ? 'threads' : 'single';
}

/** Import object for a variant. `threads` gets a fresh shared memory. */
export function buildImports(variant: Variant, threadsInitialPages: number): WebAssembly.Imports {
  if (variant === 'single') {
    return {};
  }
  const memory = new WebAssembly.Memory({
    initial: threadsInitialPages,
    maximum: MAX_PAGES,
    shared: true,
  });
  return { env: { memory } };
}

/** Typed view of the simulation exports. 64-bit values travel as two u32 halves. */
export interface SimHandle {
  readonly variant: Variant;
  abiVersion(): number;
  init(seed: bigint): void;
  step(maxTicks: number): number;
  tick(): bigint;
  hash(kind?: HashKind): bigint;
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

/** 16-digit lower-case hex, the format `fg hash` prints. */
export function hex64(v: bigint): string {
  return v.toString(16).padStart(16, '0');
}

/** Wraps an instantiated module. Throws if the ABI version or exports do not match. */
export function wrapSim(variant: Variant, instance: WebAssembly.Instance): SimHandle {
  const x = instance.exports;
  const abi = requireFn(x, 'fg_abi_version');
  const version = abi();
  if (version !== EXPECTED_ABI_VERSION) {
    throw new Error(
      `SYS-003: sim ABI ${String(version)}, expected ${String(EXPECTED_ABI_VERSION)}`,
    );
  }
  const init = requireFn(x, 'fg_init');
  const step = requireFn(x, 'fg_step');
  const tickLo = requireFn(x, 'fg_tick_lo');
  const tickHi = requireFn(x, 'fg_tick_hi');
  const subLo = requireFn(x, 'fg_subsystem_hash_lo');
  const subHi = requireFn(x, 'fg_subsystem_hash_hi');
  return {
    variant,
    abiVersion: () => abi(),
    init(seed) {
      const status = init(Number(seed & 0xffff_ffffn), Number(seed >> 32n));
      if (status !== 0) {
        throw new Error(`SYS-004: fg_init returned ${String(status)}`);
      }
    },
    step: (n) => step(n),
    tick: () => join(tickLo(), tickHi()),
    hash: (kind = 'combined') => join(subLo(HASH_KINDS[kind]), subHi(HASH_KINDS[kind])),
  };
}

/** Instantiates from bytes (Node runner, tests, streaming fallback). */
export async function instantiateBytes(
  bytes: BufferSource,
  variant: Variant,
  threadsInitialPages: number,
): Promise<SimHandle> {
  const { instance } = await WebAssembly.instantiate(
    bytes,
    buildImports(variant, threadsInitialPages),
  );
  return wrapSim(variant, instance);
}

/** Fetches and instantiates, streaming when the server sends `application/wasm`. */
export async function loadSim(opts: {
  variant: Variant;
  url: string;
  threadsInitialPages: number;
}): Promise<SimHandle> {
  const imports = buildImports(opts.variant, opts.threadsInitialPages);
  const response = await fetch(opts.url);
  if (!response.ok) {
    throw new Error(`SYS-005: ${opts.url} returned ${String(response.status)}`);
  }
  let instance: WebAssembly.Instance;
  try {
    ({ instance } = await WebAssembly.instantiateStreaming(response.clone(), imports));
  } catch (e) {
    if (!(e instanceof TypeError)) {
      throw e;
    }
    // Wrong MIME type on the host: fall back to buffering.
    ({ instance } = await WebAssembly.instantiate(await response.arrayBuffer(), imports));
  }
  return wrapSim(opts.variant, instance);
}
