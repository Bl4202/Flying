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
import {
  BF_TO_RENDER,
  quatMul,
  quatRotate,
  type Quat,
  type Vec3,
} from '@flying/sim-bridge/frame-math';

/** Anything that holds numbers and can be written to. */
export type NumberArray = Float32Array | Float64Array | number[];

/** Nearest visible distance, metres. Reverse-Z with an infinite far plane has no far limit. */
export const NEAR_M = 5;
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
  // Body axes in render space: render <- BF <- body.
  const q = quatMul(BF_TO_RENDER, qBf);
  const forward = quatRotate(q, [1, 0, 0]);
  const right = quatRotate(q, [0, 1, 0]);
  const down = quatRotate(q, [0, 0, 1]);
  // Camera x = right, y = up (= -down), z = back (= -forward): the rows of the rotation.
  out[0] = right[0];
  out[1] = -down[0];
  out[2] = -forward[0];
  out[3] = 0;
  out[4] = right[1];
  out[5] = -down[1];
  out[6] = -forward[1];
  out[7] = 0;
  out[8] = right[2];
  out[9] = -down[2];
  out[10] = -forward[2];
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
