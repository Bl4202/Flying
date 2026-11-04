/**
 * Camera maths. Everything here is plain numbers (no GPU), so it is tested in Node.
 *
 * The camera sits at the origin of "eye space": the CPU subtracts the eye position from every
 * world position in f64, in BF, and only then converts to render axes and rounds to f32 (GDD
 * 44.8, relative-to-eye). A body 400 km from the BF origin that is 100 m from the eye is then
 * stored to within microns; subtracting after rounding to f32 would leave centimetres of error and
 * visible shimmer. The view matrix is therefore rotation only.
 *
 * Matrices are column-major, as both WebGPU and WebGL2 expect.
 */
import type { Quat, Vec3 } from '@flying/sim-bridge/frame-math';

/** Anything that holds numbers and can be written to. */
export type NumberArray = Float32Array | Float64Array | number[];

/**
 * Nearest visible distance, metres. Reverse-Z with an infinite far plane has no far limit, and its
 * float depth keeps its precision however close the near plane is. It is half a metre because the
 * eye now sits a couple of metres above a runway: a near plane of five metres would clip the
 * ground under the nose.
 */
export const NEAR_M = 0.5;
/** Vertical field of view, radians (70 degrees). */
export const FOV_Y = (70 * Math.PI) / 180;

/**
 * Clip-space depth convention. WebGPU (and WebGL2 with `EXT_clip_control`) use 0..1; plain WebGL2
 * uses -1..1. Both are reverse-Z: the near plane maps to the top of the range and infinity to the
 * bottom, which spreads float depth precision evenly over distance.
 */
export type ZRange = '0to1' | '-1to1';

/** A 4x4 projection, reverse-Z, infinite far plane. Writes 16 numbers to `out`. */
export function projection(
  out: NumberArray,
  fovY: number,
  aspect: number,
  near: number,
  zRange: ZRange,
): void {
  const f = 1 / Math.tan(fovY / 2);
  const z2 = zRange === '0to1' ? 0 : 1;
  const z3 = zRange === '0to1' ? near : 2 * near;
  // z_clip = z2 * z_view + z3, w_clip = -z_view.
  out[0] = f / aspect;
  out[1] = 0;
  out[2] = 0;
  out[3] = 0;
  out[4] = 0;
  out[5] = f;
  out[6] = 0;
  out[7] = 0;
  out[8] = 0;
  out[9] = 0;
  out[10] = z2;
  out[11] = -1;
  out[12] = 0;
  out[13] = 0;
  out[14] = z3;
  out[15] = 0;
}

/**
 * Rotation-only view matrix for an observer whose body-to-BF attitude is `qBf` (`[w, x, y, z]`).
 * The body frame is forward-right-down; the camera looks along its forward axis with its up
 * along the body's up. Writes 16 numbers to `out`.
 */
export function viewRotation(out: NumberArray, qBf: Quat): void {
  // Body axes in render space: render <- BF <- body, written out in scalars (this runs every frame
  // and makes no arrays). q = BF_TO_RENDER * qBf, with BF_TO_RENDER = (s, -s, 0, 0), s = sqrt(1/2).
  const s = Math.SQRT1_2;
  const w = s * (qBf[0] + qBf[1]);
  const x = s * (qBf[1] - qBf[0]);
  const y = s * (qBf[2] + qBf[3]);
  const z = s * (qBf[3] - qBf[2]);
  // The columns of the rotation matrix of q: where body forward, right and down go.
  const fx = 1 - 2 * (y * y + z * z);
  const fy = 2 * (x * y + w * z);
  const fz = 2 * (x * z - w * y);
  const rx = 2 * (x * y - w * z);
  const ry = 1 - 2 * (x * x + z * z);
  const rz = 2 * (y * z + w * x);
  const dx = 2 * (x * z + w * y);
  const dy = 2 * (y * z - w * x);
  const dz = 1 - 2 * (x * x + y * y);
  // Camera x = right, y = up (= -down), z = back (= -forward): the rows of the rotation.
  out[0] = rx;
  out[1] = -dx;
  out[2] = -fx;
  out[3] = 0;
  out[4] = ry;
  out[5] = -dy;
  out[6] = -fy;
  out[7] = 0;
  out[8] = rz;
  out[9] = -dz;
  out[10] = -fz;
  out[11] = 0;
  out[12] = 0;
  out[13] = 0;
  out[14] = 0;
  out[15] = 1;
}

/** `a * b` for column-major 4x4 matrices. */
export function mul4(out: NumberArray, a: ArrayLike<number>, b: ArrayLike<number>): void {
  for (let col = 0; col < 4; col++) {
    for (let row = 0; row < 4; row++) {
      let sum = 0;
      for (let k = 0; k < 4; k++) {
        sum += (a[k * 4 + row] ?? 0) * (b[col * 4 + k] ?? 0);
      }
      out[col * 4 + row] = sum;
    }
  }
}

/**
 * A BF position relative to the eye, in render axes, as `f32`. The subtraction is done in `f64`
 * before anything is rounded; that is the whole point (see the module comment).
 * Writes 3 numbers to `out` at `at`.
 */
export function relativeToEye(
  out: NumberArray,
  at: number,
  posBf: ArrayLike<number>,
  posAt: number,
  eyeBf: ArrayLike<number>,
): void {
  const dx = (posBf[posAt] ?? 0) - (eyeBf[0] ?? 0);
  const dy = (posBf[posAt + 1] ?? 0) - (eyeBf[1] ?? 0);
  const dz = (posBf[posAt + 2] ?? 0) - (eyeBf[2] ?? 0);
  // BF east, north, up -> render x, y, z = east, up, south.
  out[at] = dx;
  out[at + 1] = dz;
  out[at + 2] = -dy;
}

/** The known direction of the sun, as a unit vector towards it in render axes. */
export function sunDirection(): Vec3 {
  const v: Vec3 = [0.4, 0.8, 0.45];
  const n = Math.hypot(...v);
  return [v[0] / n, v[1] / n, v[2] / n];
}
