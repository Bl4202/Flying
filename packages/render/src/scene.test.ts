import { describe, expect, it } from 'vitest';
import { FRAME_UNIFORM_BYTES, FRAME_UNIFORM_FLOATS, SceneBuilder } from './scene.ts';
import { SnapshotInterpolator } from './interp.ts';
import { MARKER_INSTANCE_FLOATS, MARKER_SIZE_M } from './markers.ts';
import { makeSnapshot, NORTH } from './snapshot-fixture.ts';

/** Element `i` of `a`, or NaN (which fails any comparison) if there is none. */
const at = (a: ArrayLike<number>, i: number): number => a[i] ?? Number.NaN;

function world(extra: Parameters<typeof makeSnapshot>[0]) {
  const i = new SnapshotInterpolator();
  i.push(makeSnapshot({ publishMs: 1000, ...extra }));
  const state = i.sample(2000);
  if (!state) throw new Error('no state');
  return state;
}

describe('SceneBuilder', () => {
  it('has the uniform size the shaders declare: 112 bytes', () => {
    expect(FRAME_UNIFORM_FLOATS).toBe(28);
    expect(FRAME_UNIFORM_BYTES).toBe(112);
    expect(
      new SceneBuilder().build(world({}), {
        aspect: 1,
        zRange: '0to1',
        timeS: 0,
        patchOriginBf: null,
        spacingM: 250,
      }).uniform,
    ).toHaveLength(28);
  });

  it('writes time and spacing into params, and a unit upward sun', () => {
    const u = new SceneBuilder().build(world({}), {
      aspect: 1.5,
      zRange: '0to1',
      timeS: 12.5,
      patchOriginBf: null,
      spacingM: 250,
    }).uniform;
    expect([u[20], u[21]]).toEqual([12.5, 250]);
    expect(Math.hypot(u[16] ?? 0, u[17] ?? 0, u[18] ?? 0)).toBeCloseTo(1, 6);
    expect(u[17]).toBeGreaterThan(0);
  });

  it('gives the terrain patch offset relative to the eye in render axes, subtracted in f64', () => {
    const eye: [number, number, number] = [400_000.013, -399_999.987, 2000];
    const u = new SceneBuilder().build(world({ observer: { pos: eye } }), {
      aspect: 1,
      zRange: '0to1',
      timeS: 0,
      patchOriginBf: [400_000, -400_000, 0],
      spacingM: 250,
    }).uniform;
    // BF delta (east, north, up) = (-0.013, -0.013, -2000) -> render (east, up, south).
    expect(u[24]).toBeCloseTo(-0.013, 5);
    expect(u[25]).toBeCloseTo(-2000, 9);
    expect(u[26]).toBeCloseTo(0.013, 5);
  });

  it('puts a point straight ahead of the observer at the centre of the screen', () => {
    const scene = new SceneBuilder();
    const state = world({ observer: { pos: [0, -6000, 1000], quat: NORTH } });
    const f = scene.build(state, {
      aspect: 16 / 9,
      zRange: '0to1',
      timeS: 0,
      patchOriginBf: null,
      spacingM: 250,
    });
    const vp = f.uniform;
    // 3 km straight ahead (north) and level: render (0, 0, -3000) relative to the eye.
    const p = [0, 0, -3000];
    const clip = [0, 1, 2, 3].map(
      (row) =>
        (vp[row] ?? 0) * at(p, 0) +
        (vp[4 + row] ?? 0) * at(p, 1) +
        (vp[8 + row] ?? 0) * at(p, 2) +
        (vp[12 + row] ?? 0),
    );
    expect((clip[0] ?? 1) / (clip[3] ?? 1)).toBeCloseTo(0, 6);
    expect((clip[1] ?? 1) / (clip[3] ?? 1)).toBeCloseTo(0, 6);
  });

  it('fills one instance per body: eye-relative position, size, and an orientation taking body to render axes', () => {
    const state = world({
      observer: { pos: [0, 0, 1000] },
      bodies: [
        { pos: [100, 2000, 1500], quat: NORTH },
        { pos: [-50, 500, 900], quat: NORTH },
      ],
    });
    const f = new SceneBuilder().build(state, {
      aspect: 1,
      zRange: '0to1',
      timeS: 0,
      patchOriginBf: null,
      spacingM: 250,
    });
    expect(f.instanceCount).toBe(2);
    const inst = (i: number): number[] =>
      Array.from(
        f.instances.subarray(i * MARKER_INSTANCE_FLOATS, (i + 1) * MARKER_INSTANCE_FLOATS),
      );
    // body 0: BF delta (100, 2000, 500) -> render (100, 500, -2000)
    expect(inst(0).slice(0, 4)).toEqual([100, 500, -2000, MARKER_SIZE_M]);
    expect(inst(1).slice(0, 4)).toEqual([-50, -100, -500, MARKER_SIZE_M]);

    // A body heading north has its nose (body +x) pointing along render -z (north).
    const [qx, qy, qz, qw] = inst(0).slice(4) as [number, number, number, number];
    const t = [2 * (qy * 0 - qz * 0), 2 * (qz * 1 - qx * 0), 2 * (qx * 0 - qy * 1)]; // 2 q.xyz x (1,0,0)
    const nose = [
      1 + qw * at(t, 0) + (qy * at(t, 2) - qz * at(t, 1)),
      qw * at(t, 1) + (qz * at(t, 0) - qx * at(t, 2)),
      qw * at(t, 2) + (qx * at(t, 1) - qy * at(t, 0)),
    ];
    expect(nose[0]).toBeCloseTo(0, 6);
    expect(nose[1]).toBeCloseTo(0, 6);
    expect(nose[2]).toBeCloseTo(-1, 6);
    expect(Math.hypot(qx, qy, qz, qw)).toBeCloseTo(1, 6);
  });

  it('reuses its buffers frame to frame (no allocation per frame) and caps at the body capacity', () => {
    const scene = new SceneBuilder();
    const state = world({
      bodies: Array.from({ length: 40 }, (_, i) => ({
        pos: [i, i, i] as [number, number, number],
      })),
    });
    const a = scene.build(state, {
      aspect: 1,
      zRange: '0to1',
      timeS: 0,
      patchOriginBf: null,
      spacingM: 250,
    });
    const b = scene.build(state, {
      aspect: 1,
      zRange: '0to1',
      timeS: 1,
      patchOriginBf: null,
      spacingM: 250,
    });
    expect(a).toBe(b);
    expect(a.uniform).toBe(b.uniform);
    expect(a.instanceCount).toBe(40);
  });

  it('can build for either depth range', () => {
    const scene = new SceneBuilder();
    const state = world({});
    const zero = Float32Array.from(
      scene.build(state, {
        aspect: 1,
        zRange: '0to1',
        timeS: 0,
        patchOriginBf: null,
        spacingM: 250,
      }).uniform,
    );
    const neg = Float32Array.from(
      scene.build(state, {
        aspect: 1,
        zRange: '-1to1',
        timeS: 0,
        patchOriginBf: null,
        spacingM: 250,
      }).uniform,
    );
    expect(neg.slice(0, 16)).not.toEqual(zero.slice(0, 16));
  });
});
