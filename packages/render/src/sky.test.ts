import { describe, expect, it } from 'vitest';
import { WORLD } from '@flying/sim-bridge/layout';
import { eyeGeometry, globeFloorM, horizonDip, horizonDistance, newEyeGeometry } from './sky.ts';

const deg = (rad: number): number => (rad * 180) / Math.PI;

describe('the horizon against GDD 7.4.2', () => {
  // Observer height, dip (degrees), geometric horizon (km), as the table gives them.
  const table = [
    [100, 0.32, 35.7],
    [1_000, 1.02, 112.9],
    [5_000, 2.27, 252.5],
    [10_000, 3.21, 357.1],
    [15_000, 3.93, 437.4],
  ] as const;

  it('dips by the tabulated angle at every height (0.005 degrees)', () => {
    for (const [h, dip] of table) {
      const geometry = eyeGeometry(newEyeGeometry(), [0, 0, h], [0, 0], 0);
      expect(deg(Math.asin(geometry.sinDip)), `${String(h)} m`).toBeCloseTo(dip, 2);
      expect(Math.abs(deg(Math.asin(geometry.sinDip)) - dip), `${String(h)} m`).toBeLessThan(0.005);
    }
  });

  it('puts the horizon at the tabulated distance', () => {
    for (const [h, , km] of table) {
      expect(horizonDistance(h) / 1000).toBeCloseTo(km, 1);
      const geometry = eyeGeometry(newEyeGeometry(), [0, 0, h], [0, 0], 0);
      expect(Math.sqrt(geometry.chordTerm) / 1000).toBeCloseTo(km, 1);
    }
  });

  it('agrees with the closed form acos(R / (R + h)) to rounding', () => {
    for (const h of [1, 10, 100, 1_234, 20_000]) {
      const geometry = eyeGeometry(newEyeGeometry(), [0, 0, h], [0, 0], 0);
      expect(Math.asin(geometry.sinDip)).toBeCloseTo(horizonDip(h), 12);
    }
  });

  it('measures the dip from the globe: the eye 40 m up over a floor at 37 m is 3 m high', () => {
    const g = eyeGeometry(newEyeGeometry(), [0, 0, 40], [0, 0], 37);
    expect(g.heightM).toBeCloseTo(3, 6);
    expect(g.globeRadiusM).toBe(WORLD.EARTH_RADIUS_M + 37);
    expect(Math.asin(g.sinDip)).toBeCloseTo(horizonDip(3, 37), 12);
  });

  it('has the dip at 10 km over the plain arena within 0.05 degrees of the 40 m base', () => {
    // The gate of S1: the eye 10 km above a base of 40 m, the globe at 37 m.
    const g = eyeGeometry(newEyeGeometry(), [0, 0, 40 + 10_000], [0, 0], globeFloorM(1));
    expect(Math.abs(deg(Math.asin(g.sinDip)) - deg(horizonDip(10_000, 40)))).toBeLessThan(0.01);
  });
});

describe('eyeGeometry', () => {
  it('has local up straight up over the anchor, and tilted with distance from it', () => {
    const over = eyeGeometry(newEyeGeometry(), [0, 0, 100], [0, 0], 0);
    expect(over.up).toEqual([0, 1, 0]);
    // 100 km east: up leans 100 / 6371 rad east.
    const east = eyeGeometry(newEyeGeometry(), [100_000, 0, 0], [0, 0], 0);
    expect(Math.asin(east.up[0])).toBeCloseTo(100_000 / WORLD.EARTH_RADIUS_M, 3);
    expect(east.up[2]).toBeCloseTo(0, 12);
    // North is -z in render axes, so an eye north of the anchor has an up that leans towards -z.
    const north = eyeGeometry(newEyeGeometry(), [0, 100_000, 0], [0, 0], 0);
    expect(north.up[2]).toBeLessThan(0);
    expect(Math.hypot(...north.up)).toBeCloseTo(1, 12);
  });

  it('moves the globe with the anchor, so a scene shifted 400 km looks the same', () => {
    const a = eyeGeometry(newEyeGeometry(), [0, -8000, 1500], [0, 0], 0);
    const b = eyeGeometry(newEyeGeometry(), [400_000, 400_000 - 8000, 1500], [400_000, 400_000], 0);
    expect(b.heightM).toBeCloseTo(a.heightM, 6);
    expect(b.sinDip).toBeCloseTo(a.sinDip, 12);
    expect(b.up[0]).toBeCloseTo(a.up[0], 12);
    expect(b.up[1]).toBeCloseTo(a.up[1], 12);
    expect(b.up[2]).toBeCloseTo(a.up[2], 12);
  });

  it('keeps its precision at 2 m above the ground', () => {
    const g = eyeGeometry(newEyeGeometry(), [0, 0, 2], [0, 0], 0);
    expect(g.heightM).toBeCloseTo(2, 9);
    expect(g.eyeMslM).toBeCloseTo(2, 9);
    expect(g.chordTerm).toBeCloseTo(2 * (2 * WORLD.EARTH_RADIUS_M + 2), 3);
  });

  it('has no horizon dip for an eye at or under the globe', () => {
    const g = eyeGeometry(newEyeGeometry(), [0, 0, -5], [0, 0], 0);
    expect(g.sinDip).toBe(0);
    expect(g.chordTerm).toBe(0);
  });

  it('is flat with the seeded fault: no dip, up is the anchor up, the height is still the height', () => {
    const g = eyeGeometry(newEyeGeometry(), [300_000, 0, 10_000], [0, 0], 0, true);
    expect(g.sinDip).toBe(0);
    expect(g.chordTerm).toBe(0);
    expect(g.up).toEqual([0, 1, 0]);
    expect(g.heightM).toBeGreaterThan(9_000);
  });
});

describe('globeFloorM', () => {
  it('is the arena lower bound: 0 m for rugged, 37 m for plain', () => {
    expect(globeFloorM(0)).toBe(0);
    expect(globeFloorM(1)).toBe(37);
  });
});
