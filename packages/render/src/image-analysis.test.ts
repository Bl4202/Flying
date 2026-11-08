import { describe, expect, it } from 'vitest';
import {
  elevationRow,
  expectedHorizonRow,
  fittedAngleDeg,
  horizonDipDeg,
  lumaAt,
  rmsAndMax,
  rowElevationDeg,
  strongestStepRow,
  type RgbaImage,
} from './image-analysis.ts';

const DEG = Math.PI / 180;

/** A grey image whose columns are `sky` above a boundary at `edgeRow(x)` and `ground` below it, with the edge pixel mixed by coverage. */
function horizonImage(
  width: number,
  height: number,
  edgeRow: (x: number) => number,
  sky = 200,
  ground = 80,
): RgbaImage {
  const pixels = new Uint8Array(width * height * 4);
  for (let x = 0; x < width; x++) {
    const edge = edgeRow(x);
    for (let y = 0; y < height; y++) {
      // The share of this pixel (rows y to y + 1) that is sky.
      const skyShare = Math.max(0, Math.min(1, edge - y));
      const v = Math.round(ground + (sky - ground) * skyShare);
      const i = (y * width + x) * 4;
      pixels[i] = v;
      pixels[i + 1] = v;
      pixels[i + 2] = v;
      pixels[i + 3] = 255;
    }
  }
  return { width, height, pixels };
}

describe('lumaAt', () => {
  it('weights green most (Rec. 709) and ignores alpha', () => {
    const img: RgbaImage = {
      width: 2,
      height: 1,
      pixels: new Uint8Array([255, 0, 0, 7, 0, 255, 0, 0]),
    };
    expect(lumaAt(img, 0, 0)).toBeCloseTo(0.2126 * 255, 6);
    expect(lumaAt(img, 1, 0)).toBeCloseTo(0.7152 * 255, 6);
  });
});

describe('strongestStepRow', () => {
  it('finds a step to a few hundredths of a pixel, wherever it falls in the pixel', () => {
    for (const edge of [100.0, 100.25, 100.5, 100.73, 233.1, 12.9]) {
      const img = horizonImage(8, 300, () => edge);
      const found = strongestStepRow(img, 3);
      expect(found).toBeDefined();
      expect(Math.abs((found ?? 0) - edge)).toBeLessThan(0.06);
    }
  });

  it('finds it whichever way the step goes (a sky darker than the ground, as in the classify view)', () => {
    const img = horizonImage(4, 200, () => 77.4, 30, 180);
    expect(Math.abs((strongestStepRow(img, 1) ?? 0) - 77.4)).toBeLessThan(0.06);
  });

  it('looks only inside the window it is given, so a nearer edge does not win', () => {
    const img = horizonImage(4, 200, () => 150.5);
    // An unrelated, stronger step higher up.
    for (let y = 0; y < 40; y++) {
      for (let c = 0; c < 4; c++) {
        const i = (y * 4 + c) * 4;
        img.pixels[i] = img.pixels[i + 1] = img.pixels[i + 2] = 0;
      }
    }
    expect(Math.abs((strongestStepRow(img, 1, 100, 199) ?? 0) - 150.5)).toBeLessThan(0.06);
    expect(strongestStepRow(img, 1, 0, 100)).toBeLessThan(45);
  });

  it('takes a window that is not whole rows (an expected position plus a margin)', () => {
    const img = horizonImage(4, 200, () => 120.4);
    const found = strongestStepRow(img, 2, 120.4 - 45, 120.4 + 45);
    expect(Math.abs((found ?? 0) - 120.4)).toBeLessThan(0.06);
  });

  it('says nothing about a flat column', () => {
    expect(
      strongestStepRow(
        horizonImage(2, 50, () => 0, 90, 90),
        0,
      ),
    ).toBeUndefined();
  });
});

describe('the pinhole', () => {
  const fov = 70 * DEG;
  it('maps the middle row to level, and the edges to half the field of view', () => {
    expect(rowElevationDeg(360, 720, fov)).toBeCloseTo(0, 9);
    expect(rowElevationDeg(0, 720, fov)).toBeCloseTo(35, 6);
    expect(rowElevationDeg(720, 720, fov)).toBeCloseTo(-35, 6);
  });
  it('is its own inverse', () => {
    for (const deg of [-30, -3.21, 0, 7.5, 34]) {
      expect(rowElevationDeg(elevationRow(deg, 720, fov), 720, fov)).toBeCloseTo(deg, 9);
    }
  });
  it('puts 3.21 degrees below the middle at about 29 rows on 720 at 70 degrees', () => {
    // f = 360 / tan 35 degrees = 514 pixels, and tan 3.21 degrees is 0.0561.
    expect(elevationRow(-3.21, 720, fov) - 360).toBeCloseTo(28.83, 1);
  });
});

describe('horizonDipDeg (GDD 7.4.2)', () => {
  const R = 6_371_000;
  it('is the table of the GDD, and 0 at the surface', () => {
    expect(horizonDipDeg(R, 0)).toBe(0);
    expect(horizonDipDeg(R, 10_000)).toBeCloseTo(3.21, 2);
    expect(horizonDipDeg(R, 100)).toBeCloseTo(0.32, 2);
    expect(horizonDipDeg(R, 1000)).toBeCloseTo(1.015, 2);
    expect(horizonDipDeg(R, 15_000)).toBeCloseTo(3.9, 1);
  });
});

describe('expectedHorizonRow', () => {
  const base = { width: 1280, height: 720, fovYRad: 70 * DEG, dipDeg: 3.21, rollDeg: 0 };

  it('is the dip below the middle at the middle column', () => {
    const row = expectedHorizonRow(base, 639.5 - 0.5) ?? 0;
    expect(rowElevationDeg(row, 720, 70 * DEG)).toBeCloseTo(-3.21, 3);
  });

  it('falls away from the middle: the horizon is a circle, so it bows towards the edges', () => {
    const middle = expectedHorizonRow(base, 640) ?? 0;
    const edge = expectedHorizonRow(base, 20) ?? 0;
    // At the side the ray is at an angle to the view axis, so the same dip is further down the image.
    expect(edge - middle).toBeGreaterThan(10);
    expect(expectedHorizonRow(base, 20)).toBeCloseTo(expectedHorizonRow(base, 1259) ?? 0, 3);
  });

  it('is straight for no dip, whatever the roll is not: a level camera sees the horizon through the middle', () => {
    const flat = { ...base, dipDeg: 0 };
    for (const x of [0, 300, 640, 1000, 1279]) {
      expect(expectedHorizonRow(flat, x)).toBeCloseTo(360, 6);
    }
  });

  it('tilts with the roll: with the right wing down the horizon rises to the right, as on an attitude indicator', () => {
    const rolled = { ...base, dipDeg: 0, rollDeg: 30 };
    const left = expectedHorizonRow(rolled, 100) ?? 0;
    const right = expectedHorizonRow(rolled, 1179) ?? 0; // 100 and 1179 are symmetric about the middle
    expect(right).toBeLessThan(left); // smaller rows are higher up
    // With no dip it is a straight line through the middle at the roll angle, as seen on screen.
    expect((left - right) / 1079).toBeCloseTo(Math.tan(30 * DEG), 3);
    expect((left + right) / 2).toBeCloseTo(360, 3);
  });

  it('widens with the field of view: at 100 degrees the same dip is nearer the middle', () => {
    const narrow = (expectedHorizonRow(base, 640) ?? 0) - 360;
    const wide = (expectedHorizonRow({ ...base, fovYRad: 100 * DEG }, 640) ?? 0) - 360;
    expect(wide).toBeLessThan(narrow);
  });

  it('reaches nowhere past the front hemisphere', () => {
    expect(expectedHorizonRow({ ...base, fovYRad: 175 * DEG }, 0)).toBeDefined();
  });
});

describe('fittedAngleDeg', () => {
  it('is the angle of a line, positive when it rises to the right', () => {
    const rising = Array.from(
      { length: 50 },
      (_, i) => [i * 10, 400 - Math.tan(30 * DEG) * i * 10] as const,
    );
    expect(fittedAngleDeg(rising)).toBeCloseTo(30, 6);
    const falling = rising.map(([x, y]) => [x, 800 - y] as const);
    expect(fittedAngleDeg(falling)).toBeCloseTo(-30, 6);
    expect(fittedAngleDeg([[0, 5]])).toBe(0);
  });
});

describe('rmsAndMax', () => {
  it('summarises errors', () => {
    expect(rmsAndMax([3, -4])).toEqual({ rms: Math.sqrt(12.5), max: 4 });
    expect(rmsAndMax([])).toEqual({ rms: 0, max: 0 });
  });
});
