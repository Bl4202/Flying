import { describe, expect, it } from 'vitest';
import { flowReducer, INITIAL_FLOW, type FlowEvent, type FlowState } from './gameflow.ts';

function run(events: FlowEvent[], from: FlowState = INITIAL_FLOW): FlowState {
  return events.reduce(flowReducer, from);
}

const BEGIN: FlowEvent = { type: 'begin-check' };
const OK_SILENT: FlowEvent = { type: 'check-done', verdict: 'ok', showScreen: false };
const OK_SCREEN: FlowEvent = { type: 'check-done', verdict: 'degraded', showScreen: true };
const FATAL_CHECK: FlowEvent = {
  type: 'check-done',
  verdict: 'fatal',
  showScreen: true,
  code: 'SYS-102',
};
const FATAL_RUNTIME: FlowEvent = { type: 'fatal', code: 'SYS-032', message: 'worker died' };

describe('gameflow', () => {
  it('starts in BOOT', () => {
    expect(INITIAL_FLOW).toEqual({ name: 'BOOT' });
  });

  it('goes BOOT -> COMPAT_CHECK -> DEMO when the screen is not needed', () => {
    expect(run([BEGIN])).toEqual({ name: 'COMPAT_CHECK', phase: 'probing' });
    expect(run([BEGIN, OK_SILENT])).toEqual({ name: 'DEMO', paused: false, menu: null });
  });

  it('waits for the person when the screen is shown', () => {
    const waiting = run([BEGIN, OK_SCREEN]);
    expect(waiting).toEqual({ name: 'COMPAT_CHECK', phase: 'waiting' });
    expect(flowReducer(waiting, { type: 'acknowledge' })).toEqual({
      name: 'DEMO',
      paused: false,
      menu: null,
    });
  });

  it('goes to FATAL, with the code, when the check finds a missing required feature', () => {
    const s = run([BEGIN, FATAL_CHECK]);
    expect(s).toMatchObject({ name: 'FATAL', reason: 'compat', code: 'SYS-102' });
  });

  it('cannot be acknowledged out of FATAL', () => {
    const s = run([BEGIN, FATAL_CHECK]);
    expect(flowReducer(s, { type: 'acknowledge' })).toBe(s);
    expect(flowReducer(s, { type: 'begin-check' })).toBe(s);
    expect(flowReducer(s, OK_SILENT)).toBe(s);
  });

  it('pauses and resumes in DEMO only', () => {
    const demo = run([BEGIN, OK_SILENT]);
    const paused = flowReducer(demo, { type: 'pause' });
    expect(paused).toEqual({ name: 'DEMO', paused: true, menu: null });
    expect(flowReducer(paused, { type: 'pause' })).toBe(paused);
    expect(flowReducer(paused, { type: 'resume' })).toEqual({
      name: 'DEMO',
      paused: false,
      menu: null,
    });
    expect(flowReducer(demo, { type: 'resume' })).toBe(demo);
    expect(flowReducer(INITIAL_FLOW, { type: 'pause' })).toBe(INITIAL_FLOW);
  });

  it('opens and closes a menu over the flight, in DEMO only, independently of the pause', () => {
    const demo = run([BEGIN, OK_SILENT]);
    const paused = flowReducer(demo, { type: 'menu', menu: 'pause' });
    expect(paused).toEqual({ name: 'DEMO', paused: false, menu: 'pause' });
    expect(flowReducer(paused, { type: 'menu', menu: 'pause' })).toBe(paused);
    const controls = flowReducer(paused, { type: 'menu', menu: 'controls' });
    expect(controls).toEqual({ name: 'DEMO', paused: false, menu: 'controls' });
    // The overlay's pause and the menu do not undo each other.
    const both = flowReducer(controls, { type: 'pause' });
    expect(both).toEqual({ name: 'DEMO', paused: true, menu: 'controls' });
    expect(flowReducer(both, { type: 'menu', menu: null })).toEqual({
      name: 'DEMO',
      paused: true,
      menu: null,
    });
    expect(flowReducer(both, { type: 'resume' })).toEqual({
      name: 'DEMO',
      paused: false,
      menu: 'controls',
    });
    // Not in the other states.
    for (const s of [
      INITIAL_FLOW,
      run([BEGIN]),
      run([BEGIN, OK_SCREEN]),
      run([BEGIN, FATAL_CHECK]),
    ]) {
      expect(flowReducer(s, { type: 'menu', menu: 'pause' })).toBe(s);
    }
    expect(flowReducer(demo, { type: 'menu', menu: null })).toBe(demo);
  });

  it('a runtime failure ends in FATAL from any live state, and nothing leaves it', () => {
    for (const s of [
      INITIAL_FLOW,
      run([BEGIN]),
      run([BEGIN, OK_SILENT]),
      run([BEGIN, OK_SCREEN]),
    ]) {
      const f = flowReducer(s, FATAL_RUNTIME);
      expect(f).toEqual({
        name: 'FATAL',
        reason: 'runtime',
        code: 'SYS-032',
        message: 'worker died',
      });
      expect(flowReducer(f, FATAL_RUNTIME)).toBe(f);
    }
  });

  it('ignores events that do not fit the state, returning the same object', () => {
    expect(flowReducer(INITIAL_FLOW, OK_SILENT)).toBe(INITIAL_FLOW);
    expect(flowReducer(INITIAL_FLOW, { type: 'acknowledge' })).toBe(INITIAL_FLOW);
    const probing = run([BEGIN]);
    expect(flowReducer(probing, BEGIN)).toBe(probing);
    expect(flowReducer(probing, { type: 'acknowledge' })).toBe(probing);
    const demo = run([BEGIN, OK_SILENT]);
    expect(flowReducer(demo, BEGIN)).toBe(demo);
    expect(flowReducer(demo, OK_SCREEN)).toBe(demo);
    expect(flowReducer(demo, { type: 'acknowledge' })).toBe(demo);
  });

  it('only ever reaches states that exist, whatever the order of events', () => {
    const events: FlowEvent[] = [
      BEGIN,
      OK_SILENT,
      OK_SCREEN,
      FATAL_CHECK,
      { type: 'acknowledge' },
      { type: 'pause' },
      { type: 'resume' },
      { type: 'menu', menu: 'pause' },
      FATAL_RUNTIME,
    ];
    const names = new Set(['BOOT', 'COMPAT_CHECK', 'DEMO', 'FATAL']);
    const seen = new Set<string>();
    const walk = (s: FlowState, depth: number): void => {
      seen.add(s.name);
      expect(names.has(s.name)).toBe(true);
      if (depth === 0) return;
      for (const e of events) walk(flowReducer(s, e), depth - 1);
    };
    walk(INITIAL_FLOW, 4);
    expect([...seen].sort()).toEqual(['BOOT', 'COMPAT_CHECK', 'DEMO', 'FATAL']);
  });
});
