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

describe('heightAt', () => {
  // The test patch: base 300 m, rolling hills of 150 m amplitude, wavelength 2.5 km.
  const hills = (x: number, y: number): number =>
    300 + 150 * (Math.sin(x / 2500) + Math.cos(y / 2500));

  it('is exact at a post and close between posts', () => {
    const set = new TerrainSet();
    set.set(0, patchFor(0, [0, 0]));
    // Posts are 250 m apart; the middle post is the centre.
    expect(set.heightAt(0, 0)).toBeCloseTo(hills(0, 0), 3);
    expect(set.heightAt(250, 500)).toBeCloseTo(hills(250, 500), 3);
    // Between posts the surface is a facet: within a metre of the smooth hills here.
    expect(Math.abs(set.heightAt(137.5, -62.5) - hills(137.5, -62.5))).toBeLessThan(1);
  });

  it('uses the finest level that covers the point, and a coarser one beyond it', () => {
    const set = new TerrainSet();
    set.set(0, patchFor(0, [0, 0]));
    set.set(1, patchFor(1, [0, 0]));
    // 10 km east is inside both (L0 reaches 16 km): the finer one answers, and is the closer.
    expect(Math.abs(set.heightAt(10_000, 0) - hills(10_000, 0))).toBeLessThan(1);
    // 40 km east is only in L1 (spacing 1 km): coarser, still on the hills.
    expect(Math.abs(set.heightAt(40_000, 0) - hills(40_000, 0))).toBeLessThan(20);
    expect(Number.isNaN(set.heightAt(100_000, 0))).toBe(true);
  });

  it('is NaN with no terrain, and at the very edge it still answers', () => {
    expect(Number.isNaN(new TerrainSet().heightAt(0, 0))).toBe(true);
    const set = new TerrainSet();
    set.set(0, patchFor(0, [0, 0]));
    expect(Number.isNaN(set.heightAt(16_000, 16_000))).toBe(false);
    expect(Number.isNaN(set.heightAt(16_000.5, 0))).toBe(true);
    expect(Number.isNaN(set.heightAt(Number.NaN, 0))).toBe(true);
  });

  it('follows the very triangles the GPU draws: the diagonal runs from the south-east post to the north-west', () => {
    const set = new TerrainSet();
    set.set(0, patchFor(0, [0, 0]));
    const mesh = set.mesh(0);
    if (!mesh) throw new Error('no mesh');
    // The cell north-east of the centre post: posts a (0,0), b (250,0), c (0,250), d (250,250).
    const [ha, hb, hc, hd] = [
      set.heightAt(0, 0),
      set.heightAt(250, 0),
      set.heightAt(0, 250),
      set.heightAt(250, 250),
    ];
    // On the diagonal b-c the height is the mean of those two.
    expect(set.heightAt(125, 125)).toBeCloseTo((hb + hc) / 2, 4);
    // On the other diagonal a-d it is not (unless the cell is a plane), which is how a bilinear
    // lookup would differ from the drawn surface.
    expect(set.heightAt(62.5, 62.5)).toBeCloseTo(ha + 0.25 * (hb - ha) + 0.25 * (hc - ha), 4);
    expect(hd).toBeDefined();
  });
});

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
