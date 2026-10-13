/**
 * Turns the terrain patch bytes from the sim (layout `TERRAIN`) into a mesh the GPU draws.
 *
 * The sim stores positions and normals in BF axes (east, north, up), patch-local so `f32` is
 * plenty. The mesh converts them to render axes (east, up, south) once. The patch origin stays
 * as an `f64` so each frame can subtract the eye from it before rounding (GDD 44.8): the vertices
 * never move, only a per-draw offset does.
 */
import { TERRAIN } from '@flying/sim-bridge/layout';

export const VERTEX_FLOATS = 7; // position (3), normal (3), height above MSL (1)
export const VERTEX_BYTES = VERTEX_FLOATS * 4;
const POSTS = TERRAIN.POSTS;
const INTERVALS = POSTS - 1;

export interface TerrainMesh {
  /** Interleaved position, normal, height; render axes, patch-local. */
  vertices: Float32Array;
  /** Triangle list, counter-clockwise seen from above. */
  indices: Uint16Array;
  /** Patch origin in BF metres (east, north, up). */
  originBf: Float64Array;
  /** Distance between posts, metres. */
  spacingM: number;
  vertexCount: number;
  triangleCount: number;
}

export function buildTerrainMesh(patch: Uint8Array): TerrainMesh {
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

  const vertices = new Float32Array(TERRAIN.VERTEX_COUNT * VERTEX_FLOATS);
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
    vertices[out] = px;
    vertices[out + 1] = pz;
    vertices[out + 2] = -py;
    vertices[out + 3] = nx;
    vertices[out + 4] = nz;
    vertices[out + 5] = -ny;
    vertices[out + 6] = dv.getFloat32(at + TERRAIN.V_HEIGHT, true);
  }

  // Posts are row-major, east fastest. Seen from above with north up, (i,j) -> (i+1,j) -> (i,j+1)
  // turns counter-clockwise.
  const indices = new Uint16Array(INTERVALS * INTERVALS * 6);
  let k = 0;
  for (let j = 0; j < INTERVALS; j++) {
    for (let i = 0; i < INTERVALS; i++) {
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
  return {
    vertices,
    indices,
    originBf,
    spacingM,
    vertexCount: TERRAIN.VERTEX_COUNT,
    triangleCount: indices.length / 3,
  };
}
