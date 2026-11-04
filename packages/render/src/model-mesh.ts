/**
 * The procedural exterior of an aircraft and its cockpit (ADR-41), built in code from the `model/1`
 * and `cockpit_layout/1` presentation data. Original shapes (ADR-11): rings at stations for the
 * fuselage and the canopy, trapezoid slabs for wings and tail, boxes for intakes and plates, a cone
 * for the nozzle, a strut and a wheel for each gear leg. At most 8 000 triangles; the Kestrel needs
 * 812 (the baker counts them by the same rules, `present_check.rs`).
 *
 * Everything is in BODY axes (x forward, y right, z down) from the aircraft's reference point. Each
 * vertex carries the index of the part it belongs to (0 for the fixed airframe, 1 and up for the
 * parts that move) and a material; the vertex shader moves it with that part's matrix, which
 * `ModelRig` writes every frame from the snapshot (control surfaces, flaps, airbrake, gear, glow)
 * without allocating.
 *
 * Triangles are wound counter-clockwise seen from outside, so back-face culling keeps the outside;
 * every primitive is oriented against an outward hint, so no winding rule is trusted by hand.
 */
import type {
  BoxSpec,
  CockpitSpec,
  GearLegSpec,
  ModelChannel,
  ModelSpec,
  NozzleSpec,
  PartSpec,
  StationSpec,
  SurfaceSpec,
} from './present.ts';

/** Floats per vertex: position (3), normal (3), part (1), material (1). */
export const MESH_VERTEX_FLOATS = 8;
export const MESH_VERTEX_BYTES = MESH_VERTEX_FLOATS * 4;

/** Most parts a model may have (the part matrices are one uniform block with the fixed airframe). */
export const MAX_PARTS = 15;
/** Part matrices in the uniform block: the fixed airframe and the parts. */
export const MODEL_MATRICES = MAX_PARTS + 1;
/** Floats in the model uniform block: the matrices and a vec4 of glow. */
export const MODEL_UNIFORM_FLOATS = MODEL_MATRICES * 16 + 4;
export const MODEL_UNIFORM_BYTES = MODEL_UNIFORM_FLOATS * 4;

/** Materials, by the number the shader switches on. */
export const MATERIAL = {
  PAINT: 0,
  DARK: 1,
  GLASS: 2,
  GLOW: 3,
  TYRE: 4,
  METAL: 5,
  COCKPIT: 6,
} as const;

/** The most triangles an exterior may have (GDD 62 budget for the procedural Kestrel). */
export const MAX_MODEL_TRIANGLES = 8000;

// The builder's rules; `present_check.rs` mirrors them.
export const FUSELAGE_SIDES = 16;
export const CANOPY_SIDES = 12;
export const NOZZLE_SIDES = 12;
export const WHEEL_SIDES = 12;

export interface ModelMesh {
  /** Interleaved `MESH_VERTEX_FLOATS` per vertex. */
  vertices: Float32Array;
  indices: Uint16Array;
  vertexCount: number;
  triangleCount: number;
  /**
   * The indices to draw without the canopy: the canopy's triangles come last, so this is a prefix
   * (the same as `indices.length` for a mesh with no canopy).
   */
  bodyIndexCount: number;
}

type V3 = readonly [number, number, number];

const sub = (a: V3, b: V3): V3 => [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
const add = (a: V3, b: V3): V3 => [a[0] + b[0], a[1] + b[1], a[2] + b[2]];
const scale = (a: V3, s: number): V3 => [a[0] * s, a[1] * s, a[2] * s];
const dot = (a: V3, b: V3): number => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
const cross = (a: V3, b: V3): V3 => [
  a[1] * b[2] - a[2] * b[1],
  a[2] * b[0] - a[0] * b[2],
  a[0] * b[1] - a[1] * b[0],
];
const norm = (a: V3): V3 => {
  const l = Math.hypot(a[0], a[1], a[2]);
  return l === 0 ? [0, 0, 0] : [a[0] / l, a[1] / l, a[2] / l];
};
const mid = (a: V3, b: V3): V3 => scale(add(a, b), 0.5);

class Builder {
  readonly vertices: number[] = [];
  readonly indices: number[] = [];

  get vertexCount(): number {
    return this.vertices.length / MESH_VERTEX_FLOATS;
  }

  vertex(p: V3, n: V3, part: number, material: number): number {
    this.vertices.push(p[0], p[1], p[2], n[0], n[1], n[2], part, material);
    return this.vertexCount - 1;
  }

  position(i: number): V3 {
    const at = i * MESH_VERTEX_FLOATS;
    return [this.vertices[at] ?? 0, this.vertices[at + 1] ?? 0, this.vertices[at + 2] ?? 0];
  }

  /** A triangle of existing vertices, wound so its normal points along `outward`. */
  triangle(a: number, b: number, c: number, outward: V3): void {
    const n = cross(
      sub(this.position(b), this.position(a)),
      sub(this.position(c), this.position(a)),
    );
    if (dot(n, outward) >= 0) this.indices.push(a, b, c);
    else this.indices.push(a, c, b);
  }

  /** A flat quad (four corners in order round it): two triangles with their own vertices. */
  quad(p: readonly [V3, V3, V3, V3], outward: V3, part: number, material: number): void {
    const n = norm(cross(sub(p[1], p[0]), sub(p[2], p[0])));
    const flat: V3 = dot(n, outward) >= 0 ? n : scale(n, -1);
    const [a, b, c, d] = p.map((q) => this.vertex(q, flat, part, material)) as [
      number,
      number,
      number,
      number,
    ];
    this.triangle(a, b, c, flat);
    this.triangle(a, c, d, flat);
  }

  /** A closed convex hexahedron from eight corners: bottom face (0-3) and top face (4-7). */
  hexahedron(c: readonly V3[], part: number, material: number): void {
    const centre = scale(
      c.reduce<V3>((s, q) => add(s, q), [0, 0, 0]),
      1 / c.length,
    );
    const face = (i: number, j: number, k: number, l: number): void => {
      const corners = [c[i], c[j], c[k], c[l]] as [V3, V3, V3, V3];
      const faceCentre = scale(add(add(corners[0], corners[1]), add(corners[2], corners[3])), 0.25);
      this.quad(corners, sub(faceCentre, centre), part, material);
    };
    face(0, 1, 2, 3);
    face(4, 5, 6, 7);
    face(0, 1, 5, 4);
    face(1, 2, 6, 5);
    face(2, 3, 7, 6);
    face(3, 0, 4, 7);
  }

  build(canopyFrom: number): ModelMesh {
    return {
      vertices: Float32Array.from(this.vertices),
      indices: Uint16Array.from(this.indices),
      vertexCount: this.vertexCount,
      triangleCount: this.indices.length / 3,
      bodyIndexCount: Math.min(canopyFrom, this.indices.length),
    };
  }
}

/** The section of a station as a ring of `sides` points. */
function sectionPoint(s: StationSpec, sides: number, k: number): V3 {
  const phi = (2 * Math.PI * k) / sides;
  const centreZ = (s.topZM + s.bottomZM) / 2;
  const halfHeight = (s.bottomZM - s.topZM) / 2;
  return [s.xM, s.halfWidthM * Math.cos(phi), centreZ + halfHeight * Math.sin(phi)];
}

/**
 * A closed solid from rings at stations, nose to tail: smooth normals round it, and a fan of
 * `sides` triangles closes each end. `2 * sides * (stations - 1) + 2 * sides` triangles.
 */
function ringSolid(
  b: Builder,
  stations: readonly StationSpec[],
  sides: number,
  part: number,
  material: number,
): void {
  if (stations.length < 2) return;
  const rows = stations.length;
  const point = (i: number, k: number): V3 =>
    sectionPoint(stations[i] as StationSpec, sides, ((k % sides) + sides) % sides);
  // Smooth normals from the surface tangents: along the body, and round it.
  const first = b.vertexCount;
  for (let i = 0; i < rows; i++) {
    for (let k = 0; k < sides; k++) {
      const along = sub(point(Math.min(i + 1, rows - 1), k), point(Math.max(i - 1, 0), k));
      const round = sub(point(i, k + 1), point(i, k - 1));
      // Outward is away from the section's centre, in y and z.
      const s = stations[i] as StationSpec;
      const centre: V3 = [s.xM, 0, (s.topZM + s.bottomZM) / 2];
      const p = point(i, k);
      const outward = sub(p, centre);
      let n = norm(cross(along, round));
      if (dot(n, outward) < 0) n = scale(n, -1);
      b.vertex(p, n, part, material);
    }
  }
  const at = (i: number, k: number): number => first + i * sides + (((k % sides) + sides) % sides);
  for (let i = 0; i < rows - 1; i++) {
    for (let k = 0; k < sides; k++) {
      const [a, c, d, e] = [at(i, k), at(i, k + 1), at(i + 1, k + 1), at(i + 1, k)] as const;
      const outward = sub(mid(b.position(a), b.position(d)), [
        (stations[i] as StationSpec).xM / 2 + (stations[i + 1] as StationSpec).xM / 2,
        0,
        ((stations[i] as StationSpec).topZM + (stations[i] as StationSpec).bottomZM) / 4 +
          ((stations[i + 1] as StationSpec).topZM + (stations[i + 1] as StationSpec).bottomZM) / 4,
      ]);
      b.triangle(a, c, d, outward);
      b.triangle(a, d, e, outward);
    }
  }
  // The ends: a fan round a centre vertex, facing along x (outward at the nose, aft at the tail).
  const cap = (i: number, outward: V3): void => {
    const s = stations[i] as StationSpec;
    const centre = b.vertex([s.xM, 0, (s.topZM + s.bottomZM) / 2], outward, part, material);
    for (let k = 0; k < sides; k++) {
      // Caps have their own rim vertices so the flat shading does not smear into the sides.
      const rimA = b.vertex(point(i, k), outward, part, material);
      const rimB = b.vertex(point(i, k + 1), outward, part, material);
      b.triangle(centre, rimA, rimB, outward);
    }
  };
  cap(0, [1, 0, 0]);
  cap(rows - 1, [-1, 0, 0]);
}

/** A slab: a trapezoid planform with a thickness (twelve triangles), optionally with its twin. */
function slab(b: Builder, s: SurfaceSpec, part: number, material: number): void {
  const make = (flip: boolean): void => {
    const y = (v: number): number => (flip ? -v : v);
    const corner = (
      le: V3,
      chord: number,
      thickness: number,
      side: 1 | -1,
      edge: 'le' | 'te',
    ): V3 => [edge === 'le' ? le[0] : le[0] - chord, y(le[1]), le[2] + (side * thickness) / 2];
    // Bottom face (z down: +z side) corners round the root leading edge, root trailing edge, tip
    // trailing edge, tip leading edge; top face the same with -z.
    const root = s.rootLeM;
    const tip = s.tipLeM;
    const c: V3[] = [
      corner(root, s.rootChordM, s.rootThicknessM, 1, 'le'),
      corner(root, s.rootChordM, s.rootThicknessM, 1, 'te'),
      corner(tip, s.tipChordM, s.tipThicknessM, 1, 'te'),
      corner(tip, s.tipChordM, s.tipThicknessM, 1, 'le'),
      corner(root, s.rootChordM, s.rootThicknessM, -1, 'le'),
      corner(root, s.rootChordM, s.rootThicknessM, -1, 'te'),
      corner(tip, s.tipChordM, s.tipThicknessM, -1, 'te'),
      corner(tip, s.tipChordM, s.tipThicknessM, -1, 'le'),
    ];
    b.hexahedron(c, part, material);
  };
  make(false);
  if (s.mirror) make(true);
}

/** An axis-aligned box (twelve triangles). */
function box(b: Builder, spec: BoxSpec, part: number, material: number): void {
  const [x0, y0, z0] = spec.minM;
  const [x1, y1, z1] = spec.maxM;
  b.hexahedron(
    [
      [x0, y0, z1],
      [x1, y0, z1],
      [x1, y1, z1],
      [x0, y1, z1],
      [x0, y0, z0],
      [x1, y0, z0],
      [x1, y1, z0],
      [x0, y1, z0],
    ],
    part,
    material,
  );
}

/** A short cone from the nozzle's front, its rear face the glow (`3 * NOZZLE_SIDES` triangles). */
function nozzle(b: Builder, n: NozzleSpec, part: number): void {
  const [x, y, z] = n.atM;
  const rear = x - n.lengthM;
  const rRear = n.radiusM * 0.85;
  const ringAt = (px: number, r: number, k: number): V3 => {
    const phi = (2 * Math.PI * (k % NOZZLE_SIDES)) / NOZZLE_SIDES;
    return [px, y + r * Math.cos(phi), z + r * Math.sin(phi)];
  };
  for (let k = 0; k < NOZZLE_SIDES; k++) {
    const a = ringAt(x, n.radiusM, k);
    const c = ringAt(x, n.radiusM, k + 1);
    const d = ringAt(rear, rRear, k + 1);
    const e = ringAt(rear, rRear, k);
    const outward = sub(mid(a, d), [mid(a, d)[0], y, z]);
    const flat = norm(cross(sub(c, a), sub(d, a)));
    const face: V3 = dot(flat, outward) >= 0 ? flat : scale(flat, -1);
    const [ia, ic, id, ie] = [a, c, d, e].map((p) => b.vertex(p, face, part, MATERIAL.DARK)) as [
      number,
      number,
      number,
      number,
    ];
    b.triangle(ia, ic, id, outward);
    b.triangle(ia, id, ie, outward);
  }
  // The glowing rear face.
  const centre = b.vertex([rear, y, z], [-1, 0, 0], part, MATERIAL.GLOW);
  for (let k = 0; k < NOZZLE_SIDES; k++) {
    const p = b.vertex(ringAt(rear, rRear, k), [-1, 0, 0], part, MATERIAL.GLOW);
    const q = b.vertex(ringAt(rear, rRear, k + 1), [-1, 0, 0], part, MATERIAL.GLOW);
    b.triangle(centre, p, q, [-1, 0, 0]);
  }
}

/** A gear leg: a box strut from the leg's top to the wheel, and a twelve-sided wheel (60 triangles). */
function gearLeg(b: Builder, g: GearLegSpec, part: number): void {
  const strutHalf = 0.05;
  const hubZ = g.contactM[2] - g.wheelRadiusM;
  box(
    b,
    {
      id: g.id,
      minM: [
        g.topM[0] - strutHalf,
        Math.min(g.topM[1], g.contactM[1]) - strutHalf,
        Math.min(g.topM[2], hubZ),
      ],
      maxM: [
        g.topM[0] + strutHalf,
        Math.max(g.topM[1], g.contactM[1]) + strutHalf,
        Math.max(g.topM[2], hubZ),
      ],
    },
    part,
    MATERIAL.METAL,
  );
  // The wheel's axis is along y.
  const [cx, cy] = [g.contactM[0], g.contactM[1]];
  const ringAt = (side: 1 | -1, k: number): V3 => {
    const phi = (2 * Math.PI * (k % WHEEL_SIDES)) / WHEEL_SIDES;
    return [
      cx + g.wheelRadiusM * Math.cos(phi),
      cy + (side * g.wheelWidthM) / 2,
      hubZ + g.wheelRadiusM * Math.sin(phi),
    ];
  };
  for (let k = 0; k < WHEEL_SIDES; k++) {
    const a = ringAt(-1, k);
    const c = ringAt(-1, k + 1);
    const d = ringAt(1, k + 1);
    const e = ringAt(1, k);
    const outward = sub(mid(a, d), [cx, cy, hubZ]);
    b.quad([a, c, d, e], [outward[0], 0, outward[2]], part, MATERIAL.TYRE);
  }
  for (const side of [-1, 1] as const) {
    const out: V3 = [0, side, 0];
    const hub = b.vertex([cx, cy + (side * g.wheelWidthM) / 2, hubZ], out, part, MATERIAL.METAL);
    for (let k = 0; k < WHEEL_SIDES; k++) {
      const p = b.vertex(ringAt(side, k), out, part, MATERIAL.METAL);
      const q = b.vertex(ringAt(side, k + 1), out, part, MATERIAL.METAL);
      b.triangle(hub, p, q, out);
    }
  }
}

/** The index of each part in the model: 1 and up (0 is the fixed airframe). */
export function partIndices(parts: readonly PartSpec[]): Map<string, number> {
  const out = new Map<string, number>();
  parts.forEach((p, i) => out.set(p.id, i + 1));
  return out;
}

/**
 * The exterior of an aircraft. The canopy comes last in the index buffer so a cockpit view can draw
 * the rest and look out through where it is.
 *
 * @throws RangeError if the model has more than `MAX_PARTS` parts or more than the triangle budget.
 */
export function buildModelMesh(spec: ModelSpec): ModelMesh {
  if (spec.parts.length > MAX_PARTS) {
    throw new RangeError(
      `SYS-040: the model has ${String(spec.parts.length)} parts; at most ${String(MAX_PARTS)}`,
    );
  }
  const part = partIndices(spec.parts);
  const index = (name: string | undefined): number =>
    name === undefined ? 0 : (part.get(name) ?? 0);
  const b = new Builder();
  ringSolid(b, spec.fuselage, FUSELAGE_SIDES, 0, MATERIAL.PAINT);
  for (const s of spec.surfaces) slab(b, s, index(s.part), MATERIAL.PAINT);
  for (const x of spec.boxes)
    box(b, x, index(x.part), x.part === undefined ? MATERIAL.DARK : MATERIAL.PAINT);
  for (const g of spec.gear) gearLeg(b, g, index(g.part));
  nozzle(b, spec.nozzle, index(spec.nozzle.part));
  const canopyFrom = b.indices.length;
  ringSolid(b, spec.canopy, CANOPY_SIDES, 0, MATERIAL.GLASS);
  const mesh = b.build(canopyFrom);
  if (mesh.triangleCount > MAX_MODEL_TRIANGLES) {
    throw new RangeError(
      `SYS-040: the model has ${String(mesh.triangleCount)} triangles; the budget is ${String(MAX_MODEL_TRIANGLES)}`,
    );
  }
  if (mesh.vertexCount > 65_535) throw new RangeError('SYS-040: the model needs 32-bit indices');
  return mesh;
}

/** Segments in each canopy bow's arch. */
export const BOW_SEGMENTS = 12;

/**
 * The inside of the cockpit: the canopy bows (arches over the pilot), the glare shield, the panel
 * and the side consoles. All fixed to the airframe (part 0), drawn only in the cockpit view.
 */
export function buildCockpitMesh(spec: CockpitSpec): ModelMesh {
  const b = new Builder();
  for (const bow of spec.canopyBows) {
    // An arch in the y-z plane from one sill over the top to the other: z at the sill, rising to
    // `topZM` (more negative is higher).
    const point = (k: number, dx: number, dz: number): V3 => {
      const phi = (Math.PI * k) / BOW_SEGMENTS;
      return [
        bow.xM + dx,
        bow.halfWidthM * Math.cos(phi),
        bow.sillZM + (bow.topZM - bow.sillZM) * Math.sin(phi) + dz,
      ];
    };
    const t = bow.thicknessM;
    for (let k = 0; k < BOW_SEGMENTS; k++) {
      // Each segment is a small beam: the arch's inner face, outer face, and the two sides.
      const inner = (kk: number, dx: number): V3 => point(kk, dx, 0);
      const outer = (kk: number, dx: number): V3 => point(kk, dx, -t);
      const front = bow.xM + t / 2 - bow.xM;
      const back = -t / 2;
      const centre: V3 = mid(inner(k, 0), outer(k + 1, 0));
      const corners: V3[] = [
        inner(k, front),
        inner(k + 1, front),
        inner(k + 1, back),
        inner(k, back),
        outer(k, front),
        outer(k + 1, front),
        outer(k + 1, back),
        outer(k, back),
      ];
      const faceCentre = (i: readonly number[]): V3 =>
        scale(
          i.reduce<V3>((s, j) => add(s, corners[j] as V3), [0, 0, 0]),
          1 / i.length,
        );
      for (const idx of [
        [0, 1, 2, 3],
        [4, 5, 6, 7],
        [0, 1, 5, 4],
        [2, 3, 7, 6],
      ] as const) {
        b.quad(
          idx.map((j) => corners[j]) as unknown as [V3, V3, V3, V3],
          sub(faceCentre(idx), centre),
          0,
          MATERIAL.COCKPIT,
        );
      }
    }
  }
  box(b, spec.glareShield, 0, MATERIAL.COCKPIT);
  box(b, spec.panel, 0, MATERIAL.COCKPIT);
  for (const c of spec.consoles) box(b, c, 0, MATERIAL.COCKPIT);
  const mesh = b.build(b.indices.length);
  if (mesh.vertexCount > 65_535) throw new RangeError('SYS-040: the cockpit needs 32-bit indices');
  return mesh;
}

/** Triangles of the exterior by the builder's rules (the baker's `model_triangles`). */
export function modelTriangleBudget(spec: ModelSpec): number {
  const ring = (sides: number, stations: number): number =>
    2 * sides * Math.max(stations - 1, 0) + 2 * sides;
  const slabs = spec.surfaces.reduce((n, s) => n + 12 * (s.mirror ? 2 : 1), 0);
  return (
    ring(FUSELAGE_SIDES, spec.fuselage.length) +
    ring(CANOPY_SIDES, spec.canopy.length) +
    3 * NOZZLE_SIDES +
    slabs +
    12 * spec.boxes.length +
    60 * spec.gear.length
  );
}

export type { ModelChannel };
