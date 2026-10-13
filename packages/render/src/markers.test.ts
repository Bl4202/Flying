import { describe, expect, it } from 'vitest';
import {
  buildMarkerMesh,
  MARKER_TRIANGLES,
  MARKER_VERTEX_COUNT,
  MARKER_VERTEX_FLOATS,
} from './markers.ts';

/** Element `i` of `a`, or NaN (which fails any comparison) if there is none. */
const at = (a: ArrayLike<number>, i: number): number => a[i] ?? Number.NaN;

describe('marker mesh', () => {
  const mesh = buildMarkerMesh();

  it('is six flat-shaded triangles', () => {
    expect(MARKER_TRIANGLES).toHaveLength(6);
    expect(MARKER_VERTEX_COUNT).toBe(18);
    expect(mesh).toHaveLength(18 * MARKER_VERTEX_FLOATS);
  });

  it('points its nose along +x, one unit long', () => {
    const xs = Array.from({ length: MARKER_VERTEX_COUNT }, (_, i) => mesh[i * 6] ?? 0);
    expect(Math.max(...xs)).toBe(0.5);
    expect(Math.min(...xs)).toBe(-0.5);
  });

  it('has unit normals, and they point out of the solid', () => {
    // The mesh is convex, so the vector from its centre to a face points the way the normal should.
    let cx = 0,
      cy = 0,
      cz = 0;
    for (let i = 0; i < MARKER_VERTEX_COUNT; i++) {
      cx += (mesh[i * 6] ?? 0) / MARKER_VERTEX_COUNT;
      cy += (mesh[i * 6 + 1] ?? 0) / MARKER_VERTEX_COUNT;
      cz += (mesh[i * 6 + 2] ?? 0) / MARKER_VERTEX_COUNT;
    }
    for (let t = 0; t < 6; t++) {
      const v = (k: number): number[] =>
        Array.from({ length: 6 }, (_, c) => mesh[(t * 3 + k) * 6 + c] ?? 0);
      const [a, b, c] = [v(0), v(1), v(2)] as [number[], number[], number[]];
      const n = [a[3] ?? 0, a[4] ?? 0, a[5] ?? 0];
      expect(Math.hypot(...n)).toBeCloseTo(1, 6);
      // All three vertices of a face share its normal.
      for (const w of [b, c]) expect([w[3], w[4], w[5]]).toEqual(n);
      const face = [
        (at(a, 0) + at(b, 0) + at(c, 0)) / 3 - cx,
        (at(a, 1) + at(b, 1) + at(c, 1)) / 3 - cy,
        (at(a, 2) + at(b, 2) + at(c, 2)) / 3 - cz,
      ];
      const outward = at(face, 0) * at(n, 0) + at(face, 1) * at(n, 1) + at(face, 2) * at(n, 2);
      expect(outward, `face ${String(t)}`).toBeGreaterThan(0);
    }
  });
});
