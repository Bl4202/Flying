import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import { wgsl } from './vite-plugin-wgsl.ts';

type Load = (this: unknown, id: string) => string | undefined;

describe('the WGSL Vite plugin', () => {
  const load = wgsl().load as Load;

  it('turns a .wgsl import into a string module of the preprocessed source', () => {
    const dir = mkdtempSync(join(tmpdir(), 'fg-wgsl-plugin-'));
    try {
      writeFileSync(join(dir, 'common.wgsl'), 'const K : f32 = 1.0;');
      writeFileSync(
        join(dir, 'a.wgsl'),
        '#include "common.wgsl"\n#define N 4\nfn f() -> f32 { return K * N; }',
      );
      const code = load.call({}, `${join(dir, 'a.wgsl')}?import`) ?? '';
      const source = JSON.parse(code.replace(/^export default /, '').replace(/;$/, '')) as string;
      expect(source).toContain('const K : f32 = 1.0;');
      expect(source).toContain('return K * 4;');
      expect(source).not.toContain('#include');
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });

  it('leaves other files alone', () => {
    expect(load.call({}, '/x/y.ts')).toBeUndefined();
    expect(load.call({}, '/x/y.glsl')).toBeUndefined();
  });
});
