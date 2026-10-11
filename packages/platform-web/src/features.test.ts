import { describe, expect, it } from 'vitest';
import { detect, type FeatureEnvironment } from './features.ts';

describe('detect', () => {
  it('reports everything missing for an empty environment', () => {
    expect(detect({})).toEqual({
      crossOriginIsolated: false,
      sharedArrayBuffer: false,
      wasm: false,
      wasmSimd: false,
      wasmThreads: false,
      serviceWorker: false,
      webgpu: false,
      offscreenCanvas: false,
    });
  });

  it('reads flags from the environment', () => {
    const env: FeatureEnvironment = {
      crossOriginIsolated: true,
      SharedArrayBuffer: SharedArrayBuffer,
      WebAssembly: { validate: () => true },
      navigator: { serviceWorker: {}, gpu: {} },
      OffscreenCanvas: function OffscreenCanvas() {
        /* stand-in */
      },
    };
    expect(detect(env)).toEqual({
      crossOriginIsolated: true,
      sharedArrayBuffer: true,
      wasm: true,
      wasmSimd: true,
      wasmThreads: true,
      serviceWorker: true,
      webgpu: true,
      offscreenCanvas: true,
    });
  });

  it('survives a validate() that throws', () => {
    const f = detect({
      WebAssembly: {
        validate: () => {
          throw new Error('boom');
        },
      },
    });
    expect(f.wasm).toBe(true);
    expect(f.wasmSimd).toBe(false);
    expect(f.wasmThreads).toBe(false);
  });

  it('the probe modules validate on this Node (real WebAssembly)', () => {
    const f = detect(globalThis as FeatureEnvironment);
    expect(f.wasm).toBe(true);
    expect(f.wasmSimd).toBe(true);
    expect(f.wasmThreads).toBe(true);
  });
});
