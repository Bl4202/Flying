import { describe, expect, it } from 'vitest';
import { attitudeQuat, NORTH } from './snapshot-fixture.ts';

type V3 = [number, number, number];

/** Rotates `v` by the unit quaternion `[w, x, y, z]`. */
function rotate(q: [number, number, number, number], v: V3): V3 {
  const [w, x, y, z] = q;
  const [vx, vy, vz] = v;
  const tx = 2 * (y * vz - z * vy);
  const ty = 2 * (z * vx - x * vz);
  const tz = 2 * (x * vy - y * vx);
  return [
    vx + w * tx + (y * tz - z * ty),
    vy + w * ty + (z * tx - x * tz),
    vz + w * tz + (x * ty - y * tx),
  ];
}

const close = (a: V3, b: V3): void => {
  for (let i = 0; i < 3; i++) expect(a[i]).toBeCloseTo(b[i] as number, 6);
};

const FORWARD: V3 = [1, 0, 0];
const RIGHT: V3 = [0, 1, 0];
const DOWN: V3 = [0, 0, 1];

describe('attitudeQuat', () => {
  it('is the spawn attitude for no heading, pitch or roll: nose north, right wing east, belly down', () => {
    const q = attitudeQuat(0, 0, 0);
    close(rotate(q, FORWARD), [0, 1, 0]);
    close(rotate(q, RIGHT), [1, 0, 0]);
    close(rotate(q, DOWN), [0, 0, -1]);
    // The same rotation as NORTH, up to the sign of the quaternion.
    const sign = Math.sign(q[0] * NORTH[0] + q[1] * NORTH[1] + q[2] * NORTH[2] + q[3] * NORTH[3]);
    q.forEach((v, i) => {
      expect(sign * v).toBeCloseTo(NORTH[i] as number, 6);
    });
  });

  it('turns clockwise with the heading, seen from above', () => {
    close(rotate(attitudeQuat(90, 0, 0), FORWARD), [1, 0, 0]);
    close(rotate(attitudeQuat(180, 0, 0), FORWARD), [0, -1, 0]);
    close(rotate(attitudeQuat(270, 0, 0), FORWARD), [-1, 0, 0]);
    close(rotate(attitudeQuat(90, 0, 0), RIGHT), [0, -1, 0]);
  });

  it('raises the nose with a positive pitch and keeps the wings level', () => {
    const q = attitudeQuat(0, 30, 0);
    close(rotate(q, FORWARD), [0, Math.cos(Math.PI / 6), 0.5]);
    expect(rotate(q, RIGHT)[2]).toBeCloseTo(0, 9);
    close(rotate(attitudeQuat(0, 90, 0), FORWARD), [0, 0, 1]);
  });

  it('drops the right wing with a positive roll, and leaves the nose where it was', () => {
    const q = attitudeQuat(0, 0, 30);
    close(rotate(q, FORWARD), [0, 1, 0]);
    close(rotate(q, RIGHT), [Math.cos(Math.PI / 6), 0, -0.5]);
    close(rotate(attitudeQuat(0, 0, 90), RIGHT), [0, 0, -1]);
    close(rotate(attitudeQuat(0, 0, 90), DOWN), [-1, 0, 0]);
  });

  it('is a unit quaternion, and the axes stay at right angles, for any attitude', () => {
    for (const [h, p, r] of [
      [10, 20, 30],
      [200, -45, 120],
      [359, 89, -170],
    ] as const) {
      const q = attitudeQuat(h, p, r);
      expect(Math.hypot(...q)).toBeCloseTo(1, 6);
      const f = rotate(q, FORWARD);
      const rt = rotate(q, RIGHT);
      const d = rotate(q, DOWN);
      expect(f[0] * rt[0] + f[1] * rt[1] + f[2] * rt[2]).toBeCloseTo(0, 6);
      expect(f[0] * d[0] + f[1] * d[1] + f[2] * d[2]).toBeCloseTo(0, 6);
      // Forward cross right is down.
      close(
        [f[1] * rt[2] - f[2] * rt[1], f[2] * rt[0] - f[0] * rt[2], f[0] * rt[1] - f[1] * rt[0]],
        d,
      );
    }
  });
});
