/** A synthetic terrain patch in the sim's byte layout, for tests and renderer self-tests. */
import { TERRAIN } from '@flying/sim-bridge/layout';

/**
 * Rolling hills centred on `(centreEast, centreNorth)`: height `amplitude * (sin(x/w) + cos(y/w))`
 * plus a flat offset, with analytic normals. Every field is a pure function of position.
 */
export function makeTestPatch(
  opts: {
    centreEast?: number;
    centreNorth?: number;
    spacingM?: number;
    amplitudeM?: number;
    wavelengthM?: number;
    baseM?: number;
  } = {},
): Uint8Array<ArrayBuffer> {
  const {
    centreEast = 0,
    centreNorth = 0,
    spacingM = 250,
    amplitudeM = 150,
    wavelengthM = 2500,
    baseM = 300,
  } = opts;
  const out = new Uint8Array(TERRAIN.SIZE);
  const dv = new DataView(out.buffer);
  dv.setFloat64(TERRAIN.H_ORIGIN_BF, centreEast, true);
  dv.setFloat64(TERRAIN.H_ORIGIN_BF + 8, centreNorth, true);
  dv.setFloat64(TERRAIN.H_ORIGIN_BF + 16, 0, true);
  dv.setUint32(TERRAIN.H_POSTS, TERRAIN.POSTS, true);
  dv.setFloat32(TERRAIN.H_SPACING_M, spacingM, true);
  const half = (TERRAIN.POSTS - 1) / 2;
  for (let j = 0; j < TERRAIN.POSTS; j++) {
    for (let i = 0; i < TERRAIN.POSTS; i++) {
      const x = (i - half) * spacingM;
      const y = (j - half) * spacingM;
      const z = baseM + amplitudeM * (Math.sin(x / wavelengthM) + Math.cos(y / wavelengthM));
      const dzdx = (amplitudeM / wavelengthM) * Math.cos(x / wavelengthM);
      const dzdy = (-amplitudeM / wavelengthM) * Math.sin(y / wavelengthM);
      const n = Math.hypot(dzdx, dzdy, 1);
      const at = TERRAIN.HEADER_SIZE + (j * TERRAIN.POSTS + i) * TERRAIN.VERTEX_BYTES;
      const values: [number, number][] = [
        [TERRAIN.V_POS, x],
        [TERRAIN.V_POS + 4, y],
        [TERRAIN.V_POS + 8, z],
        [TERRAIN.V_NORMAL, -dzdx / n],
        [TERRAIN.V_NORMAL + 4, -dzdy / n],
        [TERRAIN.V_NORMAL + 8, 1 / n],
        [TERRAIN.V_HEIGHT, z],
      ];
      for (const [o, v] of values) dv.setFloat32(at + o, v, true);
    }
  }
  return out;
}
