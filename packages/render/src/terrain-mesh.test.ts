import { describe, expect, it } from 'vitest';
import { TERRAIN } from '@flying/sim-bridge/layout';
import {
  buildTerrainMesh,
  sameHole,
  VERTEX_FLOATS,
  withHole,
  type TerrainMesh,
} from './terrain-mesh.ts';
import { makeTestPatch } from './terrain-test-patch.ts';

describe('buildTerrainMesh', () => {
  const patch = makeTestPatch({ centreEast: 1000, centreNorth: -2000, spacingM: 250 });
  const mesh = buildTerrainMesh(patch);

  it('has 16 641 grid vertices and 32 768 surface triangles, then the skirts, in 16-bit indices', () => {
    expect(mesh.gridVertexCount).toBe(16_641);
    expect(mesh.surfaceTriangleCount).toBe(32_768);
    // The outer skirt: 512 edge segments, four triangles each (a quad, drawn both ways round).
    expect(mesh.vertexCount).toBe(16_641 + 512);
    expect(mesh.vertices).toHaveLength(mesh.vertexCount * VERTEX_FLOATS);
    expect(mesh.triangleCount).toBe(32_768 + 512 * 4);
    expect(mesh.indices).toBeInstanceOf(Uint16Array);
    expect(Math.max(...mesh.indices)).toBe(mesh.vertexCount - 1);
    // The grid comes first, untouched.
    expect(mesh.vertices.subarray(0, mesh.grid.length)).toEqual(mesh.grid);
  });

  it('keeps the origin in f64 BF and the spacing', () => {
    expect([...mesh.originBf]).toEqual([1000, -2000, 0]);
    expect(mesh.spacingM).toBe(250);
  });

  it('converts BF east, north, up to render east, up, south', () => {
    const bf = new DataView(patch.buffer);
    for (const [i, j] of [
      [0, 0],
      [10, 3],
      [64, 64],
      [128, 128],
      [5, 100],
    ] as const) {
      const v = j * TERRAIN.POSTS + i;
      const at = TERRAIN.HEADER_SIZE + v * TERRAIN.VERTEX_BYTES;
      const [x, y, z] = [0, 4, 8].map((o) => bf.getFloat32(at + o, true));
      const [nx, ny, nz] = [12, 16, 20].map((o) => bf.getFloat32(at + o, true));
      const m = v * VERTEX_FLOATS;
      expect([mesh.vertices[m], mesh.vertices[m + 1], mesh.vertices[m + 2]]).toEqual([
        x,
        z,
        -(y ?? 0),
      ]);
      expect([mesh.vertices[m + 3], mesh.vertices[m + 4], mesh.vertices[m + 5]]).toEqual([
        nx,
        nz,
        -(ny ?? 0),
      ]);
      expect(mesh.vertices[m + 6]).toBe(bf.getFloat32(at + TERRAIN.V_HEIGHT, true));
    }
  });

  it('has unit normals pointing up (render +y) everywhere', () => {
    for (let v = 0; v < mesh.vertexCount; v += 37) {
      const m = v * VERTEX_FLOATS;
      const n = [mesh.vertices[m + 3] ?? 0, mesh.vertices[m + 4] ?? 0, mesh.vertices[m + 5] ?? 0];
      expect(Math.hypot(...n)).toBeCloseTo(1, 5);
      expect(n[1]).toBeGreaterThan(0.9);
    }
  });

  it('winds every surface triangle counter-clockwise seen from above, so back-face culling keeps the top', () => {
    const p = (i: number): [number, number, number] => [
      mesh.vertices[i * VERTEX_FLOATS] ?? 0,
      mesh.vertices[i * VERTEX_FLOATS + 1] ?? 0,
      mesh.vertices[i * VERTEX_FLOATS + 2] ?? 0,
    ];
    for (let t = 0; t < mesh.surfaceTriangleCount; t += 101) {
      const [a, b, c] = [
        p(mesh.indices[t * 3] ?? 0),
        p(mesh.indices[t * 3 + 1] ?? 0),
        p(mesh.indices[t * 3 + 2] ?? 0),
      ];
      const u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
      const v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
      const normalY = (u[2] ?? 0) * (v[0] ?? 0) - (u[0] ?? 0) * (v[2] ?? 0); // y of u x v
      expect(normalY, `triangle ${String(t)}`).toBeGreaterThan(0);
    }
  });

  it('covers every cell exactly twice-triangulated with no repeats', () => {
    const seen = new Set<string>();
    for (let t = 0; t < mesh.surfaceTriangleCount; t++) {
      const tri = [mesh.indices[t * 3], mesh.indices[t * 3 + 1], mesh.indices[t * 3 + 2]];
      seen.add([...tri].sort((x, y) => (x ?? 0) - (y ?? 0)).join(','));
    }
    expect(seen.size).toBe(mesh.surfaceTriangleCount);
  });

  it('refuses patches of the wrong size or shape', () => {
    expect(() => buildTerrainMesh(new Uint8Array(100))).toThrow(/SYS-040/);
    const bad = patch.slice();
    new DataView(bad.buffer).setUint32(TERRAIN.H_POSTS, 65, true);
    expect(() => buildTerrainMesh(bad)).toThrow(/65 posts/);
  });
});

describe('holes and skirts', () => {
  const patch = makeTestPatch({ spacingM: 1000 });
  const plain = buildTerrainMesh(patch, { level: 1 });
  const hole = { i0: 48, i1: 80, j0: 48, j1: 80 };
  const holed = withHole(plain, hole);

  /** The grid vertex pairs that bound the surface: edges used by exactly one surface triangle. */
  function boundaryEdges(mesh: TerrainMesh): Set<string> {
    const count = new Map<string, number>();
    for (let t = 0; t < mesh.surfaceTriangleCount; t++) {
      const tri = [
        mesh.indices[t * 3],
        mesh.indices[t * 3 + 1],
        mesh.indices[t * 3 + 2],
      ] as number[];
      for (let e = 0; e < 3; e++) {
        const [a, b] = [tri[e] ?? 0, tri[(e + 1) % 3] ?? 0];
        const key = a < b ? `${String(a)},${String(b)}` : `${String(b)},${String(a)}`;
        count.set(key, (count.get(key) ?? 0) + 1);
      }
    }
    return new Set([...count].filter(([, n]) => n === 1).map(([key]) => key));
  }

  /** The grid-vertex edges that a skirt hangs from. */
  function skirtTops(mesh: TerrainMesh): Set<string> {
    const tops = new Set<string>();
    for (let t = mesh.surfaceTriangleCount; t < mesh.triangleCount; t++) {
      const tri = [
        mesh.indices[t * 3],
        mesh.indices[t * 3 + 1],
        mesh.indices[t * 3 + 2],
      ] as number[];
      const onGrid = tri.filter((v) => v < mesh.gridVertexCount).sort((x, y) => x - y);
      if (onGrid.length === 2) tops.add(onGrid.join(','));
    }
    return tops;
  }

  it('leaves the hole cells out: two triangles fewer per cell', () => {
    expect(holed.surfaceTriangleCount).toBe(32_768 - 2 * 32 * 32);
    expect(holed.hole).toEqual(hole);
    expect(plain.hole).toBeUndefined();
  });

  it('draws no surface triangle inside the hole', () => {
    const half = TERRAIN.POSTS - 1;
    for (let t = 0; t < holed.surfaceTriangleCount; t++) {
      const v = holed.indices[t * 3] ?? 0;
      const i = v % TERRAIN.POSTS;
      const j = Math.floor(v / TERRAIN.POSTS);
      // The triangle's first vertex is its cell's corner or one past it; test the cell centre.
      const cx = [0, 1, 2].map((k) => (holed.indices[t * 3 + k] ?? 0) % TERRAIN.POSTS);
      const cy = [0, 1, 2].map((k) => Math.floor((holed.indices[t * 3 + k] ?? 0) / TERRAIN.POSTS));
      const mx = (Math.min(...cx) + Math.max(...cx)) / 2;
      const my = (Math.min(...cy) + Math.max(...cy)) / 2;
      const inside = mx > hole.i0 && mx < hole.i1 && my > hole.j0 && my < hole.j1;
      expect(inside, `triangle ${String(t)} at ${String(i)},${String(j)} of ${String(half)}`).toBe(
        false,
      );
    }
  });

  it('hangs a skirt from every edge of the surface, outer and hole', () => {
    for (const mesh of [plain, holed]) {
      const boundary = boundaryEdges(mesh);
      const tops = skirtTops(mesh);
      expect(boundary.size).toBe(mesh === plain ? 512 : 512 + 128);
      for (const edge of boundary) expect(tops.has(edge), `no skirt on ${edge}`).toBe(true);
      expect(tops.size).toBe(boundary.size);
    }
  });

  it('lowers the skirt vertices straight down by one spacing, keeping normal and height', () => {
    expect(plain.skirtDepthM).toBe(1000);
    for (let v = plain.gridVertexCount; v < plain.vertexCount; v += 17) {
      const m = v * VERTEX_FLOATS;
      // Find the grid vertex it copies: same x and z, and a position `depth` above.
      const x = plain.vertices[m] ?? 0;
      const z = plain.vertices[m + 2] ?? 0;
      const above = Array.from({ length: plain.gridVertexCount }, (_, g) => g).find(
        (g) =>
          plain.vertices[g * VERTEX_FLOATS] === x && plain.vertices[g * VERTEX_FLOATS + 2] === z,
      );
      expect(above).toBeDefined();
      const g = (above ?? 0) * VERTEX_FLOATS;
      expect((plain.vertices[g + 1] ?? 0) - (plain.vertices[m + 1] ?? 0)).toBeCloseTo(1000, 3);
      for (const c of [3, 4, 5, 6]) expect(plain.vertices[m + c]).toBe(plain.vertices[g + c]);
    }
  });

  it('makes every skirt triangle visible from both sides', () => {
    // Each skirt quad is two triangles one way round and two the other: pair them up.
    const seen = new Map<string, number>();
    for (let t = holed.surfaceTriangleCount; t < holed.triangleCount; t++) {
      const tri = [
        holed.indices[t * 3],
        holed.indices[t * 3 + 1],
        holed.indices[t * 3 + 2],
      ] as number[];
      const key = [...tri].sort((x, y) => x - y).join(',');
      seen.set(key, (seen.get(key) ?? 0) + 1);
    }
    expect([...seen.values()].every((n) => n === 2)).toBe(true);
  });

  it('shares the grid and leaves the mesh it was made from alone', () => {
    expect(holed.grid).toBe(plain.grid);
    expect(plain.hole).toBeUndefined();
    expect(withHole(holed, undefined).triangleCount).toBe(plain.triangleCount);
  });

  it('compares holes', () => {
    expect(sameHole(undefined, undefined)).toBe(true);
    expect(sameHole(hole, { ...hole })).toBe(true);
    expect(sameHole(hole, undefined)).toBe(false);
    expect(sameHole(hole, { ...hole, j1: 79 })).toBe(false);
  });

  it('stays within 16-bit indices even with the biggest hole', () => {
    const big = withHole(plain, { i0: 0, i1: 128, j0: 0, j1: 128 });
    expect(big.vertexCount).toBeLessThan(65_536);
    expect(big.surfaceTriangleCount).toBe(0);
  });
});
