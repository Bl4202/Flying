import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { parsePresentBundle } from '@flying/sim-bridge/present';
import {
  buildModelMesh,
  MESH_VERTEX_FLOATS,
  MODEL_MATRICES,
  MODEL_UNIFORM_FLOATS,
  partIndices,
} from './model-mesh.ts';
import { ModelRig, newModelUniform, type ModelState } from './model-rig.ts';
import { renderPresentation, type StationSpec } from './present.ts';
import { NORTH } from './snapshot-fixture.ts';

const bundle = parsePresentBundle(
  JSON.parse(
    readFileSync(new URL('../../../tests/vectors/present_bundle.json', import.meta.url), 'utf8'),
  ),
);
const presentation = renderPresentation(bundle, 'core:ac/kestrel');
if (!presentation) throw new Error('no Kestrel');
const { model } = presentation;
const mesh = buildModelMesh(model);
const rig = new ModelRig(model);
const index = partIndices(model.parts);
const DEG = Math.PI / 180;

type V3 = [number, number, number];

/** A state: heading north, wings level, at the BF origin, with the eye there too. */
function state(over: Partial<ModelState> = {}): ModelState {
  return {
    ownshipPosBf: [0, 0, 0],
    ownshipQuat: NORTH,
    elevatorRad: 0,
    aileronRad: 0,
    rudderRad: 0,
    flapRad: 0,
    airbrake: 0,
    gearPos: 1,
    abLevel: 0,
    ...over,
  };
}

function matrixFor(s: ModelState, eyeBf: ArrayLike<number> = [0, 0, 0]): Float32Array {
  const out = newModelUniform();
  rig.update(out, s, eyeBf);
  return out;
}

/** A body-axes point of a part, moved by that part's matrix into eye-relative render axes. */
function moved(out: Float32Array, partId: string | undefined, p: V3): V3 {
  const at = (partId === undefined ? 0 : (index.get(partId) ?? 0)) * 16;
  const m = (k: number): number => out[at + k] ?? 0;
  return [
    m(0) * p[0] + m(4) * p[1] + m(8) * p[2] + m(12),
    m(1) * p[0] + m(5) * p[1] + m(9) * p[2] + m(13),
    m(2) * p[0] + m(6) * p[1] + m(10) * p[2] + m(14),
  ];
}

/** The mesh vertex of `part` farthest back (the trailing edge), at the outermost span. */
function trailingEdge(part: string): V3 {
  const want = index.get(part);
  let best: V3 | undefined;
  for (let i = 0; i < mesh.vertexCount; i++) {
    if (mesh.vertices[i * MESH_VERTEX_FLOATS + 6] !== want) continue;
    const p: V3 = [
      mesh.vertices[i * MESH_VERTEX_FLOATS] ?? 0,
      mesh.vertices[i * MESH_VERTEX_FLOATS + 1] ?? 0,
      mesh.vertices[i * MESH_VERTEX_FLOATS + 2] ?? 0,
    ];
    if (
      best === undefined ||
      p[0] < best[0] - 1e-9 ||
      (Math.abs(p[0] - best[0]) < 1e-9 && Math.abs(p[1]) > Math.abs(best[1]))
    ) {
      best = p;
    }
  }
  if (!best) throw new Error(`no vertices for ${part}`);
  return best;
}

describe('the model uniform block', () => {
  it('holds sixteen matrices and the glow level', () => {
    expect(MODEL_MATRICES).toBe(16);
    expect(MODEL_UNIFORM_FLOATS).toBe(16 * 16 + 4);
    expect(model.parts.length).toBe(12);
    expect(rig.partCount).toBe(12);
  });

  it('puts the airframe in render axes: heading north, forward is -z, right is +x, down is -y', () => {
    const out = matrixFor(state());
    const forward = moved(out, undefined, [1, 0, 0]);
    const right = moved(out, undefined, [0, 1, 0]);
    const down = moved(out, undefined, [0, 0, 1]);
    expect(forward.map((v) => Math.round(v) + 0)).toEqual([0, 0, -1]);
    expect(right.map((v) => Math.round(v) + 0)).toEqual([1, 0, 0]);
    expect(down.map((v) => Math.round(v) + 0)).toEqual([0, -1, 0]);
  });

  it('places the airframe relative to the eye, subtracting in f64 before rounding', () => {
    const out = matrixFor(
      state({ ownshipPosBf: [400_000.013, -399_999.987, 2000] }),
      [400_000, -400_000, 1998],
    );
    // BF delta (east, north, up) = (0.013, 0.013, 2) -> render (east, up, south) = (0.013, 2, -0.013).
    expect(out[12]).toBeCloseTo(0.013, 5);
    expect(out[13]).toBeCloseTo(2, 6);
    expect(out[14]).toBeCloseTo(-0.013, 5);
  });

  it('writes the afterburner level into the glow slot', () => {
    const out = matrixFor(state({ abLevel: 0.75 }));
    expect(out[MODEL_MATRICES * 16]).toBeCloseTo(0.75, 6);
  });

  it('leaves the parts where they are with every control at rest', () => {
    const rest = matrixFor(state());
    for (const part of model.parts) {
      const at = (index.get(part.id) ?? 0) * 16;
      for (let k = 0; k < 16; k++) {
        // The gear is down (position 1 = retract 0), so it is at rest too.
        expect(rest[at + k], `${part.id} ${String(k)}`).toBeCloseTo(rest[k] ?? 0, 6);
      }
    }
  });
});

describe('the parts follow the sim, with the signs the sim uses', () => {
  // Render y is up: a trailing edge that goes down has a smaller y.
  it('elevator: positive is trailing edge down (nose down), both stabilators', () => {
    const [r, l] = ['stabilator_r', 'stabilator_l'].map((id) => {
      const te = trailingEdge(id);
      const rest = moved(matrixFor(state()), id, te);
      const down = moved(matrixFor(state({ elevatorRad: 10 * DEG })), id, te);
      return { rest, down };
    }) as [{ rest: V3; down: V3 }, { rest: V3; down: V3 }];
    for (const s of [r, l]) {
      expect(s.down[1]).toBeLessThan(s.rest[1] - 0.05);
    }
  });

  it('elevator: negative is trailing edge up', () => {
    const te = trailingEdge('stabilator_r');
    const rest = moved(matrixFor(state()), 'stabilator_r', te);
    const up = moved(matrixFor(state({ elevatorRad: -10 * DEG })), 'stabilator_r', te);
    expect(up[1]).toBeGreaterThan(rest[1] + 0.05);
  });

  it('aileron: positive rolls the right wing down: the right trailing edge goes up, the left down', () => {
    const [r, l] = ['aileron_r', 'aileron_l'].map((id) => {
      const te = trailingEdge(id);
      return {
        rest: moved(matrixFor(state()), id, te),
        moved: moved(matrixFor(state({ aileronRad: 10 * DEG })), id, te),
      };
    }) as [{ rest: V3; moved: V3 }, { rest: V3; moved: V3 }];
    expect(r.moved[1]).toBeGreaterThan(r.rest[1] + 0.02);
    expect(l.moved[1]).toBeLessThan(l.rest[1] - 0.02);
  });

  it('rudder: positive is trailing edge left (render -x)', () => {
    const te = trailingEdge('rudder');
    const rest = moved(matrixFor(state()), 'rudder', te);
    const left = moved(matrixFor(state({ rudderRad: 10 * DEG })), 'rudder', te);
    expect(left[0]).toBeLessThan(rest[0] - 0.05);
  });

  it('flaps: positive is down, both sides', () => {
    for (const id of ['flap_r', 'flap_l']) {
      const te = trailingEdge(id);
      const rest = moved(matrixFor(state()), id, te);
      const down = moved(matrixFor(state({ flapRad: 20 * DEG })), id, te);
      expect(down[1], id).toBeLessThan(rest[1] - 0.05);
    }
  });

  it('airbrake: opens upward at the rear edge', () => {
    const te = trailingEdge('airbrake');
    const rest = moved(matrixFor(state()), 'airbrake', te);
    const open = moved(matrixFor(state({ airbrake: 1 })), 'airbrake', te);
    expect(open[1]).toBeGreaterThan(rest[1] + 0.3);
  });

  it("turns a surface by the channel times the data's degrees per unit, about its hinge", () => {
    const part = model.parts.find((p) => p.id === 'stabilator_r');
    if (part?.motion !== 'rotate') throw new Error('not a rotating part');
    // A point on the hinge does not move.
    const hinge = part.pivotM;
    const out = matrixFor(state({ elevatorRad: 25 * DEG }));
    const at = moved(out, 'stabilator_r', [hinge[0], hinge[1], hinge[2]]);
    const rest = moved(matrixFor(state()), 'stabilator_r', [hinge[0], hinge[1], hinge[2]]);
    for (let k = 0; k < 3; k++) expect(at[k]).toBeCloseTo(rest[k] ?? 0, 5);
    // A point 1 m behind the hinge swings through 25 degrees: it drops sin(25) metres.
    const behind = moved(out, 'stabilator_r', [hinge[0] - 1, hinge[1], hinge[2]]);
    const behindRest = moved(matrixFor(state()), 'stabilator_r', [
      hinge[0] - 1,
      hinge[1],
      hinge[2],
    ]);
    expect(behindRest[1] - behind[1]).toBeCloseTo(Math.sin(25 * DEG), 5);
  });

  describe('the gear', () => {
    const fuselage = (x: number): { half: number; top: number; bottom: number } | undefined => {
      const st: readonly StationSpec[] = model.fuselage;
      for (let i = 0; i + 1 < st.length; i++) {
        const [a, b] = [st[i] as StationSpec, st[i + 1] as StationSpec];
        if (x <= a.xM && x >= b.xM) {
          const t = (a.xM - x) / (a.xM - b.xM);
          return {
            half: a.halfWidthM + (b.halfWidthM - a.halfWidthM) * t,
            top: a.topZM + (b.topZM - a.topZM) * t,
            bottom: a.bottomZM + (b.bottomZM - a.bottomZM) * t,
          };
        }
      }
      return undefined;
    };

    /** The wheel's hub in body axes, with the gear at `gearPos`, in render axes made body again. */
    function hubWhenRetracted(id: string): V3 {
      const leg = model.gear.find((g) => g.id === id);
      if (!leg) throw new Error(id);
      const hub: V3 = [leg.contactM[0], leg.contactM[1], leg.contactM[2] - leg.wheelRadiusM];
      const out = matrixFor(state({ gearPos: 0 }));
      const r = moved(out, leg.part, hub);
      // Render axes back to body axes for a north-heading, level aircraft at the origin:
      // body (x, y, z) -> render (y, -z, -x).
      return [-r[2], r[0], -r[1]];
    }

    it('leaves the wheels on the ground with the gear down, where the aircraft data says', () => {
      for (const leg of model.gear) {
        const out = matrixFor(state({ gearPos: 1 }));
        const c = moved(out, leg.part, [...leg.contactM] as V3);
        // Render axes back to body axes: body (x, y, z) -> render (y, -z, -x).
        expect(-c[2]).toBeCloseTo(leg.contactM[0], 5);
        expect(c[0]).toBeCloseTo(leg.contactM[1], 5);
        expect(-c[1]).toBeCloseTo(leg.contactM[2], 5);
      }
    });

    it('draws the wheels inside the fuselage when they are retracted', () => {
      for (const id of ['nose', 'main_l', 'main_r']) {
        const hub = hubWhenRetracted(id);
        const f = fuselage(hub[0]);
        expect(f, `${id} at x ${hub[0].toFixed(2)}`).toBeDefined();
        expect(Math.abs(hub[1]), `${id} y`).toBeLessThanOrEqual((f?.half ?? 0) - 0.05);
        expect(hub[2], `${id} z (top)`).toBeGreaterThanOrEqual((f?.top ?? 0) + 0.05);
        expect(hub[2], `${id} z (bottom)`).toBeLessThanOrEqual((f?.bottom ?? 0) - 0.05);
      }
    });

    it('moves the gear part by part between down and up', () => {
      const leg = model.gear[0];
      if (!leg) throw new Error('no gear');
      const half = matrixFor(state({ gearPos: 0.5 }));
      const down = matrixFor(state({ gearPos: 1 }));
      const a = moved(half, leg.part, [...leg.contactM] as V3);
      const b = moved(down, leg.part, [...leg.contactM] as V3);
      expect(Math.hypot(a[0] - b[0], a[1] - b[1], a[2] - b[2])).toBeGreaterThan(0.5);
    });
  });
});
