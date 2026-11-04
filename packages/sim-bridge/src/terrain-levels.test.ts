import { describe, expect, it } from 'vitest';
import {
  footprint,
  footprintWithin,
  holeCells,
  levelSpec,
  nextLevelCentre,
  snapCentre,
  TERRAIN_LEVEL_COUNT,
  TERRAIN_LEVELS,
  type Centre,
} from './terrain-levels.ts';

describe('the level table', () => {
  it('has four levels, each four times coarser than the one inside it', () => {
    expect(TERRAIN_LEVELS).toHaveLength(TERRAIN_LEVEL_COUNT);
    expect(TERRAIN_LEVELS.map((l) => l.spacingM)).toEqual([250, 1_000, 4_000, 16_000]);
    for (let l = 1; l < TERRAIN_LEVEL_COUNT; l++) {
      expect(levelSpec(l).spacingM).toBe(4 * levelSpec(l - 1).spacingM);
    }
  });

  it('gives footprints of 32, 128, 512 and 2 048 km', () => {
    expect(TERRAIN_LEVELS.map((l) => 2 * l.halfSpanM)).toEqual([
      32_000, 128_000, 512_000, 2_048_000,
    ]);
  });

  it('snaps each level to the next coarser spacing (L3 to 64 km)', () => {
    expect(TERRAIN_LEVELS.map((l) => l.snapM)).toEqual([1_000, 4_000, 16_000, 64_000]);
    for (let l = 0; l < TERRAIN_LEVEL_COUNT - 1; l++) {
      expect(levelSpec(l).snapM).toBe(levelSpec(l + 1).spacingM);
    }
  });

  it('refuses a level that does not exist', () => {
    expect(() => levelSpec(4)).toThrow(RangeError);
    expect(() => levelSpec(-1)).toThrow(RangeError);
  });
});

describe('snapCentre', () => {
  it('rounds to the level grid', () => {
    expect(snapCentre(0, [12_345.6, -3_499])).toEqual([12_000, -3_000]);
    expect(snapCentre(1, [12_345.6, -3_499])).toEqual([12_000, -4_000]);
    expect(snapCentre(3, [100_000, -40_000])).toEqual([128_000, -64_000]);
  });
  it('never returns negative zero', () => {
    const [x, y] = snapCentre(0, [-1, -100]);
    expect(Object.is(x, 0)).toBe(true);
    expect(Object.is(y, 0)).toBe(true);
  });
});

describe('nextLevelCentre', () => {
  it('keeps a level while the aircraft is within its distance', () => {
    expect(nextLevelCentre(0, [0, 0], [0, -6000])).toBeUndefined();
    expect(nextLevelCentre(0, [0, 0], [5600, -5600])).toBeUndefined(); // 7.9 km
    expect(nextLevelCentre(2, [0, 0], [100_000, 0])).toBeUndefined();
  });

  it('re-centres once it is further, on the level grid', () => {
    expect(nextLevelCentre(0, [0, 0], [0, 8001])).toEqual([0, 8000]);
    expect(nextLevelCentre(0, [0, 0], [12_345.6, -3_499])).toEqual([12_000, -3_000]);
    expect(nextLevelCentre(1, [0, 0], [0, 33_000])).toEqual([0, 32_000]);
    expect(nextLevelCentre(3, [0, 0], [600_000, 0])).toEqual([576_000, 0]);
  });

  it('gives a level with no centre one, wherever the aircraft is', () => {
    expect(nextLevelCentre(2, undefined, [100, 100])).toEqual([0, 0]);
  });

  it('ignores a position that is not a number', () => {
    expect(nextLevelCentre(0, [0, 0], [Number.NaN, 1e9])).toBeUndefined();
    expect(nextLevelCentre(0, undefined, [Number.NaN, 0])).toBeUndefined();
  });
});

describe('snapping keeps finer footprints on the coarser grid', () => {
  it('puts every edge of a snapped footprint on a multiple of the next coarser spacing', () => {
    for (let l = 0; l < TERRAIN_LEVEL_COUNT - 1; l++) {
      const coarse = levelSpec(l + 1).spacingM;
      for (const pos of [
        [123_456.7, -98_765.4],
        [0, 0],
        [-1, 1],
        [7e5, 3e5],
      ] as Centre[]) {
        const f = footprint(l, snapCentre(l, pos));
        for (const edge of [f.west, f.east, f.south, f.north]) {
          expect(edge / coarse).toBe(Math.round(edge / coarse));
        }
      }
    }
  });
});

describe('holeCells', () => {
  it('covers 32 coarse cells where the finer level is', () => {
    // L0 around the origin lies in the middle of L1 around the origin: cells 48 to 80.
    expect(holeCells(0, [0, 0], 1, [0, 0])).toEqual({ i0: 48, i1: 80, j0: 48, j1: 80 });
  });

  it('follows the finer level when it moves', () => {
    // L0 moved 3 km east and 2 km north.
    expect(holeCells(0, [3_000, 2_000], 1, [0, 0])).toEqual({ i0: 51, i1: 83, j0: 50, j1: 82 });
    // L1 centred 4 km west and 8 km south of L0: the hole shifts the other way.
    expect(holeCells(0, [0, 0], 1, [-4_000, -8_000])).toEqual({ i0: 52, i1: 84, j0: 56, j1: 88 });
  });

  it('works for every pair of neighbouring levels', () => {
    for (let l = 0; l < TERRAIN_LEVEL_COUNT - 1; l++) {
      expect(holeCells(l, [0, 0], l + 1, [0, 0])).toEqual({ i0: 48, i1: 80, j0: 48, j1: 80 });
    }
  });

  it('clips a hole that reaches past the coarser level and reports none when they do not meet', () => {
    const clipped = holeCells(0, [60_000, 0], 1, [0, 0]);
    expect(clipped).toEqual({ i0: 108, i1: 128, j0: 48, j1: 80 });
    expect(holeCells(0, [500_000, 0], 1, [0, 0])).toBeUndefined();
  });

  it('leaves partly covered cells in rather than open a gap', () => {
    // A finer centre off the coarse grid by 100 m: the hole is the whole cells inside it.
    const h = holeCells(0, [100, 0], 1, [0, 0]);
    expect(h).toEqual({ i0: 49, i1: 80, j0: 48, j1: 80 });
  });
});

/** A small deterministic generator, so the property test is the same every run. */
function lcg(seed: number): () => number {
  let s = seed >>> 0;
  return () => {
    s = (Math.imul(s, 1664525) + 1013904223) >>> 0;
    return s / 0x1_0000_0000;
  };
}

describe('staggered fills keep the levels nested (one level per check)', () => {
  /**
   * Flies a random course and refreshes at most one level per check, nearest first, as the Sim
   * Worker does. After every check the aircraft must be inside L0 and every finer footprint
   * inside the next coarser one.
   */
  function fly(seed: number, speedMps: number, checks: number): void {
    const rand = lcg(seed);
    const centres: (Centre | undefined)[] = [];
    let pos: [number, number] = [(rand() - 0.5) * 2e6, (rand() - 0.5) * 2e6];
    // Everything starts filled around the start, as at init.
    for (let l = 0; l < TERRAIN_LEVEL_COUNT; l++) centres[l] = snapCentre(l, pos);
    let heading = rand() * 2 * Math.PI;
    for (let n = 0; n < checks; n++) {
      heading += (rand() - 0.5) * 0.4;
      const v = speedMps * (0.2 + 0.8 * rand());
      pos = [pos[0] + v * 0.5 * Math.cos(heading), pos[1] + v * 0.5 * Math.sin(heading)];
      for (let l = 0; l < TERRAIN_LEVEL_COUNT; l++) {
        const next = nextLevelCentre(l, centres[l], pos);
        if (next !== undefined) {
          centres[l] = next;
          break; // one per check
        }
      }
      const c0 = centres[0];
      if (c0 === undefined) throw new Error('no centre');
      const inside = footprintWithin(
        { west: pos[0], east: pos[0], south: pos[1], north: pos[1] },
        footprint(0, c0),
      );
      expect(inside, `aircraft outside L0 at check ${String(n)}`).toBe(true);
      for (let l = 0; l < TERRAIN_LEVEL_COUNT - 1; l++) {
        const fine = centres[l];
        const coarse = centres[l + 1];
        if (fine === undefined || coarse === undefined) throw new Error('no centre');
        expect(
          footprintWithin(footprint(l, fine), footprint(l + 1, coarse)),
          `L${String(l)} not inside L${String(l + 1)} at check ${String(n)} (seed ${String(seed)})`,
        ).toBe(true);
      }
    }
  }

  it('holds for a slow aircraft and a very fast one', () => {
    for (let seed = 1; seed <= 6; seed++) fly(seed, 300, 4_000);
    for (let seed = 7; seed <= 12; seed++) fly(seed, 1_500, 4_000);
  });
});
