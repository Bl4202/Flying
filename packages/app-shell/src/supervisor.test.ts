import { describe, expect, it } from 'vitest';
import { SILENCE_WARN_MS, Supervisor } from './supervisor.ts';

function rig() {
  let now = 1000;
  let last = 0;
  const warnings: number[] = [];
  const sup = new Supervisor({
    lastReportAt: () => last,
    now: () => now,
    onSilent: (s) => warnings.push(s),
  });
  return {
    sup,
    warnings,
    advance: (ms: number) => {
      now += ms;
    },
    report: () => {
      last = now;
    },
  };
}

describe('Supervisor', () => {
  it('stays quiet while reports keep arriving', () => {
    const r = rig();
    for (let i = 0; i < 60; i++) {
      r.advance(1000);
      r.report();
      r.sup.check();
    }
    expect(r.warnings).toEqual([]);
  });

  it('warns once after five seconds of silence, with the seconds', () => {
    const r = rig();
    r.report();
    r.advance(SILENCE_WARN_MS - 1);
    r.sup.check();
    expect(r.warnings).toEqual([]);
    r.advance(1);
    r.sup.check();
    expect(r.warnings).toEqual([5]);
    r.advance(1000);
    r.sup.check();
    expect(r.warnings).toEqual([5]);
  });

  it('warns again after each further stretch of silence, and not after recovery', () => {
    const r = rig();
    r.report();
    r.advance(SILENCE_WARN_MS);
    r.sup.check();
    r.advance(SILENCE_WARN_MS);
    r.sup.check();
    expect(r.warnings).toEqual([5, 10]);
    r.report();
    r.advance(1000);
    r.sup.check();
    expect(r.warnings).toEqual([5, 10]);
  });

  it('counts silence from when watching began if no report ever came', () => {
    const r = rig();
    r.advance(SILENCE_WARN_MS);
    r.sup.check();
    expect(r.warnings).toEqual([5]);
  });
});
