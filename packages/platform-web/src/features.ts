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

/** The slice of the global scope `detect` looks at; injectable for tests. */
export interface FeatureEnvironment {
  crossOriginIsolated?: boolean;
  SharedArrayBuffer?: unknown;
  WebAssembly?: { validate(bytes: Uint8Array): boolean };
  navigator?: { serviceWorker?: unknown; gpu?: unknown };
  OffscreenCanvas?: unknown;
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
