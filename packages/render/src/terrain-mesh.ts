/**
 * Turns a terrain level's bytes from the sim (layout `TERRAIN`) into a mesh the GPU draws.
 *
 * The sim stores positions and normals in BF axes (east, north, up), patch-local so `f32` is
 * plenty. The mesh converts them to render axes (east, up, south) once. The patch origin stays
 * as an `f64` so each frame can subtract the eye from it before rounding (GDD 44.8): the vertices
 * never move, only a per-draw offset does.
 *
 * Levels nest (ADR-40), so a mesh has two extras:
 *
 * - a **hole**: the cells of this level that a finer level covers are left out, so the two never
 *   draw the same ground. The hole is a rectangle of whole cells and changes when the finer level
 *   moves, which rebuilds only the indices and the skirt vertices (`withHole`);
 * - **skirts**: a wall hanging straight down from the outer edge of the patch and from the edge of
 *   the hole. Where two levels meet, their edges are sampled differently and could leave a thin
 *   gap that shows whatever lies behind it; the skirt closes it. A skirt is drawn from both sides
 *   (it has both windings), so it needs no care about which way round it faces.
 *
 * The grid vertices come first and are never reordered; the lowered skirt vertices follow, then
 * the surface triangles, then the skirt triangles.
 */
import { TERRAIN } from '@flying/sim-bridge/layout';
import type { HoleCells } from '@flying/sim-bridge/terrain-levels';

export const VERTEX_FLOATS = 7; // position (3), normal (3), height above MSL (1)
export const VERTEX_BYTES = VERTEX_FLOATS * 4;
const POSTS = TERRAIN.POSTS;
const INTERVALS = POSTS - 1;

/** A skirt hangs this far below its edge, or one post spacing if that is more. */
const SKIRT_MIN_M = 100;

/** Depth of a level's skirts, metres. */
export function skirtDepthM(spacingM: number): number {
  return Math.max(SKIRT_MIN_M, spacingM);
}

export interface TerrainMesh {
  /** Which level this is (0 is the finest). */
  level: number;
  /** Interleaved position, normal, height; render axes, patch-local. Grid vertices, then skirt vertices. */
  vertices: Float32Array;
  /** Triangle list: the surface (counter-clockwise seen from above), then the skirts. */
  indices: Uint16Array;
  /** Patch origin in BF metres (east, north, up). */
  originBf: Float64Array;
  /** Distance between posts, metres. */
  spacingM: number;
  /** Grid vertices (`POSTS` squared); the vertices after these are the lowered skirt vertices. */
  gridVertexCount: number;
  vertexCount: number;
  triangleCount: number;
  /** Triangles of the surface alone: the first `surfaceTriangleCount` ones in `indices`. */
  surfaceTriangleCount: number;
  skirtDepthM: number;
  /** The cells left out, or undefined if none are. */
  hole: HoleCells | undefined;
  /** The grid vertices alone, shared by every mesh made from the same patch. */
  readonly grid: Float32Array;
}

/** Grid vertex indices around the rectangle `i0..i1` by `j0..j1`, as a closed loop. */
function ringAround(i0: number, i1: number, j0: number, j1: number): number[] {
  const ring: number[] = [];
  for (let i = i0; i < i1; i++) ring.push(j0 * POSTS + i); // south edge, eastwards
  for (let j = j0; j < j1; j++) ring.push(j * POSTS + i1); // east edge, northwards
  for (let i = i1; i > i0; i--) ring.push(j1 * POSTS + i); // north edge, westwards
  for (let j = j1; j > j0; j--) ring.push(j * POSTS + i0); // west edge, southwards
  return ring;
}

/** Reads a patch into its grid vertices and origin. */
function readGrid(patch: Uint8Array): {
  grid: Float32Array;
  originBf: Float64Array;
  spacingM: number;
} {
  if (patch.byteLength !== TERRAIN.SIZE) {
    throw new RangeError(
      `SYS-040: terrain patch is ${String(patch.byteLength)} bytes, expected ${String(TERRAIN.SIZE)}`,
    );
  }
  const dv = new DataView(patch.buffer, patch.byteOffset, patch.byteLength);
  const posts = dv.getUint32(TERRAIN.H_POSTS, true);
  if (posts !== POSTS) {
    throw new RangeError(
      `SYS-040: terrain patch has ${String(posts)} posts, expected ${String(POSTS)}`,
    );
  }
  const originBf = new Float64Array([
    dv.getFloat64(TERRAIN.H_ORIGIN_BF, true),
    dv.getFloat64(TERRAIN.H_ORIGIN_BF + 8, true),
    dv.getFloat64(TERRAIN.H_ORIGIN_BF + 16, true),
  ]);
  const spacingM = dv.getFloat32(TERRAIN.H_SPACING_M, true);

  const grid = new Float32Array(TERRAIN.VERTEX_COUNT * VERTEX_FLOATS);
  for (let v = 0; v < TERRAIN.VERTEX_COUNT; v++) {
    const at = TERRAIN.HEADER_SIZE + v * TERRAIN.VERTEX_BYTES;
    const [px, py, pz] = [0, 4, 8].map((o) => dv.getFloat32(at + TERRAIN.V_POS + o, true)) as [
      number,
      number,
      number,
    ];
    const [nx, ny, nz] = [0, 4, 8].map((o) => dv.getFloat32(at + TERRAIN.V_NORMAL + o, true)) as [
      number,
      number,
      number,
    ];
    const out = v * VERTEX_FLOATS;
    // BF (east, north, up) -> render (east, up, south).
    grid[out] = px;
    grid[out + 1] = pz;
    grid[out + 2] = -py;
    grid[out + 3] = nx;
    grid[out + 4] = nz;
    grid[out + 5] = -ny;
    grid[out + 6] = dv.getFloat32(at + TERRAIN.V_HEIGHT, true);
  }
  return { grid, originBf, spacingM };
}

/**
 * Builds the mesh of a patch for `level`, optionally with a hole left in it.
 *
 * @throws RangeError (`SYS-040`) if the bytes are not a terrain patch.
 */
export function buildTerrainMesh(
  patch: Uint8Array,
  opts: { level?: number; hole?: HoleCells | undefined } = {},
): TerrainMesh {
  const { grid, originBf, spacingM } = readGrid(patch);
  return assemble(grid, originBf, spacingM, opts.level ?? 0, opts.hole);
}

/** The same patch with another hole (or none): a new mesh that shares the grid. */
export function withHole(mesh: TerrainMesh, hole: HoleCells | undefined): TerrainMesh {
  return assemble(mesh.grid, mesh.originBf, mesh.spacingM, mesh.level, hole);
}

/** True if two holes leave out the same cells. */
export function sameHole(a: HoleCells | undefined, b: HoleCells | undefined): boolean {
  if (a === undefined || b === undefined) return a === b;
  return a.i0 === b.i0 && a.i1 === b.i1 && a.j0 === b.j0 && a.j1 === b.j1;
}

function assemble(
  grid: Float32Array,
  originBf: Float64Array,
  spacingM: number,
  level: number,
  hole: HoleCells | undefined,
): TerrainMesh {
  const depth = skirtDepthM(spacingM);
  const outer = ringAround(0, INTERVALS, 0, INTERVALS);
  const inner = hole ? ringAround(hole.i0, hole.i1, hole.j0, hole.j1) : [];
  const gridVertexCount = TERRAIN.VERTEX_COUNT;
  const vertexCount = gridVertexCount + outer.length + inner.length;
  const vertices = new Float32Array(vertexCount * VERTEX_FLOATS);
  vertices.set(grid);

  // The lowered copy of each ring vertex: same normal and height (so the colour carries on down),
  // only the position drops.
  const lowered = (ring: readonly number[], first: number): void => {
    ring.forEach((v, k) => {
      const out = (first + k) * VERTEX_FLOATS;
      for (let c = 0; c < VERTEX_FLOATS; c++) vertices[out + c] = grid[v * VERTEX_FLOATS + c] ?? 0;
      vertices[out + 1] = (grid[v * VERTEX_FLOATS + 1] ?? 0) - depth;
    });
  };
  lowered(outer, gridVertexCount);
  lowered(inner, gridVertexCount + outer.length);

  const holeCellCount = hole ? (hole.i1 - hole.i0) * (hole.j1 - hole.j0) : 0;
  const surface = INTERVALS * INTERVALS - holeCellCount;
  const skirtTriangles = (outer.length + inner.length) * 4;
  const indices = new Uint16Array((surface * 2 + skirtTriangles) * 3);
  let k = 0;
  // Posts are row-major, east fastest. Seen from above with north up, (i,j) -> (i+1,j) -> (i,j+1)
  // turns counter-clockwise.
  for (let j = 0; j < INTERVALS; j++) {
    for (let i = 0; i < INTERVALS; i++) {
      if (hole && i >= hole.i0 && i < hole.i1 && j >= hole.j0 && j < hole.j1) continue;
      const a = j * POSTS + i;
      const b = a + 1;
      const c = a + POSTS;
      const d = c + 1;
      indices[k++] = a;
      indices[k++] = b;
      indices[k++] = c;
      indices[k++] = b;
      indices[k++] = d;
      indices[k++] = c;
    }
  }
  const surfaceTriangleCount = k / 3;

  const wall = (ring: readonly number[], first: number): void => {
    for (let n = 0; n < ring.length; n++) {
      const m = (n + 1) % ring.length;
      const a = ring[n] ?? 0;
      const b = ring[m] ?? 0;
      const la = first + n;
      const lb = first + m;
      // The quad a, b, lb, la twice: once each way round.
      indices[k++] = a;
      indices[k++] = b;
      indices[k++] = lb;
      indices[k++] = a;
      indices[k++] = lb;
      indices[k++] = la;
      indices[k++] = a;
      indices[k++] = lb;
      indices[k++] = b;
      indices[k++] = a;
      indices[k++] = la;
      indices[k++] = lb;
    }
  };
  wall(outer, gridVertexCount);
  wall(inner, gridVertexCount + outer.length);

  return {
    level,
    vertices,
    indices,
    originBf,
    spacingM,
    gridVertexCount,
    vertexCount,
    triangleCount: indices.length / 3,
    surfaceTriangleCount,
    skirtDepthM: depth,
    hole,
    grid,
  };
}
