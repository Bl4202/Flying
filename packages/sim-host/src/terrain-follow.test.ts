import { describe, expect, it } from 'vitest';
import { TerrainFollower } from './terrain-follow.ts';

function filledAround(pos: [number, number]): TerrainFollower {
  const f = new TerrainFollower();
  for (let guard = 0; guard < 8; guard++) {
    const next = f.next(pos);
    if (next === undefined) break;
    f.filled(next.level, next.centre);
  }
  return f;
}

describe('terrain follower', () => {
  it('asks for the levels one at a time, nearest first, until all are filled', () => {
    const f = new TerrainFollower();
    const order: number[] = [];
    for (let guard = 0; guard < 8; guard++) {
      const next = f.next([0, 0]);
      if (next === undefined) break;
      order.push(next.level);
      f.filled(next.level, next.centre);
    }
    expect(order).toEqual([0, 1, 2, 3]);
    expect(f.next([0, 0])).toBeUndefined();
  });

  it('keeps every level while the aircraft is near the middle of each', () => {
    const f = filledAround([0, 0]);
    expect(f.next([0, -6000])).toBeUndefined();
    expect(f.next([5600, -5600])).toBeUndefined(); // 7.9 km
  });

  it('refreshes only the level the aircraft has left, snapped to its grid', () => {
    const f = filledAround([0, 0]);
    expect(f.next([0, 8001])).toEqual({ level: 0, centre: [0, 8000] });
    f.filled(0, [0, 8000]);
    expect(f.next([0, 8001])).toBeUndefined();
    // 33 km out, L0 is two refreshes behind and L1 is due as well: nearest first.
    expect(f.next([0, 33_000])).toEqual({ level: 0, centre: [0, 33_000] });
    f.filled(0, [0, 33_000]);
    expect(f.next([0, 33_000])).toEqual({ level: 1, centre: [0, 32_000] });
  });

  it('forgets everything on reset', () => {
    const f = filledAround([0, 0]);
    f.reset();
    expect(f.centre(2)).toBeUndefined();
    expect(f.next([0, 0])?.level).toBe(0);
  });

  it('ignores a position that is not a number once the levels are filled', () => {
    const f = filledAround([0, 0]);
    expect(f.next([Number.NaN, 1e9])).toBeUndefined();
  });
});
