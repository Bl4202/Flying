import { describe, expect, it } from 'vitest';
import { preprocess, WgslPreprocessError } from './wgsl-pp.ts';

function vfs(files: Record<string, string>): (path: string) => string {
  return (path) => {
    const text = files[path];
    if (text === undefined) {
      throw new Error(`ENOENT ${path}`);
    }
    return text;
  };
}

describe('preprocess', () => {
  it('passes plain WGSL through unchanged', () => {
    const readFile = vfs({ '/s/a.wgsl': 'fn f() {}' });
    expect(preprocess('/s/a.wgsl', { readFile })).toBe('fn f() {}');
  });

  it('includes relative to the including file, with markers', () => {
    const readFile = vfs({
      '/s/main.wgsl': '#include "lib/util.wgsl"\nfn main() {}',
      '/s/lib/util.wgsl': 'fn util() {}',
    });
    expect(preprocess('/s/main.wgsl', { readFile })).toBe(
      ['// >>> include lib/util.wgsl', 'fn util() {}', '// <<< lib/util.wgsl', 'fn main() {}'].join(
        '\n',
      ),
    );
  });

  it('emits a diamond include only once', () => {
    const readFile = vfs({
      '/s/main.wgsl': '#include "a.wgsl"\n#include "b.wgsl"',
      '/s/a.wgsl': '#include "c.wgsl"\nfn a() {}',
      '/s/b.wgsl': '#include "c.wgsl"\nfn b() {}',
      '/s/c.wgsl': 'fn c() {}',
    });
    const out = preprocess('/s/main.wgsl', { readFile });
    expect(out.match(/fn c\(\)/g)).toHaveLength(1);
  });

  it('substitutes defines as whole words, only after the define', () => {
    const readFile = vfs({
      '/s/a.wgsl': 'let before = N;\n#define N 8.0\nlet after = N + NN;',
    });
    const out = preprocess('/s/a.wgsl', { readFile });
    expect(out).toContain('let before = N;');
    expect(out).toContain('let after = 8.0 + NN;');
  });

  it('applies predefined defines', () => {
    const readFile = vfs({ '/s/a.wgsl': 'let x = QUALITY;' });
    expect(preprocess('/s/a.wgsl', { readFile, defines: { QUALITY: '2' } })).toBe('let x = 2;');
  });

  it('reports an include cycle with the chain', () => {
    const readFile = vfs({
      '/s/a.wgsl': '#include "b.wgsl"',
      '/s/b.wgsl': '#include "a.wgsl"',
    });
    expect(() => preprocess('/s/a.wgsl', { readFile })).toThrow(
      /include cycle: a\.wgsl -> b\.wgsl -> a\.wgsl/,
    );
  });

  it('reports a missing include with the location', () => {
    const readFile = vfs({ '/s/a.wgsl': 'x\n#include "nope.wgsl"' });
    expect(() => preprocess('/s/a.wgsl', { readFile })).toThrow(
      /include not found: nope\.wgsl \(from a\.wgsl:2\)/,
    );
  });

  it('rejects unsupported directives', () => {
    const readFile = vfs({ '/s/a.wgsl': '#ifdef FOO' });
    expect(() => preprocess('/s/a.wgsl', { readFile })).toThrow(WgslPreprocessError);
  });
});
