import { describe, expect, it } from 'vitest';
import { median, parseArgs } from './kernel-rate.ts';

describe('kernel-rate', () => {
  it('reads its arguments and has defaults', () => {
    expect(parseArgs([])).toEqual({ root: '.', seconds: 10, repeat: 3, warmupSeconds: 2 });
    expect(
      parseArgs(['--root', '../wt', '--seconds', '5', '--repeat', '4', '--warmup', '1']),
    ).toEqual({
      root: '../wt',
      seconds: 5,
      repeat: 4,
      warmupSeconds: 1,
    });
  });

  it('refuses what it does not understand', () => {
    expect(() => parseArgs(['--seconds', '0'])).toThrow(/positive/);
    expect(() => parseArgs(['--root'])).toThrow(/needs a value/);
    expect(() => parseArgs(['--bogus'])).toThrow(/unknown/);
  });

  it('takes the median, the middle of an odd list and the mean of the middle two of an even one', () => {
    expect(median([140_000, 100_000, 130_000])).toBe(130_000);
    expect(median([100, 200, 400, 300])).toBe(250);
    expect(median([7])).toBe(7);
  });
});
