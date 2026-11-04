/**
 * The four nested terrain levels (ADR-40) and the arithmetic that keeps them consistent.
 *
 * Every level is a `TERRAIN.POSTS` x `TERRAIN.POSTS` patch from the sim's own terrain function
 * (`fg_terrain_fill`), each four times coarser than the one inside it:
 *
 * | level | spacing | footprint | re-centred after the aircraft is | centre snaps to |
 * |-------|---------|-----------|----------------------------------|-----------------|
 * | L0    | 250 m   | 32 km     | 8 km from the centre             | 1 km            |
 * | L1    | 1 km    | 128 km    | 32 km                            | 4 km            |
 * | L2    | 4 km    | 512 km    | 128 km                           | 16 km           |
 * | L3    | 16 km   | 2 048 km  | 512 km                           | 64 km           |
 *
 * A centre snaps to the *next coarser spacing*, so the edges of a finer footprint lie on the coarser
 * grid and the coarser level can leave a hole of whole cells exactly where the finer one is. The
 * re-centre distances are chosen so that a finer footprint stays inside the next coarser one even
 * when the fills are staggered (one level per check): the finer level reaches at most
 * `recentre + snap / 2 + halfSpan` from the aircraft, the coarser at least
 * `halfSpan - recentre - snap / 2`.
 *
 * L3's corners are up to 1 448 km from the anchor, beyond the 1 000 km frame limit of GDD 7.4.4;
 * the levels are drawn only, never simulated on, so that limit does not bite (a clarification).
 */
import { TERRAIN } from './layout.gen.ts';

export const TERRAIN_LEVEL_COUNT = 4;

/** Posts per edge, and the intervals between them. */
const INTERVALS = TERRAIN.POSTS - 1;

export interface TerrainLevelSpec {
  readonly level: number;
  /** Distance between posts, metres. */
  readonly spacingM: number;
  /** Half the footprint's width, metres. */
  readonly halfSpanM: number;
  /** The level is regenerated once the aircraft is more than this far from its centre, metres. */
  readonly recentreM: number;
  /** The centre snaps to this grid, metres (the next coarser level's spacing). */
  readonly snapM: number;
}

function spec(level: number, spacingM: number, recentreM: number, snapM: number): TerrainLevelSpec {
  return { level, spacingM, halfSpanM: (spacingM * INTERVALS) / 2, recentreM, snapM };
}

export const TERRAIN_LEVELS: readonly TerrainLevelSpec[] = [
  spec(0, 250, 8_000, 1_000),
  spec(1, 1_000, 32_000, 4_000),
  spec(2, 4_000, 128_000, 16_000),
  spec(3, 16_000, 512_000, 64_000),
];

/** A level's specification. */
export function levelSpec(level: number): TerrainLevelSpec {
  const s = TERRAIN_LEVELS[level];
  if (s === undefined) throw new RangeError(`there is no terrain level ${String(level)}`);
  return s;
}

/** A level's centre, BF `[east, north]` metres. */
export type Centre = readonly [number, number];

/** `pos` snapped to the level's grid. */
export function snapCentre(level: number, pos: Centre): [number, number] {
  const { snapM } = levelSpec(level);
  const snap = (v: number): number => {
    const s = Math.round(v / snapM) * snapM;
    return s === 0 ? 0 : s; // never -0
  };
  return [snap(pos[0]), snap(pos[1])];
}

/**
 * The new centre of `level` if the aircraft at `pos` has left the middle of the level around
 * `centre` (or the level has no centre yet), else undefined. A position that is not finite never
 * moves a level that already has a centre.
 */
export function nextLevelCentre(
  level: number,
  centre: Centre | undefined,
  pos: Centre,
): [number, number] | undefined {
  if (!Number.isFinite(pos[0]) || !Number.isFinite(pos[1])) return undefined;
  if (
    centre !== undefined &&
    Math.hypot(pos[0] - centre[0], pos[1] - centre[1]) <= levelSpec(level).recentreM
  ) {
    return undefined;
  }
  return snapCentre(level, pos);
}

/** The square a level covers, BF metres. */
export interface Footprint {
  west: number;
  east: number;
  south: number;
  north: number;
}

export function footprint(level: number, centre: Centre): Footprint {
  const h = levelSpec(level).halfSpanM;
  return { west: centre[0] - h, east: centre[0] + h, south: centre[1] - h, north: centre[1] + h };
}

/** True if `inner` lies inside `outer` (touching edges count). */
export function footprintWithin(inner: Footprint, outer: Footprint): boolean {
  return (
    inner.west >= outer.west &&
    inner.east <= outer.east &&
    inner.south >= outer.south &&
    inner.north <= outer.north
  );
}

/**
 * The cells of the coarser level that the finer level covers, as half-open ranges of cell indices
 * (cell `i` lies between posts `i` and `i + 1`; `i` counts east, `j` north). The coarser level
 * leaves them out, so the two never draw the same ground. Cells only partly covered stay (they
 * overlap the finer level rather than leave a gap). `undefined` if the footprints do not meet.
 */
export interface HoleCells {
  i0: number;
  i1: number;
  j0: number;
  j1: number;
}

export function holeCells(
  fineLevel: number,
  fineCentre: Centre,
  coarseLevel: number,
  coarseCentre: Centre,
): HoleCells | undefined {
  const coarse = levelSpec(coarseLevel);
  const fine = footprint(fineLevel, fineCentre);
  const origin = footprint(coarseLevel, coarseCentre);
  // Positions of the finer footprint's edges in the coarser grid, in cells. Values within a
  // rounding error of a whole number are that number (the snapping makes them whole).
  const cells = (v: number, from: number): number => (v - from) / coarse.spacingM;
  const lo = (x: number): number => {
    const r = Math.round(x);
    return Math.abs(x - r) < 1e-6 ? r : Math.ceil(x);
  };
  const hi = (x: number): number => {
    const r = Math.round(x);
    return Math.abs(x - r) < 1e-6 ? r : Math.floor(x);
  };
  const i0 = Math.max(0, lo(cells(fine.west, origin.west)));
  const i1 = Math.min(INTERVALS, hi(cells(fine.east, origin.west)));
  const j0 = Math.max(0, lo(cells(fine.south, origin.south)));
  const j1 = Math.min(INTERVALS, hi(cells(fine.north, origin.south)));
  if (i1 <= i0 || j1 <= j0) return undefined;
  return { i0, i1, j0, j1 };
}
