import { describe, expect, it } from 'vitest';
import {
  AIRFIELD,
  BANDIT_LIFE,
  COMBAT_FLAG,
  EXPLOSION_KIND,
  MISSILE_STATE,
  OWNSHIP_FLAG2,
  SNAPSHOT,
} from '@flying/sim-bridge/layout';
import {
  airfieldShifted,
  CUE_COUNT,
  CUE_DISTANCE_M,
  FRAME_AT,
  FRAME_UNIFORM_BYTES,
  FRAME_UNIFORM_FLOATS,
  LEVEL_UNIFORM_FLOATS,
  MARKER_INSTANCE_CAPACITY,
  MAX_PATCH_INSTANCES,
  SceneBuilder,
  TEST_AIRFIELD,
  TINT_FLAT,
  TINT_STREAK,
  type LevelSet,
  type LevelSource,
} from './scene.ts';
import { SnapshotInterpolator } from './interp.ts';
import { FIXTURE_MODEL } from './model-fixture.ts';
import { ModelRig } from './model-rig.ts';
import { MARKER_INSTANCE_FLOATS, MARKER_SIZE_M } from './markers.ts';
import { makeSnapshot, NORTH, type FixtureCombat } from './snapshot-fixture.ts';

/** No terrain levels at all. */
const NO_TERRAIN: LevelSet = { mesh: () => undefined };

/** A terrain level 0 with the given origin and spacing, and nothing else. */
function levelZero(originBf: [number, number, number], spacingM = 250): LevelSet {
  const level: LevelSource = { originBf, spacingM };
  return { mesh: (n) => (n === 0 ? level : undefined) };
}

/** Element `i` of `a`, or NaN (which fails any comparison) if there is none. */
const at = (a: ArrayLike<number>, i: number): number => a[i] ?? Number.NaN;

/** `v` turned by the unit quaternion `q`, given as (x, y, z, w) the way the shader takes it. */
const qrot = (q: number[], v: [number, number, number]): number[] => {
  const [x = 0, y = 0, z = 0, w = 1] = q;
  const t = [2 * (y * v[2] - z * v[1]), 2 * (z * v[0] - x * v[2]), 2 * (x * v[1] - y * v[0])];
  return [
    v[0] + w * (t[0] ?? 0) + (y * (t[2] ?? 0) - z * (t[1] ?? 0)),
    v[1] + w * (t[1] ?? 0) + (z * (t[0] ?? 0) - x * (t[2] ?? 0)),
    v[2] + w * (t[2] ?? 0) + (x * (t[1] ?? 0) - y * (t[0] ?? 0)),
  ];
};

function world(extra: Parameters<typeof makeSnapshot>[0]) {
  const i = new SnapshotInterpolator();
  i.push(makeSnapshot({ publishMs: 1000, ...extra }));
  const state = i.sample(2000);
  if (!state) throw new Error('no state');
  return state;
}

describe('SceneBuilder', () => {
  it('has the uniform size the shaders declare: 224 bytes', () => {
    expect(FRAME_UNIFORM_FLOATS).toBe(56);
    expect(FRAME_UNIFORM_BYTES).toBe(224);
    expect(LEVEL_UNIFORM_FLOATS).toBe(12);
    expect(
      new SceneBuilder().build(world({}), {
        aspect: 1,
        zRange: '0to1',
        timeS: 0,
        terrain: NO_TERRAIN,
      }).uniform,
    ).toHaveLength(56);
  });

  it('writes time, the field of view and the aspect into params, and a unit upward sun', () => {
    const u = new SceneBuilder().build(world({}), {
      aspect: 1.5,
      zRange: '0to1',
      timeS: 12.5,
      terrain: NO_TERRAIN,
    }).uniform;
    expect(u[FRAME_AT.PARAMS]).toBe(12.5);
    // 70 degrees vertical: tan(35 degrees).
    expect(u[FRAME_AT.PARAMS + 1]).toBeCloseTo(0.70021, 4);
    expect(u[FRAME_AT.PARAMS + 2]).toBe(1.5);
    const sun = FRAME_AT.SUN_DIR;
    expect(Math.hypot(u[sun] ?? 0, u[sun + 1] ?? 0, u[sun + 2] ?? 0)).toBeCloseTo(1, 6);
    expect(u[sun + 1]).toBeGreaterThan(0);
  });

  it('draws one patch by default, and the requested number up to the cap', () => {
    const build = (patchInstances?: number) =>
      new SceneBuilder().build(world({}), {
        aspect: 1,
        zRange: '0to1',
        timeS: 0,
        terrain: NO_TERRAIN,
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

  it('gives each terrain level its offset relative to the eye in render axes, subtracted in f64', () => {
    const eye: [number, number, number] = [400_000.013, -399_999.987, 2000];
    const u = new SceneBuilder().build(world({ ownship: { pos: eye } }), {
      aspect: 1,
      zRange: '0to1',
      timeS: 0,
      terrain: levelZero([400_000, -400_000, 0]),
    }).levelUniform;
    // BF delta (east, north, up) = (-0.013, -0.013, -2000) -> render (east, up, south).
    expect(u[0]).toBeCloseTo(-0.013, 5);
    expect(u[1]).toBeCloseTo(-2000, 9);
    expect(u[2]).toBeCloseTo(0.013, 5);
    expect(u[3]).toBe(0); // level 0
  });

  it('describes each level present: its number, post spacing and span, and leaves the rest zero', () => {
    const terrain: LevelSet = {
      mesh: (n) => (n === 1 ? { originBf: [0, 0, 0], spacingM: 1000 } : undefined),
    };
    const f = new SceneBuilder().build(world({}), { aspect: 1, zRange: '0to1', timeS: 0, terrain });
    expect(f.levelMask).toBe(0b10);
    const at = LEVEL_UNIFORM_FLOATS;
    expect([f.levelUniform[at + 3], f.levelUniform[at + 4], f.levelUniform[at + 5]]).toEqual([
      1, 1000, 128_000,
    ]);
    expect(Array.from(f.levelUniform.subarray(0, at))).toEqual(new Array(at).fill(0));
    // Only level 0 paints a runway.
    expect(Array.from(f.levelUniform.subarray(at + 8, at + 12))).toEqual([0, 0, 0, 0]);
  });

  it('puts a point straight ahead of the observer at the centre of the screen', () => {
    const scene = new SceneBuilder();
    const state = world({ ownship: { pos: [0, -6000, 1000], quat: NORTH } });
    const f = scene.build(state, {
      aspect: 16 / 9,
      zRange: '0to1',
      timeS: 0,
      terrain: NO_TERRAIN,
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
      terrain: NO_TERRAIN,
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
      terrain: NO_TERRAIN,
    });
    const b = scene.build(state, {
      aspect: 1,
      zRange: '0to1',
      timeS: 1,
      terrain: NO_TERRAIN,
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
        terrain: NO_TERRAIN,
      }).uniform,
    );
    const neg = Float32Array.from(
      scene.build(state, {
        aspect: 1,
        zRange: '-1to1',
        timeS: 0,
        terrain: NO_TERRAIN,
      }).uniform,
    );
    expect(neg.slice(0, 16)).not.toEqual(zero.slice(0, 16));
  });

  describe('the runway', () => {
    /** Level 0's block: the runway is its last vec4. */
    const build = (
      patchOriginBf: [number, number, number] | null,
      airfield?: Parameters<SceneBuilder['build']>[1]['airfield'],
      eye: [number, number, number] = [0, -8000, 400],
    ) =>
      new SceneBuilder().build(world({ ownship: { pos: eye } }), {
        aspect: 1,
        zRange: '0to1',
        timeS: 0,
        terrain: patchOriginBf ? levelZero(patchOriginBf) : NO_TERRAIN,
        ...(airfield === undefined ? {} : { airfield }),
      }).levelUniform;

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
      expect([u[8], u[9], u[10], u[11]]).toEqual([100, 1500, 3000, 45]);
    });

    it('is left out when there is no patch to paint it on, or no runway', () => {
      expect(Array.from(build(null).subarray(8, 12))).toEqual([0, 0, 0, 0]);
      expect(Array.from(build([0, -4500, 0], null).subarray(8, 12))).toEqual([0, 0, 0, 0]);
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
      expect(Array.from(far.subarray(8, 12))).toEqual(Array.from(near.subarray(8, 12)));
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
        terrain: NO_TERRAIN,
        ...(gfxStrength === undefined ? {} : { gfxStrength }),
      });

    it('hands the stress, the red-out and the strength to the shader', () => {
      const f = build({ pos: [0, 0, 1000], gStress: 0.5, redout: 0.25 }, 40);
      expect(f.uniform[FRAME_AT.FX]).toBeCloseTo(0.5, 6);
      expect(f.uniform[FRAME_AT.FX + 1]).toBeCloseTo(0.25, 6);
      expect(f.uniform[FRAME_AT.FX + 2]).toBeCloseTo(0.4, 6);
      expect(build({ pos: [0, 0, 1000] }).uniform[FRAME_AT.FX + 2]).toBe(1);
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
      expect(build({ pos: [0, 0, 1000] }, 250).uniform[FRAME_AT.FX + 2]).toBe(1);
      expect(build({ pos: [0, 0, 1000] }, -5).uniform[FRAME_AT.FX + 2]).toBe(0);
    });
  });

  describe('the cue markers', () => {
    const build = (aim: [number, number], flags2: number = OWNSHIP_FLAG2.MOUSE_AIM, bodies = 0) => {
      const f = new SceneBuilder().build(
        world({
          ownship: { pos: [0, 0, 1000], quat: NORTH, flags2, aim },
          bodies: Array.from({ length: bodies }, (_, i) => ({
            pos: [i, 500, 1000] as [number, number, number],
          })),
        }),
        { aspect: 1, zRange: '0to1', timeS: 0, terrain: NO_TERRAIN },
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

  describe("the duel's markers", () => {
    const v3 = (x: number, y: number, z: number): [number, number, number] => [x, y, z];
    const build = (combat: FixtureCombat, opts: { bodies?: number; bandit?: boolean } = {}) =>
      new SceneBuilder().build(
        world({
          ownship: { pos: [0, 0, 1000], quat: NORTH },
          bodies: Array.from({ length: opts.bodies ?? 0 }, (_, i) => ({ pos: v3(i, 500, 1000) })),
          combat: { flags: COMBAT_FLAG.ACTIVE, ...combat },
        }),
        {
          aspect: 1,
          zRange: '0to1',
          timeS: 0,
          terrain: NO_TERRAIN,
          ...(opts.bandit ? { banditModel: new ModelRig(FIXTURE_MODEL) } : {}),
        },
      );
    const inst = (f: ReturnType<typeof build>, i: number): number[] =>
      Array.from(
        f.instances.subarray(i * MARKER_INSTANCE_FLOATS, (i + 1) * MARKER_INSTANCE_FLOATS),
      );
    /** Where the instance's dart points (body x turned by its quaternion), in render axes. */
    const nose = (m: number[]): number[] =>
      qrot(m.slice(4, 8), [1, 0, 0]).map((x) => Math.round(x * 1e4) / 1e4 + 0);
    const round = (xs: number[], digits = 3): number[] =>
      xs.map((x) => Math.round(x * 10 ** digits) / 10 ** digits + 0);

    it('are a dart per missile in flight, a streak per tracer and four spikes per young explosion, after the bodies', () => {
      const f = build(
        {
          missileSlots: [
            { pos: v3(0, 1000, 1000), vel: v3(0, 600, 0), state: MISSILE_STATE.BOOST },
            { pos: v3(0, 1100, 1000), vel: v3(0, 600, 0), state: MISSILE_STATE.COAST },
            // A missile that has burst is drawn by its explosion, not as a dart.
            { pos: v3(0, 1200, 1000), vel: v3(0, 600, 0), state: MISSILE_STATE.BURST },
          ],
          tracers: [0, 1, 2].map((id) => ({ pos: v3(id, 300, 1000), vel: v3(1000, 0, 0), id })),
          explosions: [
            { pos: v3(0, 2000, 1000), age: 0.3, kind: EXPLOSION_KIND.AIRCRAFT },
            // Older than its growth: gone.
            { pos: v3(0, 2000, 1000), age: 0.7, kind: EXPLOSION_KIND.MISSILE },
          ],
        },
        { bodies: 1 },
      );
      expect(f.instanceCount).toBe(1 + 2 + 3 + 4);
      // The body keeps its index-coloured look; the rest are tinted.
      expect(inst(f, 0)[11]).toBe(0);
    });

    it('point a white dart along each missile, where it is', () => {
      const f = build({
        missileSlots: [{ pos: v3(0, 1000, 1000), vel: v3(0, 600, 0), state: MISSILE_STATE.BOOST }],
      });
      const m = inst(f, 0);
      // BF north of the eye is render -z; flying north, its nose points along -z too.
      expect(round(m.slice(0, 4))).toEqual([0, 0, -1000, 3]);
      expect(nose(m)).toEqual([0, 0, -1]);
      expect(round(m.slice(8, 11))).toEqual([0.92, 0.92, 0.92]);
      expect(m[11]).toBe(TINT_FLAT);
    });

    it('draw each tracer as a streak along its velocity', () => {
      const f = build({ tracers: [{ pos: v3(0, 300, 1100), vel: v3(0, 0, 900), id: 5 }] });
      const m = inst(f, 0);
      expect(round(m.slice(0, 4))).toEqual([0, 100, -300, 12]);
      // Climbing: BF up is render +y.
      expect(nose(m)).toEqual([0, 1, 0]);
      expect(m[11]).toBe(TINT_STREAK);
      expect(round(m.slice(8, 11))).toEqual([1, 0.85, 0.4]);
    });

    it("grow an explosion's spikes out from its centre: up, down and both ways along a horizontal", () => {
      const f = build({
        explosions: [
          { pos: v3(0, 2000, 1000), age: 0.15, kind: EXPLOSION_KIND.MISSILE },
          { pos: v3(0, 2000, 1000), age: 0.3, kind: EXPLOSION_KIND.AIRCRAFT },
        ],
      });
      expect(f.instanceCount).toBe(8);
      // A quarter of the way through its growth a missile's burst is half its 15 m (the square
      // root of the age); halfway, an aircraft's is 40 m times the root of a half.
      const small = 15 * Math.sqrt(0.25);
      const big = 40 * Math.sqrt(0.5);
      for (let k = 0; k < 8; k++) {
        const m = inst(f, k);
        const len = k < 4 ? small : big;
        expect(m[3]).toBeCloseTo(len, 4);
        expect(m[11]).toBe(TINT_STREAK);
        const dir = nose(m);
        // Each spike is centred half its length out from the burst, along its direction.
        const centre = [0, 0, -2000];
        dir.forEach((d, axis) => {
          expect(m[axis]).toBeCloseTo((centre[axis] ?? 0) + d * len * 0.5, 2);
        });
        if (k % 4 === 0) expect(dir).toEqual([0, 1, 0]);
        if (k % 4 === 1) expect(dir).toEqual([0, -1, 0]);
        if (k % 4 >= 2) expect(dir[1]).toBe(0);
      }
      // The second burst's horizontal is turned from the first's, so neighbours do not look alike.
      expect(nose(inst(f, 2))).not.toEqual(nose(inst(f, 6)));
    });

    it("draw the bandit's model only with a rig for it, and only while it exists", () => {
      const here = { pos: v3(0, 120, 1000), quat: NORTH };
      expect(build({ bandit: here }).bandit).toBe(false);
      expect(
        build({ bandit: { ...here, life: BANDIT_LIFE.ABSENT } }, { bandit: true }).bandit,
      ).toBe(false);
      const f = build({ bandit: here }, { bandit: true });
      expect(f.bandit).toBe(true);
      expect(f.banditUniform.some((x) => x !== 0)).toBe(true);
      // A wreck still falls, so it is drawn too.
      expect(build({ bandit: { ...here, life: BANDIT_LIFE.WRECK } }, { bandit: true }).bandit).toBe(
        true,
      );
    });

    it('fit in the instance buffer with every body, both cues and every combat slot full', () => {
      const f = new SceneBuilder().build(
        world({
          ownship: { pos: [0, 0, 1000], quat: NORTH, flags2: OWNSHIP_FLAG2.MOUSE_AIM, aim: [0, 0] },
          bodies: Array.from({ length: SNAPSHOT.BODY_CAPACITY }, (_, i) => ({
            pos: v3(i, 500, 1000),
          })),
          combat: {
            flags: COMBAT_FLAG.ACTIVE,
            missileSlots: Array.from({ length: SNAPSHOT.MISSILE_CAPACITY }, (_, i) => ({
              pos: v3(i, 900, 1000),
              vel: v3(0, 600, 0),
              state: MISSILE_STATE.BOOST,
            })),
            tracers: Array.from({ length: SNAPSHOT.TRACER_CAPACITY }, (_, i) => ({
              pos: v3(i, 300, 1000),
              vel: v3(0, 1000, 0),
              id: i,
            })),
            explosions: Array.from({ length: SNAPSHOT.EXPLOSION_CAPACITY }, (_, i) => ({
              pos: v3(i * 10, 2000, 1000),
              age: 0.1,
              kind: EXPLOSION_KIND.AIRCRAFT,
            })),
          },
        }),
        { aspect: 1, zRange: '0to1', timeS: 0, terrain: NO_TERRAIN },
      );
      expect(f.instanceCount).toBe(MARKER_INSTANCE_CAPACITY);
      expect(f.instances.length).toBe(MARKER_INSTANCE_CAPACITY * MARKER_INSTANCE_FLOATS);
    });
  });
});
