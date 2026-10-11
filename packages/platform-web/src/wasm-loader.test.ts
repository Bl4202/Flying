import { describe, expect, it } from 'vitest';
import type { Features } from './features.ts';
import { buildImports, chooseVariant, hex64, MAX_PAGES } from './wasm-loader.ts';

const base: Features = {
  crossOriginIsolated: true,
  sharedArrayBuffer: true,
  wasm: true,
  wasmSimd: true,
  wasmThreads: true,
  serviceWorker: true,
  webgpu: true,
  offscreenCanvas: true,
};

describe('chooseVariant', () => {
  it('prefers threads when isolated', () => {
    expect(chooseVariant(base)).toBe('threads');
  });
  it('falls back to single without isolation', () => {
    expect(chooseVariant({ ...base, crossOriginIsolated: false })).toBe('single');
  });
  it('falls back to single without SharedArrayBuffer or threads', () => {
    expect(chooseVariant({ ...base, sharedArrayBuffer: false })).toBe('single');
    expect(chooseVariant({ ...base, wasmThreads: false })).toBe('single');
  });
  it('has no variant without wasm or simd', () => {
    expect(chooseVariant({ ...base, wasm: false })).toBeUndefined();
    expect(chooseVariant({ ...base, wasmSimd: false })).toBeUndefined();
  });
});

describe('buildImports', () => {
  it('single needs no imports', () => {
    expect(buildImports('single', 256)).toEqual({});
  });
  it('threads provides a shared memory with the right bounds', () => {
    const imports = buildImports('threads', 256);
    const memory = (imports['env'] as { memory: WebAssembly.Memory }).memory;
    expect(memory.buffer).toBeInstanceOf(SharedArrayBuffer);
    expect(memory.buffer.byteLength).toBe(256 * 65536);
    expect(MAX_PAGES * 65536).toBe(2 * 1024 * 1024 * 1024);
  });
});

describe('hex64', () => {
  it('pads to 16 digits', () => {
    expect(hex64(0n)).toBe('0000000000000000');
    expect(hex64(0xdeadbeefn)).toBe('00000000deadbeef');
    expect(hex64(0xffffffffffffffffn)).toBe('ffffffffffffffff');
  });
});
