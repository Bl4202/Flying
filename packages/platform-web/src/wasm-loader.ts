import type { Features } from './features.ts';

/** `single` owns its memory; `threads` imports a shared memory (needs cross-origin isolation). */
export type Variant = 'single' | 'threads';

/** 2 GiB in 64 KiB pages (GDD 0.6.3). */
export const MAX_PAGES = 32768;

/** Picks the best variant the environment can run, or `undefined` if neither can run. */
export function chooseVariant(f: Features): Variant | undefined {
  if (!f.wasm || !f.wasmSimd) {
    return undefined;
  }
  return f.crossOriginIsolated && f.sharedArrayBuffer && f.wasmThreads ? 'threads' : 'single';
}

/** What the host gives a module: the clock the profiler reads (`env.fg_now`, milliseconds). */
export interface HostImports {
  now(): number;
}

const DEFAULT_HOST: HostImports = { now: () => performance.now() };

/**
 * Import object for a variant: the host clock, and for `threads` a fresh shared memory. The clock
 * only ever measures; nothing the module computes from it feeds simulation state.
 */
export function buildImports(
  variant: Variant,
  threadsInitialPages: number,
  host: HostImports = DEFAULT_HOST,
): WebAssembly.Imports {
  const env: Record<string, WebAssembly.ImportValue> = { fg_now: () => host.now() };
  if (variant === 'threads') {
    env['memory'] = new WebAssembly.Memory({
      initial: threadsInitialPages,
      maximum: MAX_PAGES,
      shared: true,
    });
  }
  return { env };
}

/** An instantiated module and the memory it runs on. */
export interface LoadedWasm {
  variant: Variant;
  instance: WebAssembly.Instance;
  /** Imported for `threads`, exported for `single`. */
  memory: WebAssembly.Memory;
}

function memoryOf(
  imports: WebAssembly.Imports,
  instance: WebAssembly.Instance,
): WebAssembly.Memory {
  const imported = imports['env']?.['memory'];
  const memory = imported instanceof WebAssembly.Memory ? imported : instance.exports['memory'];
  if (!(memory instanceof WebAssembly.Memory)) {
    throw new Error('SYS-002: sim module has no memory');
  }
  return memory;
}

/** Instantiates from bytes (Node runner, tests, streaming fallback). */
export async function instantiateBytes(
  bytes: BufferSource,
  variant: Variant,
  threadsInitialPages: number,
  host?: HostImports,
): Promise<LoadedWasm> {
  const imports = buildImports(variant, threadsInitialPages, host);
  const { instance } = await WebAssembly.instantiate(bytes, imports);
  return { variant, instance, memory: memoryOf(imports, instance) };
}

/** Fetches and instantiates, streaming when the server sends `application/wasm`. */
export async function loadWasm(opts: {
  variant: Variant;
  url: string;
  threadsInitialPages: number;
  host?: HostImports;
}): Promise<LoadedWasm> {
  const imports = buildImports(opts.variant, opts.threadsInitialPages, opts.host);
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
  return { variant: opts.variant, instance, memory: memoryOf(imports, instance) };
}

/** 16-digit lower-case hex, the format `fg hash` prints. */
export function hex64(v: bigint): string {
  return v.toString(16).padStart(16, '0');
}
