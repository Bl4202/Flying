/**
 * Keeps the terrain patch under the aircraft. The renderer draws one 32 km patch (129 x 129
 * posts, 250 m apart); once the ownship is more than `TERRAIN_RECENTRE_M` from its centre, the
 * Sim Worker generates a new patch around it, snapped to a 1 km grid so that small wanderings do
 * not refill it, and sends it to the renderer.
 */
import { TERRAIN_RECENTRE_M, TERRAIN_SNAP_M } from './constants.ts';

/** The new patch centre if the ownship at `pos` (BF east, north) has left the old one's middle. */
export function nextPatchCentre(
  centre: readonly [number, number],
  pos: readonly [number, number],
): [number, number] | undefined {
  const [x, y] = pos;
  if (!Number.isFinite(x) || !Number.isFinite(y)) return undefined;
  if (Math.hypot(x - centre[0], y - centre[1]) <= TERRAIN_RECENTRE_M) return undefined;
  const snap = (v: number): number => Math.round(v / TERRAIN_SNAP_M) * TERRAIN_SNAP_M;
  return [snap(x), snap(y)];
}
