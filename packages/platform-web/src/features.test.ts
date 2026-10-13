import { describe, expect, it, vi } from 'vitest';
import { detect, detectAsync, type FeatureEnvironment } from './features.ts';

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
      navigator: { serviceWorker: {}, gpu: {} as never },
      OffscreenCanvas: class {
        getContext(): unknown {
          return null;
        }
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

describe('detectAsync', () => {
  const canvasWith = (ctx: unknown) =>
    class {
      getContext(id: string): unknown {
        return id === 'webgl2' ? ctx : null;
      }
    };

  it('reports an adapter, its description, WebGL2, OPFS and gamepads', async () => {
    const f = await detectAsync({
      navigator: {
        gpu: {
          requestAdapter: (options?: object) => {
            expect(options).toEqual({ powerPreference: 'high-performance' });
            return Promise.resolve({
              info: { vendor: 'apple', architecture: 'metal-3', device: '', description: 'M4' },
            });
          },
        },
        storage: { getDirectory: () => undefined },
        getGamepads: () => [],
      },
      OffscreenCanvas: canvasWith({}),
    });
    expect(f.webgpuAdapter).toBe(true);
    expect(f.adapterInfo).toBe('apple metal-3 M4');
    expect(f.webgl2).toBe(true);
    expect(f.opfs).toBe(true);
    expect(f.gamepad).toBe(true);
    expect(f.webgpu).toBe(true);
  });

  it('treats a missing, null or throwing adapter as no WebGPU', async () => {
    const withGpu = (requestAdapter: () => Promise<null>) => ({
      navigator: { gpu: { requestAdapter } },
    });
    expect((await detectAsync({})).webgpuAdapter).toBe(false);
    expect((await detectAsync(withGpu(() => Promise.resolve(null)))).webgpuAdapter).toBe(false);
    const throwing = withGpu(() => Promise.reject(new Error('no gpu')));
    const f = await detectAsync(throwing);
    expect(f.webgpuAdapter).toBe(false);
    expect(f.webgpu).toBe(true); // the API exists even though no adapter does
  });

  it('gives up on an adapter request that never answers', async () => {
    vi.useFakeTimers();
    try {
      const pending = detectAsync({
        navigator: { gpu: { requestAdapter: () => new Promise<null>(() => undefined) } },
      });
      await vi.advanceTimersByTimeAsync(5000);
      expect((await pending).webgpuAdapter).toBe(false);
    } finally {
      vi.useRealTimers();
    }
  });

  it('finds WebGL2 on a document canvas and tolerates context failures', async () => {
    const viaDocument = await detectAsync({
      document: {
        createElement: () => ({ getContext: (id: string) => (id === 'webgl2' ? {} : null) }),
      },
    });
    expect(viaDocument.webgl2).toBe(true);
    expect((await detectAsync({ OffscreenCanvas: canvasWith(null) })).webgl2).toBe(false);
    const throwing = {
      document: {
        createElement: () => {
          throw new Error('no dom');
        },
      },
    };
    expect((await detectAsync(throwing)).webgl2).toBe(false);
    expect((await detectAsync({})).webgl2).toBe(false);
  });

  it('keeps the synchronous answers', async () => {
    const f = await detectAsync({
      crossOriginIsolated: true,
      WebAssembly: { validate: () => true },
    });
    expect(f.crossOriginIsolated).toBe(true);
    expect(f.wasm).toBe(true);
  });
});
