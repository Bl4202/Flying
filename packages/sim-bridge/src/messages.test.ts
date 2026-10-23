import { describe, expect, it } from 'vitest';
import {
  parseSimCommand,
  parseSimEvent,
  SIM_COMMAND_TYPES,
  SIM_EVENT_TYPES,
  type SimInit,
} from './messages.ts';

const init: SimInit = {
  type: 'init',
  variant: 'single',
  wasmUrl: '/a.wasm',
  threadsInitialPages: 256,
  contentUrl: '/c.cpack',
  seed: '1f',
  script: 1,
  session: { spawn: 1, assist: 2, gEffects: 2 },
  snapshot: { kind: 'shared', sab: new SharedArrayBuffer(4) },
  input: { kind: 'shared', sab: new SharedArrayBuffer(4) },
  terrainPort: null,
  wake: null,
  forceTimer: false,
  profiling: false,
  allowStall: false,
};

describe('parseSimCommand', () => {
  it('accepts every well-formed command', () => {
    for (const c of [
      init,
      { type: 'start' },
      { type: 'pause' },
      { type: 'resume' },
      { type: 'stop' },
      { type: 'hash', id: 3 },
      { type: 'profiling', on: true },
      { type: 'reset-run' },
      { type: 'stall', ms: 100 },
    ]) {
      expect(parseSimCommand(c)).toBe(c);
    }
  });

  it('names what is wrong', () => {
    expect(() => parseSimCommand(null)).toThrow(/SYS-030.*not an object/);
    expect(() => parseSimCommand('start')).toThrow(/SYS-030/);
    expect(() => parseSimCommand({})).toThrow(/unknown command type undefined/);
    expect(() => parseSimCommand({ type: 'explode' })).toThrow(/unknown command type explode/);
    expect(() => parseSimCommand({ type: 'hash' })).toThrow(/hash.*id: number/);
    expect(() => parseSimCommand({ type: 'stall', ms: '100' })).toThrow(/stall.*ms: number/);
    expect(() => parseSimCommand({ ...init, seed: 5 })).toThrow(/init.*seed: string/);
    expect(() => parseSimCommand({ ...init, input: null })).toThrow(/init.*input/);
    expect(() => parseSimCommand({ ...init, session: null })).toThrow(/init.*session/);
    expect(() => parseSimCommand({ ...init, session: undefined })).toThrow(/init.*session/);
    expect(() => parseSimCommand({ ...init, forceTimer: 1 })).toThrow(/forceTimer: boolean/);
  });

  it('lets any value through for the fields documented as nullable', () => {
    expect(() => parseSimCommand({ ...init, wake: new SharedArrayBuffer(4) })).not.toThrow();
    expect(() => parseSimCommand({ ...init, terrainPort: null })).not.toThrow();
  });
});

describe('parseSimEvent', () => {
  it('accepts every well-formed event', () => {
    for (const e of [
      { type: 'ready', abi: 2, variant: 'single', sleeper: 'timer', contentHash: 'ab' },
      { type: 'stats', stats: {} },
      { type: 'state', coreState: 2 },
      { type: 'hash', id: 1, tick: 5, hashes: {} },
      { type: 'error', message: 'x' },
    ]) {
      expect(parseSimEvent(e)).toBe(e);
    }
  });

  it('rejects the rest', () => {
    expect(() => parseSimEvent({ type: 'stats' })).toThrow(/stats.*stats: object/);
    expect(() => parseSimEvent({ type: 'stats', stats: null })).toThrow(/needs stats, got null/);
    expect(() => parseSimEvent({ type: 'ready', abi: '2' })).toThrow(/abi: number/);
    expect(() => parseSimEvent(7)).toThrow(/SYS-030/);
  });
});

describe('message tables', () => {
  it('list every type once', () => {
    expect(new Set(SIM_COMMAND_TYPES).size).toBe(SIM_COMMAND_TYPES.length);
    expect(SIM_COMMAND_TYPES).toContain('init');
    expect(SIM_EVENT_TYPES).toEqual(['ready', 'stats', 'state', 'hash', 'error']);
  });
});
