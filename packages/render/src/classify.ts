/**
 * The classify view (ADR-40): the scene drawn in flat key colours, with no lighting, haze or
 * anti-aliasing of the shading, so a test can tell from a pixel what drew it. The shaders hold the
 * same values as constants (`common.wgsl`); a test compares them.
 *
 * Bytes, as read back from an 8-bit target (a draw may land one off, so match with a tolerance).
 */
export const CLASSIFY_KEYS = {
  sky: [0, 0, 255],
  globe: [255, 0, 255],
  /** Terrain levels L0 to L3. */
  terrain: [
    [0, 255, 0],
    [0, 191, 0],
    [0, 127, 0],
    [0, 63, 0],
  ],
  marker: [255, 255, 0],
  model: [255, 128, 0],
  cockpit: [128, 0, 255],
} as const;

export type Rgb = readonly [number, number, number];

/** True if the pixel at `px[at..at+3]` is `key`, give or take `tolerance` in each channel. */
export function isKey(px: Uint8Array, at: number, key: Rgb, tolerance = 2): boolean {
  return (
    Math.abs((px[at] ?? 0) - key[0]) <= tolerance &&
    Math.abs((px[at + 1] ?? 0) - key[1]) <= tolerance &&
    Math.abs((px[at + 2] ?? 0) - key[2]) <= tolerance
  );
}

/** How many pixels of an RGBA readback are `key`. */
export function countKey(px: Uint8Array, key: Rgb, tolerance = 2): number {
  let n = 0;
  for (let at = 0; at < px.length; at += 4) if (isKey(px, at, key, tolerance)) n += 1;
  return n;
}
