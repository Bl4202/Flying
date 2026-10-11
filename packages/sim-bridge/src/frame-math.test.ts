import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import {
  BF_TO_RENDER,
  fromRenderVec,
  quatRotate,
  toRenderQuat,
  toRenderVec,
  type Quat,
  type Vec3,
} from './frame-math.ts';

interface Vectors {
  tolerance: number;
  vectors: { bf: Vec3; render: Vec3 }[];
  quaternions: { q_bf: Quat; q_render: Quat }[];
  rotations: { q_bf: Quat; v_bf: Vec3; rotated_render: Vec3 }[];
}

const data = JSON.parse(
  readFileSync(new URL('../../../tests/vectors/frame_math.json', import.meta.url), 'utf8'),
) as Vectors;

function near(actual: readonly number[], expected: readonly number[], tol: number): void {
  expect(actual).toHaveLength(expected.length);
  actual.forEach((a, i) => {
    expect(Math.abs(a - (expected[i] ?? NaN))).toBeLessThanOrEqual(tol);
  });
}

describe('frame math against the Rust reference vectors', () => {
  it('has vectors to check', () => {
    expect(data.vectors.length).toBeGreaterThan(10);
    expect(data.quaternions.length).toBeGreaterThan(5);
    expect(data.rotations.length).toBeGreaterThan(5);
  });

  it('maps vectors the same way', () => {
    for (const { bf, render } of data.vectors) {
      near(toRenderVec(bf), render, data.tolerance);
    }
  });

  it('maps quaternions the same way (R q R^-1)', () => {
    for (const { q_bf, q_render } of data.quaternions) {
      const got = toRenderQuat(q_bf);
      // q and -q are the same rotation; compare after fixing the sign on the larger component.
      const sign = Math.sign(
        got[0] * q_render[0] + got[1] * q_render[1] + got[2] * q_render[2] + got[3] * q_render[3],
      );
      near(
        got.map((c) => c * sign),
        q_render,
        data.tolerance,
      );
    }
  });

  it('rotates a BF vector to the same render vector', () => {
    for (const { q_bf, v_bf, rotated_render } of data.rotations) {
      near(quatRotate(toRenderQuat(q_bf), toRenderVec(v_bf)), rotated_render, data.tolerance);
    }
  });
});

describe('frame math identities', () => {
  it('maps the cardinal directions', () => {
    expect(toRenderVec([1, 0, 0])).toEqual([1, 0, -0]);
    expect(toRenderVec([0, 0, 1])).toEqual([0, 1, -0]);
    expect(toRenderVec([0, 1, 0])).toEqual([0, 0, -1]);
  });

  it('round-trips exactly', () => {
    const v: Vec3 = [123_456.789, -98.5, 7.25];
    expect(fromRenderVec(toRenderVec(v))).toEqual(v);
  });

  it('BF_TO_RENDER is the vector map as a rotation', () => {
    near(quatRotate(BF_TO_RENDER, [0.3, -1.7, 2.9]), toRenderVec([0.3, -1.7, 2.9]), 1e-14);
  });
});
