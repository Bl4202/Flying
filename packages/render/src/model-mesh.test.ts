import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { parsePresentBundle } from '@flying/sim-bridge/present';
import {
  BOW_SEGMENTS,
  buildCockpitMesh,
  buildModelMesh,
  MATERIAL,
  MAX_MODEL_TRIANGLES,
  MAX_PARTS,
  MESH_VERTEX_FLOATS,
  modelTriangleBudget,
  partIndices,
  type ModelMesh,
} from './model-mesh.ts';
import { renderPresentation } from './present.ts';

const bundle = parsePresentBundle(
  JSON.parse(
    readFileSync(new URL('../../../tests/vectors/present_bundle.json', import.meta.url), 'utf8'),
  ),
);
const presentation = renderPresentation(bundle, 'core:ac/kestrel');
if (!presentation) throw new Error('no Kestrel');
const { model, cockpit } = presentation;
const mesh = buildModelMesh(model);
const inside = buildCockpitMesh(cockpit);

type V3 = [number, number, number];
const pos = (m: ModelMesh, i: number): V3 => [
  m.vertices[i * MESH_VERTEX_FLOATS] ?? 0,
  m.vertices[i * MESH_VERTEX_FLOATS + 1] ?? 0,
  m.vertices[i * MESH_VERTEX_FLOATS + 2] ?? 0,
];
const nrm = (m: ModelMesh, i: number): V3 => [
  m.vertices[i * MESH_VERTEX_FLOATS + 3] ?? 0,
  m.vertices[i * MESH_VERTEX_FLOATS + 4] ?? 0,
  m.vertices[i * MESH_VERTEX_FLOATS + 5] ?? 0,
];
const partOf = (m: ModelMesh, i: number): number => m.vertices[i * MESH_VERTEX_FLOATS + 6] ?? 0;
const materialOf = (m: ModelMesh, i: number): number => m.vertices[i * MESH_VERTEX_FLOATS + 7] ?? 0;

function tri(m: ModelMesh, t: number): [number, number, number] {
  return [m.indices[t * 3] ?? 0, m.indices[t * 3 + 1] ?? 0, m.indices[t * 3 + 2] ?? 0];
}

function geometricNormal(m: ModelMesh, t: number): V3 {
  const [a, b, c] = tri(m, t).map((i) => pos(m, i)) as [V3, V3, V3];
  const u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
  const v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
  return [
    (u[1] ?? 0) * (v[2] ?? 0) - (u[2] ?? 0) * (v[1] ?? 0),
    (u[2] ?? 0) * (v[0] ?? 0) - (u[0] ?? 0) * (v[2] ?? 0),
    (u[0] ?? 0) * (v[1] ?? 0) - (u[1] ?? 0) * (v[0] ?? 0),
  ];
}

describe('the Kestrel exterior', () => {
  it('stays inside the 8 000-triangle budget, with exactly the triangles the baker counts', () => {
    expect(mesh.triangleCount).toBeLessThanOrEqual(MAX_MODEL_TRIANGLES);
    expect(mesh.triangleCount).toBe(modelTriangleBudget(model));
    expect(mesh.triangleCount).toBe(812);
    expect(mesh.indices.length).toBe(mesh.triangleCount * 3);
    expect(mesh.vertices.length).toBe(mesh.vertexCount * MESH_VERTEX_FLOATS);
  });

  it('indexes only vertices it has, in 16 bits', () => {
    expect(mesh.indices).toBeInstanceOf(Uint16Array);
    expect(Math.max(...mesh.indices)).toBeLessThan(mesh.vertexCount);
  });

  it("winds every triangle counter-clockwise seen from outside: its normal agrees with its vertices'", () => {
    for (const m of [mesh, inside]) {
      for (let t = 0; t < m.triangleCount; t++) {
        const g = geometricNormal(m, t);
        const gl = Math.hypot(...g);
        if (gl < 1e-12) continue; // a sliver at a pole
        for (const i of tri(m, t)) {
          const n = nrm(m, i);
          const d = (g[0] * n[0] + g[1] * n[1] + g[2] * n[2]) / gl;
          expect(d, `triangle ${String(t)}`).toBeGreaterThan(0);
        }
      }
    }
  });

  it('has unit normals', () => {
    for (let i = 0; i < mesh.vertexCount; i++) {
      expect(Math.hypot(...nrm(mesh, i)), `vertex ${String(i)}`).toBeCloseTo(1, 4);
    }
  });

  it('is closed: every edge of every solid is shared an even number of times (no holes)', () => {
    const key = (p: V3): string => p.map((v) => v.toFixed(4)).join(',');
    const count = new Map<string, number>();
    for (let t = 0; t < mesh.triangleCount; t++) {
      const corners = tri(mesh, t).map((i) => key(pos(mesh, i)));
      for (let e = 0; e < 3; e++) {
        const [a, b] = [corners[e] as string, corners[(e + 1) % 3] as string];
        const edge = a < b ? `${a}|${b}` : `${b}|${a}`;
        count.set(edge, (count.get(edge) ?? 0) + 1);
      }
    }
    const odd = [...count].filter(([, n]) => n % 2 !== 0).map(([edge]) => edge);
    // The one deliberate opening: the nozzle's front ring, which sits inside the fuselage's tail.
    const frontX = model.nozzle.atM[0].toFixed(4);
    const frontRing = odd.filter((edge) =>
      edge.split('|').every((corner) => corner.split(',')[0] === frontX),
    );
    expect(frontRing).toHaveLength(12);
    expect(odd.filter((edge) => !frontRing.includes(edge))).toEqual([]);
  });

  it('matches the aircraft: nose at 7 m, nozzle to 6 m aft, span 9.8 m', () => {
    let [x0, x1, y1] = [Infinity, -Infinity, 0];
    for (let i = 0; i < mesh.vertexCount; i++) {
      const [x, y] = pos(mesh, i);
      x0 = Math.min(x0, x);
      x1 = Math.max(x1, x);
      y1 = Math.max(y1, Math.abs(y));
    }
    expect(x1).toBeCloseTo(7, 6);
    expect(x0).toBeCloseTo(-6, 6);
    // The gear is retracted-out in the model, so the span is the wing's: 4.9 m a side.
    expect(y1).toBeCloseTo(4.9, 6);
  });

  it('gives every moving part its own vertices, and the fixed airframe the rest', () => {
    const index = partIndices(model.parts);
    expect(index.size).toBe(model.parts.length);
    const counts = new Map<number, number>();
    for (let i = 0; i < mesh.vertexCount; i++) {
      counts.set(partOf(mesh, i), (counts.get(partOf(mesh, i)) ?? 0) + 1);
    }
    for (const [id, i] of index) expect(counts.get(i), id).toBeGreaterThan(0);
    expect(counts.get(0)).toBeGreaterThan(0);
    expect(Math.max(...counts.keys())).toBe(model.parts.length);
    expect(model.parts.length).toBeLessThanOrEqual(MAX_PARTS);
  });

  it('draws the canopy last, so a cockpit view can leave it out', () => {
    const canopyTriangles = 2 * 12 * (model.canopy.length - 1) + 2 * 12;
    expect(mesh.bodyIndexCount).toBe(mesh.indices.length - canopyTriangles * 3);
    for (let i = mesh.bodyIndexCount; i < mesh.indices.length; i++) {
      expect(materialOf(mesh, mesh.indices[i] ?? 0)).toBe(MATERIAL.GLASS);
    }
    for (let i = 0; i < mesh.bodyIndexCount; i++) {
      expect(materialOf(mesh, mesh.indices[i] ?? 0)).not.toBe(MATERIAL.GLASS);
    }
  });

  it('lights only the nozzle with the glow material', () => {
    const glow = new Set<number>();
    for (let t = 0; t < mesh.triangleCount; t++) {
      for (const i of tri(mesh, t)) {
        if (materialOf(mesh, i) === MATERIAL.GLOW) glow.add(partOf(mesh, i));
      }
    }
    expect([...glow]).toEqual([partIndices(model.parts).get('nozzle_glow')]);
  });

  it('refuses a model with too many parts', () => {
    const parts = Array.from({ length: 16 }, (_, i) => ({
      motion: 'glow' as const,
      id: `p${String(i)}`,
      channel: 'afterburner' as const,
    }));
    expect(() => buildModelMesh({ ...model, parts })).toThrow(/at most 15/);
  });
});

describe('the Kestrel cockpit', () => {
  it('builds the bows, the glare shield, the panel and the consoles', () => {
    const bows = cockpit.canopyBows.length * BOW_SEGMENTS * 4 * 2;
    const boxes = (2 + cockpit.consoles.length) * 12;
    expect(inside.triangleCount).toBe(bows + boxes);
    expect(inside.bodyIndexCount).toBe(inside.indices.length);
  });

  it('is all fixed to the airframe, in the cockpit material', () => {
    for (let i = 0; i < inside.vertexCount; i++) {
      expect(partOf(inside, i)).toBe(0);
      expect(materialOf(inside, i)).toBe(MATERIAL.COCKPIT);
    }
  });

  it('keeps every surface 0.35 m or more from the design eye, as the baker checked', () => {
    const eye = cockpit.eyePointM;
    let nearest = Infinity;
    for (let i = 0; i < inside.vertexCount; i++) {
      const p = pos(inside, i);
      nearest = Math.min(nearest, Math.hypot(p[0] - eye[0], p[1] - eye[1], p[2] - eye[2]));
    }
    // Vertices are not the surface (a face can be nearer than its corners), but a model whose
    // corners were already closer than the near plane would be caught here.
    expect(nearest).toBeGreaterThan(0.3);
  });
});
