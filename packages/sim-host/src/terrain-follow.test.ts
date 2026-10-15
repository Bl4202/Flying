import { describe, expect, it } from 'vitest';
import { nextPatchCentre } from './terrain-follow.ts';

describe('terrain follow', () => {
  it('keeps the patch while the aircraft is near its middle', () => {
    expect(nextPatchCentre([0, 0], [0, -6000])).toBeUndefined();
    expect(nextPatchCentre([0, 0], [5600, -5600])).toBeUndefined(); // 7.9 km
  });

  it('re-centres on a 1 km grid once it is more than 8 km out', () => {
    expect(nextPatchCentre([0, 0], [0, 8001])).toEqual([0, 8000]);
    expect(nextPatchCentre([0, 0], [12_345.6, -3_499])).toEqual([12_000, -3_000]);
    expect(nextPatchCentre([12_000, -3_000], [12_100, -3_100])).toBeUndefined();
  });

  it('ignores a position that is not a number', () => {
    expect(nextPatchCentre([0, 0], [Number.NaN, 1e9])).toBeUndefined();
  });
});
