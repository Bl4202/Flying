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
      '?debug=perf&renderer=webgl2&flags=transfer,mainrender,timer,stall&compat=1&selftest=renderer&perf=ps0&seconds=120&bench=quick&spawn=air&assist=reduced&g=off&script=circuit',
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
      script: 'circuit',
      warnings: [],
    });
  });

  it('maps the short forms', () => {
    expect(parseFlags('debug=1').debug).toBe('overlay');
    expect(parseFlags('selftest=1').selftest).toBe('hash');
    expect(parseFlags('selftest=audio').selftest).toBe('audio');
    expect(parseFlags('bench=full').bench).toBe('full');
    for (const r of ['auto', 'webgpu', 'webgl2', 'null'] as const) {
      expect(parseFlags(`renderer=${r}`).renderer).toBe(r);
    }
  });

  it('ignores bad values, keeps the default, and says so', () => {
    const f = parseFlags(
      'debug=yes&renderer=vulkan&flags=transfer,bogus&compat=2&seconds=0&bench=slow&perf=ps9&selftest=2&spawn=moon&assist=auto&g=huge&script=bot',
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
    expect(f.script).toBe(false);
    expect(f.warnings).toHaveLength(12);
    expect(f.warnings.join('\n')).toMatch(/renderer=vulkan/);
  });

  it('reads the drawing-buffer size, the preset and the seeded fault', () => {
    const f = parseFlags('?debug=1&res=1920x1080&quality=high&fault=flat-earth');
    expect(f.res).toEqual({ width: 1920, height: 1080 });
    expect(f.quality).toBe('high');
    expect(f.fault).toBe('flat-earth');
    expect(f.warnings).toEqual([]);
  });

  it('bounds the drawing-buffer size and warns about nonsense', () => {
    expect(parseFlags('res=160x90').res).toEqual({ width: 160, height: 90 });
    expect(parseFlags('res=7680x4320').res).toEqual({ width: 7680, height: 4320 });
    for (const bad of [
      '159x90',
      '160x89',
      '7681x4320',
      '1920',
      '1920x',
      'x1080',
      '1920X1080',
      '0x0',
      '1e3x1e3',
    ]) {
      const f = parseFlags(`res=${bad}`);
      expect(f.res, bad).toBeUndefined();
      expect(f.warnings, bad).toHaveLength(1);
    }
    expect(parseFlags('quality=ultra').quality).toBeUndefined();
    expect(parseFlags('quality=ultra').warnings).toHaveLength(1);
  });

  it('takes a seeded fault only with the debug overlay on', () => {
    const f = parseFlags('fault=flat-earth');
    expect(f.fault).toBeUndefined();
    expect(f.warnings.join()).toMatch(/fault=flat-earth/);
    expect(parseFlags('debug=perf&fault=flat-earth').fault).toBe('flat-earth');
    expect(parseFlags('debug=1&fault=round-earth').fault).toBeUndefined();
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
