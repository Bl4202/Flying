import { describe, expect, it } from 'vitest';
import { defaultFlags, parseFlags } from './flags.ts';

describe('parseFlags', () => {
  it('defaults to everything off', () => {
    expect(parseFlags('')).toEqual(defaultFlags());
    expect(parseFlags('?')).toEqual(defaultFlags());
    expect(defaultFlags().warnings).toEqual([]);
  });

  it('reads every documented flag', () => {
    const f = parseFlags(
      '?debug=perf&renderer=webgl2&flags=transfer,mainrender,timer,stall&compat=1&selftest=renderer&perf=ps0&seconds=120&bench=quick&spawn=air&assist=reduced&g=off',
    );
    expect(f).toEqual({
      debug: 'perf',
      renderer: 'webgl2',
      transfer: true,
      mainrender: true,
      timer: true,
      stall: true,
      compat: true,
      selftest: 'renderer',
      perf: 'ps0',
      seconds: 120,
      bench: 'quick',
      spawn: 'air',
      assist: 'reduced',
      g: 'off',
      warnings: [],
    });
  });

  it('maps the short forms', () => {
    expect(parseFlags('debug=1').debug).toBe('overlay');
    expect(parseFlags('selftest=1').selftest).toBe('hash');
    expect(parseFlags('bench=full').bench).toBe('full');
    for (const r of ['auto', 'webgpu', 'webgl2', 'null'] as const) {
      expect(parseFlags(`renderer=${r}`).renderer).toBe(r);
    }
  });

  it('ignores bad values, keeps the default, and says so', () => {
    const f = parseFlags(
      'debug=yes&renderer=vulkan&flags=transfer,bogus&compat=2&seconds=0&bench=slow&perf=ps9&selftest=2&spawn=moon&assist=auto&g=huge',
    );
    expect(f.debug).toBe(false);
    expect(f.renderer).toBe('auto');
    expect(f.transfer).toBe(true);
    expect(f.mainrender).toBe(false);
    expect(f.compat).toBe(false);
    expect(f.seconds).toBeUndefined();
    expect(f.bench).toBe(false);
    expect(f.perf).toBe(false);
    expect(f.selftest).toBe(false);
    expect(f.spawn).toBeUndefined();
    expect(f.assist).toBeUndefined();
    expect(f.g).toBeUndefined();
    expect(f.warnings).toHaveLength(11);
    expect(f.warnings.join('\n')).toMatch(/renderer=vulkan/);
  });

  it('bounds the duration', () => {
    expect(parseFlags('seconds=1').seconds).toBe(1);
    expect(parseFlags('seconds=3600').seconds).toBe(3600);
    for (const bad of ['3601', '-5', '1.5', 'abc', '']) {
      expect(parseFlags(`seconds=${bad}`).seconds).toBeUndefined();
    }
  });

  it('warns about unknown flags once each', () => {
    const f = parseFlags('?foo=1&foo=2&debug=1');
    expect(f.warnings).toEqual(['unknown flag ?foo']);
    expect(f.debug).toBe('overlay');
  });
});
