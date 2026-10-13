import { describe, expect, it } from 'vitest';
import { monotonicMs, sharedNowMs } from './time.ts';

describe('time', () => {
  it('shared time sits on the wall-clock axis', () => {
    expect(Math.abs(sharedNowMs() - Date.now())).toBeLessThan(50);
  });
  it('monotonic time never goes back', () => {
    let last = monotonicMs();
    for (let i = 0; i < 1000; i++) {
      const t = monotonicMs();
      expect(t).toBeGreaterThanOrEqual(last);
      last = t;
    }
  });
  it('two readings of shared time agree with each other to the millisecond', () => {
    const a = sharedNowMs();
    const b = sharedNowMs();
    expect(b - a).toBeGreaterThanOrEqual(0);
    expect(b - a).toBeLessThan(5);
  });
});
