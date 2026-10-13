/** Result of the capability probe (GDD 53.3 compat check, simplified for v0.0a). */
export interface Features {
  crossOriginIsolated: boolean;
  sharedArrayBuffer: boolean;
  wasm: boolean;
  wasmSimd: boolean;
  wasmThreads: boolean;
  serviceWorker: boolean;
  webgpu: boolean;
  offscreenCanvas: boolean;
}

/** What `requestAdapter` resolves to, as far as this module looks. */
export interface GpuAdapterLike {
  info?: { vendor?: string; architecture?: string; device?: string; description?: string };
}

/** The slice of the global scope `detect` looks at; injectable for tests. */
export interface FeatureEnvironment {
  crossOriginIsolated?: boolean;
  SharedArrayBuffer?: unknown;
  WebAssembly?: { validate(bytes: Uint8Array): boolean };
  navigator?: {
    serviceWorker?: unknown;
    gpu?: { requestAdapter(options?: object): Promise<GpuAdapterLike | null> };
    storage?: { getDirectory?: unknown };
    getGamepads?: unknown;
  };
  OffscreenCanvas?:
    (new (width: number, height: number) => { getContext(id: string): unknown }) | undefined;
  document?: { createElement(tag: string): { getContext(id: string): unknown } };
}

/** `(module (func (result v128) i32.const 0 i8x16.splat))`, valid only with SIMD support. */
const SIMD_PROBE = new Uint8Array([
  0, 97, 115, 109, 1, 0, 0, 0, 1, 5, 1, 96, 0, 1, 123, 3, 2, 1, 0, 10, 10, 1, 8, 0, 65, 0, 253, 15,
  253, 98, 11,
]);

/** A module with a shared memory and an atomic op, valid only with the threads proposal. */
const THREADS_PROBE = new Uint8Array([
  0, 97, 115, 109, 1, 0, 0, 0, 1, 4, 1, 96, 0, 0, 3, 2, 1, 0, 5, 4, 1, 3, 1, 1, 10, 11, 1, 9, 0, 65,
  0, 254, 16, 2, 0, 26, 11,
]);

function safeValidate(env: FeatureEnvironment, bytes: Uint8Array): boolean {
  try {
    return env.WebAssembly?.validate(bytes) ?? false;
  } catch {
    return false;
  }
}

/** Probes the environment. Never throws. */
export function detect(env: FeatureEnvironment = globalThis as FeatureEnvironment): Features {
  const wasm = typeof env.WebAssembly?.validate === 'function';
  const sharedArrayBuffer = typeof env.SharedArrayBuffer === 'function';
  return {
    crossOriginIsolated: env.crossOriginIsolated === true,
    sharedArrayBuffer,
    wasm,
    wasmSimd: wasm && safeValidate(env, SIMD_PROBE),
    wasmThreads: wasm && safeValidate(env, THREADS_PROBE),
    serviceWorker: env.navigator?.serviceWorker !== undefined,
    webgpu: env.navigator?.gpu !== undefined,
    offscreenCanvas: env.OffscreenCanvas !== undefined,
  };
}

/** Facts that need an await or a throwaway context to discover. */
export interface DeepFeatures extends Features {
  /** `requestAdapter` resolved to an adapter (WebGPU is present *and* a GPU is usable). */
  webgpuAdapter: boolean;
  /** Vendor and device as the adapter reports them, for the overlay and benchmark reports. */
  adapterInfo: string;
  /** A WebGL2 context can be created. */
  webgl2: boolean;
  /** The Origin Private File System is available. */
  opfs: boolean;
  /** The Gamepad API is available. */
  gamepad: boolean;
}

/** Longest the adapter request may take before we call WebGPU unusable. */
const ADAPTER_TIMEOUT_MS = 4000;

async function probeAdapter(env: FeatureEnvironment): Promise<{ ok: boolean; info: string }> {
  const gpu = env.navigator?.gpu;
  if (gpu === undefined) {
    return { ok: false, info: '' };
  }
  try {
    const timeout = new Promise<null>((resolve) => {
      setTimeout(() => {
        resolve(null);
      }, ADAPTER_TIMEOUT_MS);
    });
    const adapter = await Promise.race([
      gpu.requestAdapter({ powerPreference: 'high-performance' }),
      timeout,
    ]);
    if (adapter === null) {
      return { ok: false, info: '' };
    }
    const i = adapter.info;
    const info = [i?.vendor, i?.architecture, i?.device, i?.description]
      .filter((s): s is string => typeof s === 'string' && s !== '')
      .join(' ');
    return { ok: true, info };
  } catch {
    return { ok: false, info: '' };
  }
}

/** A WebGL2 context on a canvas nobody else will use. */
function probeWebgl2(env: FeatureEnvironment): boolean {
  try {
    const canvas =
      env.OffscreenCanvas !== undefined
        ? new env.OffscreenCanvas(1, 1)
        : env.document?.createElement('canvas');
    return canvas?.getContext('webgl2') != null;
  } catch {
    return false;
  }
}

/** `detect` plus the probes that cost an await. Never throws. */
export async function detectAsync(
  env: FeatureEnvironment = globalThis as FeatureEnvironment,
): Promise<DeepFeatures> {
  const base = detect(env);
  const adapter = await probeAdapter(env);
  return {
    ...base,
    webgpuAdapter: adapter.ok,
    adapterInfo: adapter.info,
    webgl2: probeWebgl2(env),
    opfs: typeof env.navigator?.storage?.getDirectory === 'function',
    gamepad: typeof env.navigator?.getGamepads === 'function',
  };
}
