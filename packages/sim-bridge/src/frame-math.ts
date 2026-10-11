/**
 * Battlefield-frame to render-frame conversion (GDD 0.3.2, ADR-18). Mirrors
 * `crates/sim-core/src/frame_math.rs` and is tested against the vectors that file produces
 * (`tests/vectors/frame_math.json`, written by `cargo xtask gen-abi`).
 *
 * BF is East (+x), North (+y), Up (+z); render is right-handed, Y-up: `r = (bf.x, bf.z, -bf.y)`.
 * Quaternions are `[w, x, y, z]`.
 */

export type Vec3 = readonly [number, number, number];
export type Quat = readonly [number, number, number, number];

/** The BF -> render rotation: -90 degrees about +X. */
export const BF_TO_RENDER: Quat = [Math.SQRT1_2, -Math.SQRT1_2, 0, 0];

/** A BF vector (direction or camera-relative offset) in render axes. */
export function toRenderVec(bf: Vec3): [number, number, number] {
  return [bf[0], bf[2], -bf[1]];
}

/** A render-axes vector back in BF. */
export function fromRenderVec(r: Vec3): [number, number, number] {
  return [r[0], -r[2], r[1]];
}

/** Hamilton product: `a` applied after `b`. */
export function quatMul(a: Quat, b: Quat): [number, number, number, number] {
  return [
    a[0] * b[0] - a[1] * b[1] - a[2] * b[2] - a[3] * b[3],
    a[0] * b[1] + a[1] * b[0] + a[2] * b[3] - a[3] * b[2],
    a[0] * b[2] - a[1] * b[3] + a[2] * b[0] + a[3] * b[1],
    a[0] * b[3] + a[1] * b[2] - a[2] * b[1] + a[3] * b[0],
  ];
}

/** Conjugate; the inverse of a unit quaternion. */
export function quatConjugate(q: Quat): [number, number, number, number] {
  return [q[0], -q[1], -q[2], -q[3]];
}

/** Rotates `v` by the unit quaternion `q`. */
export function quatRotate(q: Quat, v: Vec3): [number, number, number] {
  const [, ux, uy, uz] = q;
  const w = q[0];
  // t = 2 (u x v)
  const tx = 2 * (uy * v[2] - uz * v[1]);
  const ty = 2 * (uz * v[0] - ux * v[2]);
  const tz = 2 * (ux * v[1] - uy * v[0]);
  return [
    v[0] + w * tx + (uy * tz - uz * ty),
    v[1] + w * ty + (uz * tx - ux * tz),
    v[2] + w * tz + (ux * ty - uy * tx),
  ];
}

/** A BF rotation expressed in render axes: `R q R^-1`. */
export function toRenderQuat(qBf: Quat): [number, number, number, number] {
  return quatMul(quatMul(BF_TO_RENDER, qBf), quatConjugate(BF_TO_RENDER));
}
