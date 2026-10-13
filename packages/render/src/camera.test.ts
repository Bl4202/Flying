import { describe, expect, it } from 'vitest';
import type { Quat } from '@flying/sim-bridge/frame-math';
import {
  FOV_Y,
  mul4,
  NEAR_M,
  projection,
  relativeToEye,
  sunDirection,
  viewRotation,
} from './camera.ts';

/** Element `i` of `a`, or NaN (which fails any comparison) if there is none. */
const at = (a: ArrayLike<number>, i: number): number => a[i] ?? Number.NaN;

/** Column-major 4x4 times a point (w = 1). */
function transform(
  m: ArrayLike<number>,
  p: readonly [number, number, number],
): [number, number, number, number] {
  const out: [number, number, number, number] = [0, 0, 0, 0];
  for (let row = 0; row < 4; row++) {
    out[row] =
      (m[row] ?? 0) * p[0] +
      (m[4 + row] ?? 0) * p[1] +
      (m[8 + row] ?? 0) * p[2] +
      (m[12 + row] ?? 0);
  }
  return out;
}

const HEADING_NORTH: Quat = [0, Math.SQRT1_2, Math.SQRT1_2, 0];

describe('projection', () => {
  const ndcZ = (zRange: '0to1' | '-1to1', zView: number): number => {
    const p = new Float64Array(16);
    projection(p, FOV_Y, 16 / 9, NEAR_M, zRange);
    const c = transform(p, [0, 0, zView]);
    return c[2] / c[3];
  };

  it('maps the near plane to the top of the depth range and infinity to the bottom (reverse-Z)', () => {
    expect(ndcZ('0to1', -NEAR_M)).toBeCloseTo(1, 12);
    expect(ndcZ('0to1', -1e9)).toBeCloseTo(0, 6);
    expect(ndcZ('-1to1', -NEAR_M)).toBeCloseTo(1, 12);
    expect(ndcZ('-1to1', -1e9)).toBeCloseTo(-1, 6);
  });

  it('falls monotonically with distance, so nearer is always greater', () => {
    for (const z of ['0to1', '-1to1'] as const) {
      let last = Infinity;
      for (const d of [5, 10, 100, 1000, 20_000, 400_000, 1e7]) {
        const v = ndcZ(z, -d);
        expect(v).toBeLessThan(last);
        last = v;
      }
    }
  });

  it('is a perspective with the requested field of view and aspect', () => {
    const p = new Float64Array(16);
    projection(p, FOV_Y, 2, NEAR_M, '0to1');
    // A point on the top edge of the frustum at depth 100 lands at y = 1; on the right edge, x = 1.
    const top = transform(p, [0, 100 * Math.tan(FOV_Y / 2), -100]);
    expect(top[1] / top[3]).toBeCloseTo(1, 12);
    const right = transform(p, [100 * Math.tan(FOV_Y / 2) * 2, 0, -100]);
    expect(right[0] / right[3]).toBeCloseTo(1, 12);
    expect(p[11]).toBe(-1); // w = -z
  });
});

describe('view rotation', () => {
  const view = (q: Quat): Float64Array => {
    const v = new Float64Array(16);
    viewRotation(v, q);
    return v;
  };

  it('looking north from a level attitude: north is ahead, east is right, up is up', () => {
    const v = view(HEADING_NORTH);
    // Render axes: x east, y up, z south.
    const north = transform(v, [0, 0, -1000]);
    expect(north[0]).toBeCloseTo(0, 9);
    expect(north[1]).toBeCloseTo(0, 9);
    expect(north[2]).toBeCloseTo(-1000, 9); // in front of the camera
    const east = transform(v, [1000, 0, 0]);
    expect([east[0], east[1], east[2]].map((x) => Math.round(x) + 0)).toEqual([1000, 0, 0]); // to the right
    const up = transform(v, [0, 1000, 0]);
    expect([up[0], up[1], up[2]].map((x) => Math.round(x) + 0)).toEqual([0, 1000, 0]);
  });

  it('turning to face east puts east ahead', () => {
    // Heading north, then yaw 90 degrees to the right about body down: a quarter turn about BF -z.
    const s = Math.SQRT1_2;
    const yawRight: Quat = [s, 0, 0, -s]; // rotation by +90 degrees about BF down
    const faceEast = ((): Quat => {
      const [w1, x1, y1, z1] = yawRight;
      const [w2, x2, y2, z2] = HEADING_NORTH;
      return [
        w1 * w2 - x1 * x2 - y1 * y2 - z1 * z2,
        w1 * x2 + x1 * w2 + y1 * z2 - z1 * y2,
        w1 * y2 - x1 * z2 + y1 * w2 + z1 * x2,
        w1 * z2 + x1 * y2 - y1 * x2 + z1 * w2,
      ];
    })();
    const v = view(faceEast);
    const east = transform(v, [1000, 0, 0]);
    expect([east[0], east[1], east[2]].map((x) => Math.round(x) + 0)).toEqual([0, 0, -1000]);
  });

  it('is a proper rotation for any attitude', () => {
    let seed = 12345;
    const rnd = (): number => {
      seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0;
      return seed / 2 ** 32 - 0.5;
    };
    for (let i = 0; i < 50; i++) {
      const q: [number, number, number, number] = [rnd(), rnd(), rnd(), rnd()];
      const n = Math.hypot(...q);
      const v = view([q[0] / n, q[1] / n, q[2] / n, q[3] / n]);
      const col = (c: number): number[] => [v[c * 4] ?? 0, v[c * 4 + 1] ?? 0, v[c * 4 + 2] ?? 0];
      const dot = (a: number[], b: number[]): number =>
        a.reduce((s, x, k) => s + x * (b[k] ?? 0), 0);
      for (let a = 0; a < 3; a++) {
        expect(dot(col(a), col(a))).toBeCloseTo(1, 12);
        for (let b = a + 1; b < 3; b++) expect(dot(col(a), col(b))).toBeCloseTo(0, 12);
      }
      const [c0, c1, c2] = [col(0), col(1), col(2)] as [number[], number[], number[]];
      const det =
        at(c0, 0) * (at(c1, 1) * at(c2, 2) - at(c1, 2) * at(c2, 1)) -
        at(c1, 0) * (at(c0, 1) * at(c2, 2) - at(c0, 2) * at(c2, 1)) +
        at(c2, 0) * (at(c0, 1) * at(c1, 2) - at(c0, 2) * at(c1, 1));
      expect(det).toBeCloseTo(1, 12);
    }
  });

  it('has no translation (the eye is the origin)', () => {
    const v = view(HEADING_NORTH);
    expect([v[12], v[13], v[14], v[15]]).toEqual([0, 0, 0, 1]);
  });

  it('combines with a projection so something straight ahead is at the centre of the screen', () => {
    const p = new Float64Array(16);
    projection(p, FOV_Y, 1.6, NEAR_M, '0to1');
    const vp = new Float64Array(16);
    mul4(vp, p, view(HEADING_NORTH));
    const ahead = transform(vp, [0, 0, -5000]); // 5 km north in render axes
    expect(ahead[0] / ahead[3]).toBeCloseTo(0, 12);
    expect(ahead[1] / ahead[3]).toBeCloseTo(0, 12);
    expect(ahead[2] / ahead[3]).toBeGreaterThan(0);
    expect(ahead[3]).toBeGreaterThan(0);
    const behind = transform(vp, [0, 0, 5000]);
    expect(behind[3]).toBeLessThan(0); // behind the camera: clipped
  });
});

describe('mul4', () => {
  it('multiplies column-major matrices (identity, associativity with a translation)', () => {
    const id = [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1];
    const t = [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 5, 6, 7, 1];
    const out = new Float64Array(16);
    mul4(out, id, t);
    expect([...out]).toEqual(t);
    mul4(out, t, id);
    expect([...out]).toEqual(t);
    mul4(out, t, t);
    expect([out[12], out[13], out[14]]).toEqual([10, 12, 14]);
  });
});

describe('relative-to-eye (GDD 44.8)', () => {
  it('maps BF east, north, up to render x, y, z = east, up, south', () => {
    const out = new Float64Array(3);
    relativeToEye(out, 0, [10, 20, 30], 0, [1, 2, 3]);
    expect([...out]).toEqual([9, 27, -18]);
  });

  it('keeps sub-millimetre accuracy 400 km from the origin where naive f32 loses centimetres', () => {
    const eye = [400_000.013, -399_999.987, 5_000.0];
    const pos = [at(eye, 0) + 123.4567, at(eye, 1) - 80.7654, at(eye, 2) + 9.3217];
    const exact = [123.4567, 9.3217, 80.7654]; // render axes: east, up, south

    const out = new Float32Array(3);
    relativeToEye(out, 0, pos, 0, eye);
    const err = exact.map((e, k) => Math.abs((out[k] ?? 0) - e));
    expect(Math.max(...err)).toBeLessThan(1e-4); // under 0.1 mm

    // The naive way: round both to f32 first, then subtract.
    const naive = [
      Math.fround(at(pos, 0)) - Math.fround(at(eye, 0)),
      Math.fround(at(pos, 2)) - Math.fround(at(eye, 2)),
      -(Math.fround(at(pos, 1)) - Math.fround(at(eye, 1))),
    ];
    const naiveErr = exact.map((e, k) => Math.abs((naive[k] ?? 0) - e));
    expect(Math.max(...naiveErr)).toBeGreaterThan(0.01); // over 1 cm
  });

  it('writes at the given offset in a larger array', () => {
    const out = new Float32Array(8).fill(-1);
    relativeToEye(out, 4, [1, 1, 1], 0, [0, 0, 0]);
    expect([...out]).toEqual([-1, -1, -1, -1, 1, 1, -1, -1]);
  });
});

describe('sun', () => {
  it('is a unit vector pointing upward', () => {
    const s = sunDirection();
    expect(Math.hypot(...s)).toBeCloseTo(1, 12);
    expect(s[1]).toBeGreaterThan(0);
  });
});
