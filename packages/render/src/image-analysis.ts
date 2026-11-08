/**
 * Measuring a rendered image (the screenshot tests, ADR-39 S1 to S5): where the horizon is to a
 * fraction of a pixel, what angle that is, what angle it should be, and the exact image of the
 * horizon circle for any roll and field of view. Pure functions over an RGBA image, so each is tested
 * in Node against images made on purpose, and the browser run then only has to produce the pixels.
 *
 * Rows are measured in continuous pixel units from the top edge of the image: the centre of the top
 * row is 0.5, so the middle of an image `H` rows high is `H / 2`.
 */

/** A frame as RGBA bytes, top row first. */
export interface RgbaImage {
  width: number;
  height: number;
  pixels: Uint8Array;
}

const DEG = Math.PI / 180;

/** Rec. 709 luma of one pixel, 0 to 255. */
export function lumaAt(img: RgbaImage, x: number, y: number): number {
  const i = (y * img.width + x) * 4;
  const p = img.pixels;
  return 0.2126 * (p[i] ?? 0) + 0.7152 * (p[i + 1] ?? 0) + 0.0722 * (p[i + 2] ?? 0);
}

/** The colour of one pixel as `[r, g, b]`. */
export function rgbAt(img: RgbaImage, x: number, y: number): [number, number, number] {
  const i = (y * img.width + x) * 4;
  const p = img.pixels;
  return [p[i] ?? 0, p[i + 1] ?? 0, p[i + 2] ?? 0];
}

/**
 * The row, in continuous pixel units, of the strongest brightness step down column `x` between rows
 * `from` and `to` (default the whole column), found as the peak of the vertical difference and refined
 * by the centroid of the step and its neighbours. A step between pixel rows `k` and `k + 1` is at `k + 1`.
 * Undefined if the column is flat there.
 */
export function strongestStepRow(
  img: RgbaImage,
  x: number,
  from = 0,
  to = img.height - 1,
): number | undefined {
  // A window given in continuous rows (an expected position plus or minus a margin) is widened to whole rows.
  const lo = Math.max(Math.floor(from), 0);
  const hi = Math.min(Math.ceil(to), img.height - 1);
  if (hi - lo < 2) return undefined;
  const step = (k: number): number => Math.abs(lumaAt(img, x, k + 1) - lumaAt(img, x, k));
  let best = lo;
  let bestValue = -1;
  for (let k = lo; k < hi; k++) {
    const v = step(k);
    if (v > bestValue) {
      bestValue = v;
      best = k;
    }
  }
  if (bestValue < 1) return undefined;
  // The centroid of the step over its neighbours: exact when the edge is a pixel's coverage, as it is
  // for an antialiased or blurred edge (a parabola through the peak is biased by about 0.1 pixel).
  const before = best > lo ? step(best - 1) : 0;
  const after = best < hi - 1 ? step(best + 1) : 0;
  const total = before + bestValue + after;
  return (best * before + (best + 1) * bestValue + (best + 2) * after) / total;
}

/**
 * The elevation, degrees (up positive), of continuous row `p` for a camera looking level with
 * vertical field of view `fovYRad` (a pinhole: the row's tangent is linear in `p`).
 */
export function rowElevationDeg(p: number, height: number, fovYRad: number): number {
  const ndc = (height / 2 - p) / (height / 2);
  return Math.atan(ndc * Math.tan(fovYRad / 2)) / DEG;
}

/** The inverse: the continuous row of elevation `elevationDeg`. */
export function elevationRow(elevationDeg: number, height: number, fovYRad: number): number {
  return height / 2 - (Math.tan(elevationDeg * DEG) / Math.tan(fovYRad / 2)) * (height / 2);
}

/**
 * How far below the level the horizon is for an eye `heightM` above a sphere of radius `radiusM`
 * (GDD 7.4.2): `acos(R / (R + h))`, degrees. 3.21 degrees at 10 km.
 */
export function horizonDipDeg(radiusM: number, heightM: number): number {
  return Math.acos(radiusM / (radiusM + heightM)) / DEG;
}

export interface HorizonCamera {
  width: number;
  height: number;
  /** Vertical field of view, radians. */
  fovYRad: number;
  /** How far below the level the horizon is, degrees. */
  dipDeg: number;
  /** Roll, degrees: positive is the right wing down. Pitch is level and the camera looks north. */
  rollDeg: number;
}

/**
 * The image of the horizon circle: for each column the continuous row where the cone of directions
 * `dipDeg` below the level crosses the column's centre, for a camera looking along the level with a
 * roll. (A directions at azimuth `phi` is `(cos dip sin phi, cos dip cos phi, -sin dip)` in east,
 * north, up; the camera looks north with its right axis tilted down by the roll.) Undefined where
 * the curve does not reach the column.
 */
export function expectedHorizonRow(cam: HorizonCamera, x: number): number | undefined {
  const f = cam.height / 2 / Math.tan(cam.fovYRad / 2);
  const dip = cam.dipDeg * DEG;
  const roll = cam.rollDeg * DEG;
  const cr = Math.cos(roll);
  const sr = Math.sin(roll);
  const target = x + 0.5 - cam.width / 2;
  const at = (phi: number): [number, number] | undefined => {
    const e = Math.cos(dip) * Math.sin(phi);
    const n = Math.cos(dip) * Math.cos(phi);
    const u = -Math.sin(dip);
    // The camera's axes in east, north, up: right = (cos r, 0, -sin r); up = (sin r, 0, cos r).
    const xc = e * cr - u * sr;
    const yc = e * sr + u * cr;
    if (n <= 1e-6) return undefined; // behind the camera
    return [(xc / n) * f, (yc / n) * f];
  };
  // The curve is monotonic in x over the front hemisphere: bisect on the azimuth.
  let lo = -89.9 * DEG;
  let hi = 89.9 * DEG;
  const pl = at(lo);
  const ph = at(hi);
  if (pl === undefined || ph === undefined) return undefined;
  const xs = (phi: number): number => at(phi)?.[0] ?? Number.NaN;
  const increasing = xs(hi) > xs(lo);
  if (target < Math.min(xs(lo), xs(hi)) || target > Math.max(xs(lo), xs(hi))) return undefined;
  for (let i = 0; i < 60; i++) {
    const mid = (lo + hi) / 2;
    if (xs(mid) < target === increasing) lo = mid;
    else hi = mid;
  }
  const point = at((lo + hi) / 2);
  return point === undefined ? undefined : cam.height / 2 - point[1];
}

/** The angle, degrees, a polyline of `(column, row)` points makes with the horizontal, by least squares. */
export function fittedAngleDeg(points: readonly (readonly [number, number])[]): number {
  const n = points.length;
  if (n < 2) return 0;
  let sx = 0;
  let sy = 0;
  for (const [x, y] of points) {
    sx += x;
    sy += y;
  }
  const mx = sx / n;
  const my = sy / n;
  let sxx = 0;
  let sxy = 0;
  for (const [x, y] of points) {
    sxx += (x - mx) ** 2;
    sxy += (x - mx) * (y - my);
  }
  // Rows grow downwards, so a horizon rising to the right has a negative slope: that is a positive angle.
  return -Math.atan2(sxy, sxx) / DEG;
}

/** The root-mean-square and the largest of `values`' magnitudes. */
export function rmsAndMax(values: readonly number[]): { rms: number; max: number } {
  if (values.length === 0) return { rms: 0, max: 0 };
  let sum = 0;
  let max = 0;
  for (const v of values) {
    sum += v * v;
    max = Math.max(max, Math.abs(v));
  }
  return { rms: Math.sqrt(sum / values.length), max };
}
