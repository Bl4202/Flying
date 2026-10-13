import { describe, expect, it } from 'vitest';
import { TERRAIN } from '@flying/sim-bridge/layout';
import { buildTerrainMesh, VERTEX_FLOATS } from './terrain-mesh.ts';
import { makeTestPatch } from './terrain-test-patch.ts';

describe('buildTerrainMesh', () => {
  const patch = makeTestPatch({ centreEast: 1000, centreNorth: -2000, spacingM: 250 });
  const mesh = buildTerrainMesh(patch);

  it('has the counts the plan states: 16 641 vertices, 32 768 triangles, 16-bit indices', () => {
    expect(mesh.vertexCount).toBe(16_641);
    expect(mesh.vertices).toHaveLength(16_641 * VERTEX_FLOATS);
    expect(mesh.triangleCount).toBe(32_768);
    expect(mesh.indices).toBeInstanceOf(Uint16Array);
    expect(Math.max(...mesh.indices)).toBe(16_640);
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

  it('winds every triangle counter-clockwise seen from above, so back-face culling keeps the top', () => {
    const p = (i: number): [number, number, number] => [
      mesh.vertices[i * VERTEX_FLOATS] ?? 0,
      mesh.vertices[i * VERTEX_FLOATS + 1] ?? 0,
      mesh.vertices[i * VERTEX_FLOATS + 2] ?? 0,
    ];
    for (let t = 0; t < mesh.triangleCount; t += 101) {
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
    for (let t = 0; t < mesh.triangleCount; t++) {
      const tri = [mesh.indices[t * 3], mesh.indices[t * 3 + 1], mesh.indices[t * 3 + 2]];
      seen.add([...tri].sort((x, y) => (x ?? 0) - (y ?? 0)).join(','));
    }
    expect(seen.size).toBe(mesh.triangleCount);
  });

  it('refuses patches of the wrong size or shape', () => {
    expect(() => buildTerrainMesh(new Uint8Array(100))).toThrow(/SYS-040/);
    const bad = patch.slice();
    new DataView(bad.buffer).setUint32(TERRAIN.H_POSTS, 65, true);
    expect(() => buildTerrainMesh(bad)).toThrow(/65 posts/);
  });
});
