import { describe, expect, it } from 'vitest';
import { isContentAddressed } from './dist-manifest.ts';

describe('isContentAddressed', () => {
  it('allows the fixed names', () => {
    for (const n of ['index.html', 'sw.js', 'build-manifest.json', '_headers']) {
      expect(isContentAddressed(n)).toBe(true);
    }
  });
  it('accepts hashed assets', () => {
    expect(isContentAddressed('assets/index-Dk3j9sLx.js')).toBe(true);
    expect(isContentAddressed('assets/sim.single-a1B2c3D4.wasm')).toBe(true);
  });
  it('rejects unhashed assets', () => {
    expect(isContentAddressed('assets/app.js')).toBe(false);
    expect(isContentAddressed('favicon.ico')).toBe(false);
  });
});
