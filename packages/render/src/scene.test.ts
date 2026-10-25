import { describe, expect, it } from 'vitest';
import { AIRFIELD, OWNSHIP_FLAG2 } from '@flying/sim-bridge/layout';
import {
  airfieldShifted,
  CUE_COUNT,
  CUE_DISTANCE_M,
  FRAME_UNIFORM_BYTES,
  FRAME_UNIFORM_FLOATS,
  MARKER_INSTANCE_CAPACITY,
  MAX_PATCH_INSTANCES,
  SceneBuilder,
  TEST_AIRFIELD,
} from './scene.ts';
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
  it('has the uniform size the shaders declare: 144 bytes', () => {
    expect(FRAME_UNIFORM_FLOATS).toBe(36);
    expect(FRAME_UNIFORM_BYTES).toBe(144);
    expect(
      new SceneBuilder().build(world({}), {
        aspect: 1,
        zRange: '0to1',
        timeS: 0,
        patchOriginBf: null,
        spacingM: 250,
      }).uniform,
    ).toHaveLength(36);
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
    // The patch span, 128 cells of 250 m: how far apart extra patch instances sit.
    expect(u[22]).toBe(250 * 128);
    expect(Math.hypot(u[16] ?? 0, u[17] ?? 0, u[18] ?? 0)).toBeCloseTo(1, 6);
    expect(u[17]).toBeGreaterThan(0);
  });

  it('draws one patch by default, and the requested number up to the cap', () => {
    const build = (patchInstances?: number) =>
      new SceneBuilder().build(world({}), {
        aspect: 1,
        zRange: '0to1',
        timeS: 0,
        patchOriginBf: null,
        spacingM: 250,
        ...(patchInstances === undefined ? {} : { patchInstances }),
      }).patchInstances;
    expect(build()).toBe(1);
    expect(build(16)).toBe(16);
    expect(build(64)).toBe(64);
    expect(build(1000)).toBe(MAX_PATCH_INSTANCES);
    expect(build(0)).toBe(1);
    expect(build(-5)).toBe(1);
    expect(build(2.7)).toBe(2);
  });

  it('gives the terrain patch offset relative to the eye in render axes, subtracted in f64', () => {
    const eye: [number, number, number] = [400_000.013, -399_999.987, 2000];
    const u = new SceneBuilder().build(world({ ownship: { pos: eye } }), {
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
    const state = world({ ownship: { pos: [0, -6000, 1000], quat: NORTH } });
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
      ownship: { pos: [0, 0, 1000] },
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

  describe('the runway', () => {
    const build = (
      patchOriginBf: [number, number, number] | null,
      airfield?: Parameters<SceneBuilder['build']>[1]['airfield'],
      eye: [number, number, number] = [0, -8000, 400],
    ) =>
      new SceneBuilder().build(world({ ownship: { pos: eye } }), {
        aspect: 1,
        zRange: '0to1',
        timeS: 0,
        patchOriginBf,
        spacingM: 250,
        ...(airfield === undefined ? {} : { airfield }),
      }).uniform;

    it('is the generated test airfield: 3 km by 45 m, its threshold 6 km south of the origin', () => {
      expect(TEST_AIRFIELD).toEqual({
        thresholdBf: [0, -6000],
        lengthM: 3000,
        widthM: 45,
      });
      expect(TEST_AIRFIELD.lengthM).toBe(AIRFIELD.LENGTH_M);
      expect(TEST_AIRFIELD.widthM).toBe(AIRFIELD.WIDTH_M);
      expect(TEST_AIRFIELD.thresholdBf[1]).toBe(-AIRFIELD.THRESHOLD_SOUTH_M);
    });

    it('goes to the shader as the threshold in patch-local east and south, length and width', () => {
      // The patch origin is 100 m west of the threshold and 1500 m north of it.
      const u = build([-100, -4500, 0]);
      expect([u[28], u[29], u[30], u[31]]).toEqual([100, 1500, 3000, 45]);
    });

    it('is left out when there is no patch to paint it on, or no runway', () => {
      expect(Array.from(build(null).subarray(28, 32))).toEqual([0, 0, 0, 0]);
      expect(Array.from(build([0, -4500, 0], null).subarray(28, 32))).toEqual([0, 0, 0, 0]);
    });

    it('does not move when the whole scene does: the subtraction is done before rounding', () => {
      const [near, far] = [
        build([0, -4500, 0], airfieldShifted(0, 0), [0, -8000, 400]),
        build([400_000, 400_000 - 4500, 0], airfieldShifted(400_000, 400_000), [
          400_000,
          400_000 - 8000,
          400,
        ]),
      ];
      expect(Array.from(far.subarray(28, 32))).toEqual(Array.from(near.subarray(28, 32)));
    });

    it('can be moved for a test that moves the scene', () => {
      expect(airfieldShifted(10, 20)).toEqual({
        thresholdBf: [10, -5980],
        lengthM: 3000,
        widthM: 45,
      });
    });
  });

  describe('the pilot effects', () => {
    const build = (
      ownship: NonNullable<Parameters<typeof makeSnapshot>[0]>['ownship'],
      gfxStrength?: number,
    ) =>
      new SceneBuilder().build(world({ ownship: { pos: [0, 0, 1000], ...ownship } }), {
        aspect: 1,
        zRange: '0to1',
        timeS: 0,
        patchOriginBf: null,
        spacingM: 250,
        ...(gfxStrength === undefined ? {} : { gfxStrength }),
      });

    it('hands the stress, the red-out and the strength to the shader', () => {
      const f = build({ pos: [0, 0, 1000], gStress: 0.5, redout: 0.25 }, 40);
      expect(f.uniform[32]).toBeCloseTo(0.5, 6);
      expect(f.uniform[33]).toBeCloseTo(0.25, 6);
      expect(f.uniform[34]).toBeCloseTo(0.4, 6);
      expect(build({ pos: [0, 0, 1000] }).uniform[34]).toBe(1);
    });

    it('runs the pass only when there is something to draw', () => {
      expect(build({ pos: [0, 0, 1000] }).fx).toBe(false);
      expect(build({ pos: [0, 0, 1000], gStress: 0.35 }).fx).toBe(false);
      expect(build({ pos: [0, 0, 1000], gStress: 0.36 }).fx).toBe(true);
      expect(build({ pos: [0, 0, 1000], redout: 0.45 }).fx).toBe(false);
      expect(build({ pos: [0, 0, 1000], redout: 0.46 }).fx).toBe(true);
      // The player's strength setting at zero switches the pass off whatever the pilot feels.
      expect(build({ pos: [0, 0, 1000], gStress: 1, redout: 1 }, 0).fx).toBe(false);
      expect(build({ pos: [0, 0, 1000], gStress: 1 }, 1).fx).toBe(true);
    });

    it('keeps the strength between 0 and 100 per cent', () => {
      expect(build({ pos: [0, 0, 1000] }, 250).uniform[34]).toBe(1);
      expect(build({ pos: [0, 0, 1000] }, -5).uniform[34]).toBe(0);
    });
  });

  describe('the cue markers', () => {
    const qrot = (q: number[], v: [number, number, number]): number[] => {
      const [x = 0, y = 0, z = 0, w = 1] = q;
      const t = [2 * (y * v[2] - z * v[1]), 2 * (z * v[0] - x * v[2]), 2 * (x * v[1] - y * v[0])];
      return [
        v[0] + w * (t[0] ?? 0) + (y * (t[2] ?? 0) - z * (t[1] ?? 0)),
        v[1] + w * (t[1] ?? 0) + (z * (t[0] ?? 0) - x * (t[2] ?? 0)),
        v[2] + w * (t[2] ?? 0) + (x * (t[1] ?? 0) - y * (t[0] ?? 0)),
      ];
    };
    const build = (aim: [number, number], flags2: number = OWNSHIP_FLAG2.MOUSE_AIM, bodies = 0) => {
      const f = new SceneBuilder().build(
        world({
          ownship: { pos: [0, 0, 1000], quat: NORTH, flags2, aim },
          bodies: Array.from({ length: bodies }, (_, i) => ({
            pos: [i, 500, 1000] as [number, number, number],
          })),
        }),
        { aspect: 1, zRange: '0to1', timeS: 0, patchOriginBf: null, spacingM: 250 },
      );
      return f;
    };
    const inst = (f: ReturnType<typeof build>, i: number): number[] =>
      Array.from(
        f.instances.subarray(i * MARKER_INSTANCE_FLOATS, (i + 1) * MARKER_INSTANCE_FLOATS),
      );

    it('are not drawn unless the core is using the mouse aim', () => {
      expect(build([0, 0], 0).instanceCount).toBe(0);
      expect(build([0, 0], 0, 3).instanceCount).toBe(3);
    });

    it('are two more instances after the bodies, 1.5 km along the nose and the aim', () => {
      const f = build([Math.PI / 2, 0], OWNSHIP_FLAG2.MOUSE_AIM, 2);
      expect(f.instanceCount).toBe(2 + CUE_COUNT);
      // The nose points north: render -z. The aim is due east: render +x.
      const nose = inst(f, 2);
      const aim = inst(f, 3);
      expect(nose.slice(0, 3).map((x) => Math.round(x * 1000) / 1000 + 0)).toEqual([
        0,
        0,
        -CUE_DISTANCE_M,
      ]);
      expect(aim.slice(0, 3).map((x) => Math.round(x * 1000) / 1000 + 0)).toEqual([
        CUE_DISTANCE_M,
        0,
        0,
      ]);
      expect(CUE_DISTANCE_M).toBe(1500);
    });

    it('are drawn in flat colours the shader is told to use, and bodies are not', () => {
      const f = build([0, 0], OWNSHIP_FLAG2.MOUSE_AIM, 1);
      expect(inst(f, 0).slice(8)).toEqual([0, 0, 0, 0]);
      expect(inst(f, 1)[11]).toBe(1);
      expect(inst(f, 2)[11]).toBe(1);
      expect(inst(f, 1).slice(8, 11)).not.toEqual(inst(f, 2).slice(8, 11)); // nose and aim differ
      expect(inst(f, 1)[3]).toBeLessThan(inst(f, 2)[3] ?? 0); // the aim cue is the bigger
    });

    it('turn the dart to point along their ray, whatever the direction', () => {
      for (const [az, el] of [
        [0, 0],
        [0.7, 0.3],
        [-2.1, -0.5],
        [Math.PI, 0],
        [0, Math.PI / 2],
        [1, -Math.PI / 2],
      ] as [number, number][]) {
        const f = build([az, el]);
        const aim = inst(f, 1);
        const q = aim.slice(4, 8);
        expect(Math.hypot(...q), `az ${String(az)} el ${String(el)}`).toBeCloseTo(1, 5);
        const dir = qrot(q, [1, 0, 0]);
        const want = [aim[0] ?? 0, aim[1] ?? 0, aim[2] ?? 0].map((x) => x / CUE_DISTANCE_M);
        dir.forEach((x, k) => {
          expect(x, `az ${String(az)} el ${String(el)} axis ${String(k)}`).toBeCloseTo(
            want[k] ?? 0,
            4,
          );
        });
      }
    });

    it('fit in the instance buffer even with every body', () => {
      const f = build([0.1, 0.1], OWNSHIP_FLAG2.MOUSE_AIM, 32);
      expect(f.instances.length).toBe(MARKER_INSTANCE_CAPACITY * MARKER_INSTANCE_FLOATS);
      expect(f.instanceCount).toBeLessThanOrEqual(MARKER_INSTANCE_CAPACITY);
    });
  });
});
