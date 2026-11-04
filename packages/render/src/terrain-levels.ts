/**
 * The four terrain levels as the renderer holds them (ADR-40). Installing a level (a patch from
 * the sim) gives it a hole where the next finer level is, and gives the next coarser level a new
 * hole where this one now is, so the set never draws the same ground twice and never leaves a gap.
 */
import { TERRAIN } from '@flying/sim-bridge/layout';
import { holeCells, TERRAIN_LEVEL_COUNT } from '@flying/sim-bridge/terrain-levels';
import {
  buildTerrainMesh,
  sameHole,
  VERTEX_FLOATS,
  withHole,
  type TerrainMesh,
} from './terrain-mesh.ts';

const POSTS = TERRAIN.POSTS;
const INTERVALS = POSTS - 1;

/**
 * The height (BF up) of a mesh's surface at BF `(east, north)`, on the very triangles the GPU
 * draws (each cell is split along its diagonal from the south-east post to the north-west one), or
 * NaN if the point is outside the mesh.
 */
export function sampleMesh(mesh: TerrainMesh, eastM: number, northM: number): number {
  const fx = (eastM - (mesh.originBf[0] ?? 0)) / mesh.spacingM + INTERVALS / 2;
  const fy = (northM - (mesh.originBf[1] ?? 0)) / mesh.spacingM + INTERVALS / 2;
  if (!(fx >= 0 && fx <= INTERVALS && fy >= 0 && fy <= INTERVALS)) return Number.NaN;
  const i = Math.min(Math.floor(fx), INTERVALS - 1);
  const j = Math.min(Math.floor(fy), INTERVALS - 1);
  const u = fx - i;
  const v = fy - j;
  // Render y is BF up. Posts: a (i, j), b (i + 1, j), c (i, j + 1), d (i + 1, j + 1).
  const at = (pi: number, pj: number): number =>
    mesh.grid[(pj * POSTS + pi) * VERTEX_FLOATS + 1] ?? 0;
  const ha = at(i, j);
  const hb = at(i + 1, j);
  const hc = at(i, j + 1);
  if (u + v <= 1) return ha + u * (hb - ha) + v * (hc - ha);
  const hd = at(i + 1, j + 1);
  return hd + (1 - u) * (hc - hd) + (1 - v) * (hb - hd);
}

export class TerrainSet {
  readonly #meshes: (TerrainMesh | undefined)[] = Array.from(
    { length: TERRAIN_LEVEL_COUNT },
    () => undefined,
  );

  /** The mesh of a level, or undefined if none has arrived. */
  mesh(level: number): TerrainMesh | undefined {
    return this.#meshes[level];
  }

  /**
   * The terrain's BF height (up) at BF `(east, north)`, from the finest level that covers the
   * point, or NaN if none does. The cameras use it to stay above the ground.
   */
  heightAt(eastM: number, northM: number): number {
    // Plain loops and scalars: the cameras call this dozens of times a frame, and it makes nothing.
    for (let level = 0; level < TERRAIN_LEVEL_COUNT; level++) {
      const mesh = this.#meshes[level];
      if (mesh === undefined) continue;
      const h = sampleMesh(mesh, eastM, northM);
      if (!Number.isNaN(h)) return h;
    }
    return Number.NaN;
  }

  /** Bit `n` is set if level `n` has a mesh. */
  get mask(): number {
    return this.#meshes.reduce((m: number, mesh, n) => (mesh ? m | (1 << n) : m), 0);
  }

  /** The hole level `level` should have, given the finer level now installed. */
  #holeFor(level: number): ReturnType<typeof holeCells> {
    const fine = level > 0 ? this.#meshes[level - 1] : undefined;
    const coarse = this.#meshes[level];
    if (fine === undefined || coarse === undefined) return undefined;
    return holeCells(level - 1, [fine.originBf[0] ?? 0, fine.originBf[1] ?? 0], level, [
      coarse.originBf[0] ?? 0,
      coarse.originBf[1] ?? 0,
    ]);
  }

  /**
   * Installs the patch for `level` and returns the levels whose meshes changed (this one, and the
   * next coarser if its hole moved), nearest first, so the renderer can upload them.
   *
   * @throws RangeError (`SYS-040`) if the bytes are not a terrain patch, or the level is unknown.
   */
  set(level: number, patch: Uint8Array): number[] {
    if (!Number.isInteger(level) || level < 0 || level >= TERRAIN_LEVEL_COUNT) {
      throw new RangeError(`SYS-040: there is no terrain level ${String(level)}`);
    }
    const built = buildTerrainMesh(patch, { level });
    this.#meshes[level] = built;
    const changed = [level];
    const own = this.#holeFor(level);
    if (own !== undefined) this.#meshes[level] = withHole(built, own);
    const next = level + 1;
    const coarser = this.#meshes[next];
    if (coarser) {
      const hole = this.#holeFor(next);
      if (!sameHole(hole, coarser.hole)) {
        this.#meshes[next] = withHole(coarser, hole);
        changed.push(next);
      }
    }
    return changed;
  }

  /** Every installed mesh, nearest first. */
  installed(): TerrainMesh[] {
    return this.#meshes.flatMap((m) => (m ? [m] : []));
  }
}
