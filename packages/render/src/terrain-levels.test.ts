import { holeCells, levelSpec } from '@flying/sim-bridge/terrain-levels';
import { describe, expect, it } from 'vitest';
import { TerrainSet } from './terrain-levels.ts';
import { makeTestPatch } from './terrain-test-patch.ts';

function patchFor(level: number, centre: [number, number]): Uint8Array {
  return makeTestPatch({
    centreEast: centre[0],
    centreNorth: centre[1],
    spacingM: levelSpec(level).spacingM,
  });
}

describe('TerrainSet', () => {
  it('starts empty', () => {
    const set = new TerrainSet();
    expect(set.mask).toBe(0);
    expect(set.mesh(0)).toBeUndefined();
    expect(set.installed()).toEqual([]);
  });

  it('gives a coarser level a hole where a finer one arrives', () => {
    const set = new TerrainSet();
    expect(set.set(1, patchFor(1, [0, 0]))).toEqual([1]);
    expect(set.mesh(1)?.hole).toBeUndefined();
    expect(set.set(0, patchFor(0, [0, 0]))).toEqual([0, 1]);
    expect(set.mesh(1)?.hole).toEqual({ i0: 48, i1: 80, j0: 48, j1: 80 });
    expect(set.mesh(0)?.hole).toBeUndefined();
    expect(set.mask).toBe(0b11);
  });

  it('gives a level its hole when it arrives after the finer one', () => {
    const set = new TerrainSet();
    set.set(0, patchFor(0, [0, 0]));
    expect(set.set(1, patchFor(1, [0, 0]))).toEqual([1]);
    expect(set.mesh(1)?.hole).toEqual({ i0: 48, i1: 80, j0: 48, j1: 80 });
  });

  it('moves the hole when the finer level re-centres, and reports both levels', () => {
    const set = new TerrainSet();
    for (const l of [3, 2, 1, 0]) set.set(l, patchFor(l, [0, 0]));
    expect(set.set(0, patchFor(0, [8_000, 0]))).toEqual([0, 1]);
    expect(set.mesh(1)?.hole).toEqual(holeCells(0, [8_000, 0], 1, [0, 0]));
    // L2's hole is where L1 is, which has not moved.
    expect(set.mesh(2)?.hole).toEqual({ i0: 48, i1: 80, j0: 48, j1: 80 });
  });

  it('does not report the coarser level if its hole did not change', () => {
    const set = new TerrainSet();
    for (const l of [1, 0]) set.set(l, patchFor(l, [0, 0]));
    expect(set.set(0, patchFor(0, [0, 0]))).toEqual([0]);
  });

  it('keeps the levels apart: no cell of a coarser level where a finer one draws', () => {
    const set = new TerrainSet();
    for (const l of [3, 2, 1, 0]) set.set(l, patchFor(l, [0, 0]));
    for (let l = 1; l < 4; l++) {
      const mesh = set.mesh(l);
      expect(mesh?.surfaceTriangleCount).toBe(32_768 - 2 * 32 * 32);
    }
    expect(set.mesh(0)?.surfaceTriangleCount).toBe(32_768);
  });

  it('refuses an unknown level or a bad patch', () => {
    const set = new TerrainSet();
    expect(() => set.set(4, patchFor(0, [0, 0]))).toThrow(/SYS-040/);
    expect(() => set.set(-1, patchFor(0, [0, 0]))).toThrow(/SYS-040/);
    expect(() => set.set(0, new Uint8Array(10))).toThrow(/SYS-040/);
    expect(set.mask).toBe(0);
  });
});
