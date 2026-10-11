import { describe, expect, it } from 'vitest';
import { decideIsolation, withTimeout, type IsolationState } from './sw-register.ts';

const s = (o: Partial<IsolationState>): IsolationState => ({
  isolated: false,
  swSupported: true,
  swFailed: false,
  reloadedAlready: false,
  ...o,
});

describe('decideIsolation', () => {
  it('is done when already isolated', () => {
    expect(decideIsolation(s({ isolated: true }))).toBe('isolated');
    expect(decideIsolation(s({ isolated: true, reloadedAlready: true }))).toBe('isolated');
  });
  it('reloads once after a successful install', () => {
    expect(decideIsolation(s({}))).toBe('reload');
  });
  it('never reloads twice', () => {
    expect(decideIsolation(s({ reloadedAlready: true }))).toBe('degraded');
  });
  it('degrades when service workers are unavailable or failed', () => {
    expect(decideIsolation(s({ swSupported: false }))).toBe('degraded');
    expect(decideIsolation(s({ swFailed: true }))).toBe('degraded');
  });
});

describe('withTimeout', () => {
  it('resolves with the value when fast enough', async () => {
    await expect(withTimeout(Promise.resolve(7), 100, 'x')).resolves.toBe(7);
  });
  it('rejects a promise that never settles', async () => {
    await expect(withTimeout(new Promise<never>(() => undefined), 20, 'thing')).rejects.toThrow(
      /thing timed out after 20 ms/,
    );
  });
  it('propagates rejection', async () => {
    await expect(withTimeout(Promise.reject(new Error('boom')), 100, 'x')).rejects.toThrow('boom');
  });
});
