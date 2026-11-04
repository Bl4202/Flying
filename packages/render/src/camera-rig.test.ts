import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { newViewState, type ViewState } from '@flying/sim-bridge/view-channel';
import {
  CameraRig,
  CHASE_FOV_BOOST_DEG,
  CHASE_LAG_S,
  CHASE_VMAX_MPS,
  G_OFFSET_MAX_M,
  HEAD_T63_S,
  MIN_CLEARANCE_M,
  NEAR_COCKPIT_M,
  NEAR_EXTERNAL_M,
  SHAKE_REDUCED_MAX_RAD,
  shakeDrive,
  ZOOM_FOV_DEG,
  type CockpitView,
  type RigState,
  type RigTerrain,
} from './camera-rig.ts';
import { NORTH, quatFromColumns } from './snapshot-fixture.ts';

const DEG = Math.PI / 180;
const COCKPIT: CockpitView = {
  eyeBodyM: [3.6, 0, -0.85],
  headLimits: { yawDeg: 135, pitchUpDeg: 60, pitchDownDeg: 60 },
};
const FLAT: RigTerrain = { heightAt: () => 0 };

function aircraft(over: Partial<RigState> = {}): RigState {
  return {
    ownshipPosBf: new Float64Array([0, 0, 100]),
    ownshipQuat: Float32Array.from(NORTH),
    ownshipVelBf: new Float32Array([0, 0, 0]),
    ownshipSpeed: 0,
    gLagged: 1,
    alpha: 0,
    alphaWarn: 0.3,
    groundSpeed: 0,
    abLevel: 0,
    onGround: false,
    ...over,
  };
}

function rotate(q: ArrayLike<number>, v: [number, number, number]): [number, number, number] {
  const [w, x, y, z] = [q[0] ?? 1, q[1] ?? 0, q[2] ?? 0, q[3] ?? 0];
  const t = [2 * (y * v[2] - z * v[1]), 2 * (z * v[0] - x * v[2]), 2 * (x * v[1] - y * v[0])];
  return [
    v[0] + w * (t[0] ?? 0) + (y * (t[2] ?? 0) - z * (t[1] ?? 0)),
    v[1] + w * (t[1] ?? 0) + (z * (t[0] ?? 0) - x * (t[2] ?? 0)),
    v[2] + w * (t[2] ?? 0) + (x * (t[1] ?? 0) - y * (t[0] ?? 0)),
  ];
}

const angleBetween = (a: number[], b: number[]): number => {
  const d = a.reduce((s, v, i) => s + v * (b[i] ?? 0), 0);
  return Math.acos(Math.min(Math.max(d / (Math.hypot(...a) * Math.hypot(...b)), -1), 1));
};

/** Runs `frames` steps of `dt` and returns the last pose's eye and forward. */
function run(
  rig: CameraRig,
  state: RigState,
  view: ViewState,
  frames: number,
  dt = 1 / 60,
): { eye: number[]; forward: number[] } {
  for (let i = 0; i < frames; i++) rig.step(state, view, dt);
  return { eye: [...rig.pose.eyeBf], forward: rotate(rig.pose.quatBf, [1, 0, 0]) };
}

describe('the cockpit camera', () => {
  it('puts the eye at the design eye, in body axes from the reference point, near plane 0.3 m', () => {
    const rig = new CameraRig(COCKPIT, FLAT);
    const pose = rig.step(aircraft(), newViewState(), 1 / 60);
    // Heading north: body forward is north (+y), right is east, down is -z. The eye is 3.6 m
    // forward of the reference point and 0.85 m above it.
    expect(pose.eyeBf[0]).toBeCloseTo(0, 6);
    expect(pose.eyeBf[1]).toBeCloseTo(3.6, 6);
    expect(pose.eyeBf[2]).toBeCloseTo(100.85, 6);
    expect(pose.mode).toBe('cockpit');
    expect(pose.near).toBe(NEAR_COCKPIT_M);
    expect(NEAR_COCKPIT_M).toBe(0.3);
  });

  it('looks along the body with the head centred', () => {
    const rig = new CameraRig(COCKPIT, FLAT);
    const { forward } = run(rig, aircraft(), newViewState(), 3);
    expect(forward[0]).toBeCloseTo(0, 6);
    expect(forward[1]).toBeCloseTo(1, 6);
    expect(forward[2]).toBeCloseTo(0, 6);
  });

  it('turns the head right with positive yaw and up with positive pitch', () => {
    const view = newViewState();
    const right = new CameraRig(COCKPIT, FLAT);
    right.step(aircraft(), view, 1 / 60);
    view.mouseYawTotal = 90 * DEG;
    const r = run(right, aircraft(), view, 120);
    expect(r.forward[0]).toBeCloseTo(1, 3); // east is to the right of north
    const up = new CameraRig(COCKPIT, FLAT);
    const v2 = newViewState();
    up.step(aircraft(), v2, 1 / 60);
    v2.mousePitchTotal = 30 * DEG;
    const u = run(up, aircraft(), v2, 120);
    expect(u.forward[2]).toBeCloseTo(Math.sin(30 * DEG), 3);
    expect(u.forward[1]).toBeCloseTo(Math.cos(30 * DEG), 3);
  });

  it('follows the aircraft attitude: banked, the horizon tilts with it', () => {
    const bank = 30 * DEG;
    // Heading north, banked right: right wing down.
    const f = [0, 1, 0] as const;
    const down = [Math.sin(bank), 0, -Math.cos(bank)] as const;
    const right = [Math.cos(bank), 0, Math.sin(bank)] as const;
    const q = Float32Array.from(quatFromColumns([...f], [...right], [...down]));
    const rig = new CameraRig(COCKPIT, FLAT);
    rig.step(aircraft({ ownshipQuat: q }), newViewState(), 1 / 60);
    const camRight = rotate(rig.pose.quatBf, [0, 1, 0]);
    expect(camRight[2]).toBeCloseTo(Math.sin(bank), 5);
  });

  describe('K2: head limits and smoothing', () => {
    it('stops the head at the limits: yaw 135 degrees, pitch up 60, pitch down 60', () => {
      const rig = new CameraRig(COCKPIT, FLAT);
      const view = newViewState();
      rig.step(aircraft(), view, 1 / 60);
      view.lookYawRate = 5;
      view.lookPitchRate = 5;
      run(rig, aircraft(), view, 600);
      expect(rig.head.yaw / DEG).toBeCloseTo(135, 6);
      expect(rig.head.pitch / DEG).toBeCloseTo(60, 6);
      view.lookYawRate = -5;
      view.lookPitchRate = -5;
      run(rig, aircraft(), view, 1200);
      expect(rig.head.yaw / DEG).toBeCloseTo(-135, 6);
      expect(rig.head.pitch / DEG).toBeCloseTo(-60, 6);
      // The head never passed a limit on the way, whatever the spring did.
      view.lookYawRate = 20;
      for (let i = 0; i < 600; i++) {
        rig.step(aircraft(), view, 1 / 60);
        expect(Math.abs(rig.head.yaw)).toBeLessThanOrEqual(135 * DEG + 1e-9);
      }
    });

    it('recentres on request, and keeps the target at the limit while the key is held', () => {
      const rig = new CameraRig(COCKPIT, FLAT);
      const view = newViewState();
      rig.step(aircraft(), view, 1 / 60);
      view.lookYawRate = 3;
      run(rig, aircraft(), view, 300);
      expect(rig.headTarget.yaw / DEG).toBeCloseTo(135, 6);
      view.lookYawRate = 0;
      view.recentreCount += 1;
      run(rig, aircraft(), view, 120);
      expect(rig.head.yaw).toBeCloseTo(0, 6);
      expect(rig.headTarget.yaw).toBe(0);
    });

    it('reaches 63 per cent of a step in 60 ms (within 10 ms)', () => {
      const rig = new CameraRig(COCKPIT, FLAT);
      const view = newViewState();
      rig.step(aircraft(), view, 1 / 1000);
      view.mouseYawTotal = 0.5; // a 0.5 rad step in the target, in one go
      let t = 0;
      let reached = -1;
      for (let i = 0; i < 500; i++) {
        rig.step(aircraft(), view, 0.001);
        t += 0.001;
        if (reached < 0 && rig.head.yaw >= 0.632 * 0.5) reached = t;
      }
      expect(Math.abs(reached - HEAD_T63_S)).toBeLessThan(0.01);
      // Critically damped: it settles without overshoot.
      expect(rig.head.yaw).toBeLessThanOrEqual(0.5 + 1e-9);
      expect(rig.head.yaw).toBeCloseTo(0.5, 4);
    });

    it('does not jump on the first frame when the channel already holds mouse look', () => {
      const rig = new CameraRig(COCKPIT, FLAT);
      const view = newViewState();
      view.mouseYawTotal = 1234.5; // history before this renderer started
      rig.step(aircraft(), view, 1 / 60);
      expect(rig.headTarget.yaw).toBe(0);
    });
  });

  describe('the field of view', () => {
    it('is the setting, 75 degrees by default, between 50 and 120', () => {
      const rig = new CameraRig(COCKPIT, FLAT);
      const view = newViewState();
      run(rig, aircraft(), view, 120);
      expect(rig.pose.fovY / DEG).toBeCloseTo(75, 3);
      view.fovDeg = 100;
      run(rig, aircraft(), view, 120);
      expect(rig.pose.fovY / DEG).toBeCloseTo(100, 3);
    });

    it('narrows to 30 degrees while zoom is held, and back when it is released', () => {
      const rig = new CameraRig(COCKPIT, FLAT);
      const view = newViewState();
      view.zoom = true;
      run(rig, aircraft(), view, 120);
      expect(rig.pose.fovY / DEG).toBeCloseTo(ZOOM_FOV_DEG, 3);
      view.zoom = false;
      run(rig, aircraft(), view, 120);
      expect(rig.pose.fovY / DEG).toBeCloseTo(75, 3);
    });
  });

  describe('K3: motion effects', () => {
    it('moves the eye by the load the pilot feels, by at most 2 cm, and not at all with reduced motion', () => {
      const base = new CameraRig(COCKPIT, FLAT);
      base.step(aircraft(), newViewState(), 1 / 60);
      const level = [...base.pose.eyeBf];
      const heavy = new CameraRig(COCKPIT, FLAT);
      heavy.step(aircraft({ gLagged: 9 }), newViewState(), 1 / 60);
      const sag = Math.hypot(...[0, 1, 2].map((k) => (heavy.pose.eyeBf[k] ?? 0) - (level[k] ?? 0)));
      expect(sag).toBeGreaterThan(0.01);
      expect(sag).toBeLessThanOrEqual(G_OFFSET_MAX_M + 1e-9);
      const calm = new CameraRig(COCKPIT, FLAT);
      const reduced = newViewState();
      reduced.reducedMotion = true;
      calm.step(aircraft({ gLagged: 9 }), reduced, 1 / 60);
      for (let k = 0; k < 3; k++) expect(calm.pose.eyeBf[k]).toBeCloseTo(level[k] ?? 0, 9);
    });

    const shaking = aircraft({
      alpha: 0.4,
      alphaWarn: 0.3,
      groundSpeed: 80,
      abLevel: 1,
      onGround: true,
    });

    /** The largest angle the view strays from the shake-free one over many frames. */
    function worstDeviation(view: ViewState): number {
      const rig = new CameraRig(COCKPIT, FLAT);
      const steady = new CameraRig(COCKPIT, FLAT);
      const none = newViewState();
      none.shake = 0;
      let worst = 0;
      for (let i = 0; i < 2000; i++) {
        rig.step(shaking, view, 1 / 60);
        steady.step(shaking, none, 1 / 60);
        worst = Math.max(
          worst,
          angleBetween(rotate(rig.pose.quatBf, [1, 0, 0]), rotate(steady.pose.quatBf, [1, 0, 0])),
        );
      }
      return worst;
    }

    it('shakes with buffet, ground roll and the afterburner, in proportion to the setting', () => {
      const full = newViewState();
      const half = newViewState();
      half.shake = 0.5;
      const off = newViewState();
      off.shake = 0;
      expect(worstDeviation(full)).toBeGreaterThan(0.5 * DEG);
      expect(worstDeviation(half)).toBeLessThan(worstDeviation(full) * 0.75);
      expect(worstDeviation(off)).toBeLessThan(1e-6);
    });

    it('keeps the shake to 0.5 degrees with reduced motion (AC-57.3)', () => {
      const reduced = newViewState();
      reduced.reducedMotion = true;
      const worst = worstDeviation(reduced);
      expect(worst).toBeGreaterThan(0.1 * DEG); // it is not simply off
      expect(worst).toBeLessThanOrEqual(SHAKE_REDUCED_MAX_RAD + 1e-9);
    });

    it('drives the shake from the three sources and from nothing at rest', () => {
      // alpha, warning angle, ground speed, afterburner level, weight on wheels
      expect(shakeDrive(0, 0.3, 0, 0, false)).toBe(0);
      expect(shakeDrive(0.3, 0.3, 0, 0, false)).toBe(1); // at the warning angle
      expect(shakeDrive(0.3 - 3 * DEG, 0.3, 0, 0, false)).toBe(0); // where it starts
      expect(shakeDrive(0, 0.3, 0, 1, false)).toBeCloseTo(0.3, 9);
      expect(shakeDrive(0, 0.3, 60, 0, true)).toBeCloseTo(0.4, 9);
      expect(shakeDrive(0, 0.3, 60, 0, false)).toBe(0);
      expect(shakeDrive(1, 0.3, 60, 1, true)).toBe(1);
    });
  });
});

describe('the chase camera', () => {
  const chase = (): ViewState => ({ ...newViewState(), camera: 'chase' });

  it('sits behind the nose and above at rest: 45 m back and a fifth of that up', () => {
    const rig = new CameraRig(COCKPIT, FLAT);
    const pose = rig.step(aircraft(), chase(), 1 / 60);
    expect(pose.mode).toBe('chase');
    expect(pose.near).toBe(NEAR_EXTERNAL_M);
    // Heading north at rest: behind is south.
    expect(pose.eyeBf[1]).toBeCloseTo(-45, 2);
    expect(pose.eyeBf[0]).toBeCloseTo(0, 4);
    expect(pose.eyeBf[2]).toBeCloseTo(100 + 9, 2);
    // It looks at the aircraft, level: forward points north and down a little.
    const f = rotate(pose.quatBf, [1, 0, 0]);
    expect(f[1]).toBeGreaterThan(0.97);
    expect(f[2]).toBeLessThan(0);
  });

  it('keeps the horizon level whatever the aircraft does', () => {
    const rig = new CameraRig(COCKPIT, FLAT);
    // Inverted and banked: the aircraft's roll must not reach the camera.
    const f = [0, 1, 0];
    const q = Float32Array.from(
      quatFromColumns(f as [number, number, number], [-1, 0, 0], [0, 0, 1]),
    );
    rig.step(aircraft({ ownshipQuat: q }), chase(), 1 / 60);
    const down = rotate(rig.pose.quatBf, [0, 0, 1]);
    expect(down[2]).toBeLessThan(-0.95); // the camera's down is the world's down
  });

  it('follows the velocity direction, not the nose, once it is moving', () => {
    const rig = new CameraRig(COCKPIT, FLAT);
    // Nose north, but flying east at 100 m/s (a crab): the camera goes behind the velocity, west.
    const { eye } = run(
      rig,
      aircraft({ ownshipVelBf: new Float32Array([100, 0, 0]), ownshipSpeed: 100 }),
      chase(),
      600,
    );
    expect(eye[0]).toBeCloseTo(-45, 0);
    expect(Math.abs(eye[1] ?? 99)).toBeLessThan(0.5);
  });

  it('blends to the nose below 20 m/s, where the velocity means nothing', () => {
    const rig = new CameraRig(COCKPIT, FLAT);
    const { eye } = run(
      rig,
      aircraft({ ownshipVelBf: new Float32Array([5, 0, 0]), ownshipSpeed: 5 }),
      chase(),
      600,
    );
    // At 5 m/s the weight on the velocity is a quarter: mostly nose.
    expect(eye[1]).toBeLessThan(-35);
    expect(eye[0]).toBeLessThan(0);
    expect(eye[0]).toBeGreaterThan(-20);
  });

  describe('K4: lag and field of view', () => {
    it('follows with a time constant of 0.8 s (within the 0.6 to 1.2 s range)', () => {
      const rig = new CameraRig(COCKPIT, FLAT);
      const view = chase();
      run(rig, aircraft(), view, 300);
      const settled = [...rig.pose.eyeBf];
      // Teleport nothing: slide the aircraft 100 m east and watch the camera catch up.
      const moved = aircraft({ ownshipPosBf: new Float64Array([100, 0, 100]) });
      const target = (settled[0] ?? 0) + 100;
      let t = 0;
      let reached = -1;
      for (let i = 0; i < 600; i++) {
        rig.step(moved, view, 1 / 100);
        t += 1 / 100;
        if (
          reached < 0 &&
          (rig.pose.eyeBf[0] ?? 0) - (settled[0] ?? 0) >= 0.632 * (target - (settled[0] ?? 0))
        ) {
          reached = t;
        }
      }
      expect(CHASE_LAG_S).toBe(0.8);
      expect(reached).toBeGreaterThan(0.6);
      expect(reached).toBeLessThan(1.2);
    });

    it('widens the field of view by 8 degrees at top speed, and not at rest', () => {
      const rest = new CameraRig(COCKPIT, FLAT);
      run(rest, aircraft(), chase(), 240);
      expect(rest.pose.fovY / DEG).toBeCloseTo(75, 2);
      const fast = new CameraRig(COCKPIT, FLAT);
      run(
        fast,
        aircraft({
          ownshipVelBf: new Float32Array([0, CHASE_VMAX_MPS, 0]),
          ownshipSpeed: CHASE_VMAX_MPS,
        }),
        chase(),
        240,
      );
      expect(Math.abs(fast.pose.fovY / DEG - (75 + CHASE_FOV_BOOST_DEG))).toBeLessThan(0.5);
      expect(CHASE_FOV_BOOST_DEG).toBe(8);
      // With reduced motion the field of view does not move with speed.
      const calm = new CameraRig(COCKPIT, FLAT);
      const view = chase();
      view.reducedMotion = true;
      run(
        calm,
        aircraft({
          ownshipSpeed: CHASE_VMAX_MPS,
          ownshipVelBf: new Float32Array([0, CHASE_VMAX_MPS, 0]),
        }),
        view,
        240,
      );
      expect(calm.pose.fovY / DEG).toBeCloseTo(75, 2);
    });

    it('snaps to the aircraft when it jumps (a respawn), instead of sweeping across the map', () => {
      const rig = new CameraRig(COCKPIT, FLAT);
      run(rig, aircraft(), chase(), 60);
      const far = aircraft({ ownshipPosBf: new Float64Array([50_000, 20_000, 100]) });
      rig.step(far, chase(), 1 / 60);
      expect(
        Math.hypot((rig.pose.eyeBf[0] ?? 0) - 50_000, (rig.pose.eyeBf[1] ?? 0) - 20_000),
      ).toBeLessThan(60);
    });
  });

  it('orbits with free look and comes back on recentre', () => {
    const rig = new CameraRig(COCKPIT, FLAT);
    const view = chase();
    rig.step(aircraft(), view, 1 / 60);
    view.mouseYawTotal = Math.PI / 2; // a quarter turn round the aircraft
    const { eye } = run(rig, aircraft(), view, 600);
    // It was south of the aircraft; a quarter turn puts it east or west, still 45 m out.
    expect(Math.abs(eye[0] ?? 0)).toBeGreaterThan(40);
    expect(Math.hypot(eye[0] ?? 0, eye[1] ?? 0)).toBeCloseTo(45, 0);
    view.recentreCount += 1;
    const back = run(rig, aircraft(), view, 600);
    expect(back.eye[1]).toBeCloseTo(-45, 0);
    expect(Math.abs(back.eye[0] ?? 99)).toBeLessThan(0.5);
  });

  it('keeps its distance setting, 40 to 150 m', () => {
    const rig = new CameraRig(COCKPIT, FLAT);
    const view = chase();
    view.chaseDistanceM = 100;
    const { eye } = run(rig, aircraft(), view, 600);
    expect(eye[1]).toBeCloseTo(-100, 1);
  });
});

describe('both cameras: the ground', () => {
  it('lifts the eye to 1.5 m above the terrain', () => {
    const high: RigTerrain = { heightAt: () => 99.5 };
    const rig = new CameraRig(COCKPIT, high);
    // The reference point is at 100 m; the cockpit eye is at 100.85, only 1.35 m above the ground.
    const pose = rig.step(aircraft(), newViewState(), 1 / 60);
    expect(pose.eyeBf[2]).toBeCloseTo(99.5 + MIN_CLEARANCE_M, 9);
    const c = new CameraRig(COCKPIT, { heightAt: () => 107.8 });
    const chase = c.step(aircraft(), { ...newViewState(), camera: 'chase' }, 1 / 60);
    expect(chase.eyeBf[2]).toBeGreaterThanOrEqual(107.8 + MIN_CLEARANCE_M - 1e-9);
  });

  it('ignores terrain it does not know (NaN)', () => {
    const rig = new CameraRig(COCKPIT, { heightAt: () => Number.NaN });
    const pose = rig.step(aircraft(), newViewState(), 1 / 60);
    expect(pose.eyeBf[2]).toBeCloseTo(100.85, 6);
  });

  it('pulls the chase camera in when a hill rises behind the aircraft', () => {
    // A wall of rock 20 m behind (south of) the aircraft, higher than the camera would be.
    const wall: RigTerrain = { heightAt: (_e, n) => (n < -20 ? 200 : 0) };
    const rig = new CameraRig(COCKPIT, wall);
    const { eye } = run(rig, aircraft(), { ...newViewState(), camera: 'chase' }, 300);
    expect(eye[1]).toBeGreaterThanOrEqual(-21);
    expect(Math.hypot(eye[0] ?? 0, eye[1] ?? 0, (eye[2] ?? 0) - 100)).toBeGreaterThanOrEqual(5.9);
  });
});

describe('K1: terrain clearance on 1 000 random low-level paths (AC-54.6)', () => {
  // The committed height grid of the rugged arena: 513 x 513 posts, 62.5 m apart.
  const posts = 513;
  const spacing = 62.5;
  const centre = [0, -4500] as const;
  const bytes = readFileSync(new URL('../../../tests/vectors/rugged_heights.bin', import.meta.url));
  const grid = new Float32Array(bytes.buffer, bytes.byteOffset, posts * posts);

  /** The grid's surface on the same triangles the renderer draws. */
  const ground: RigTerrain = {
    heightAt(east, north) {
      const fx = (east - centre[0]) / spacing + (posts - 1) / 2;
      const fy = (north - centre[1]) / spacing + (posts - 1) / 2;
      if (!(fx >= 0 && fx < posts - 1 && fy >= 0 && fy < posts - 1)) return Number.NaN;
      const i = Math.floor(fx);
      const j = Math.floor(fy);
      const [u, v] = [fx - i, fy - j];
      const at = (a: number, b: number): number => grid[b * posts + a] ?? 0;
      const [ha, hb, hc] = [at(i, j), at(i + 1, j), at(i, j + 1)];
      if (u + v <= 1) return ha + u * (hb - ha) + v * (hc - ha);
      const hd = at(i + 1, j + 1);
      return hd + (1 - u) * (hc - hd) + (1 - v) * (hb - hd);
    },
  };

  function lcg(seed: number): () => number {
    let s = seed >>> 0;
    return () => {
      s = (Math.imul(s, 1664525) + 1013904223) >>> 0;
      return s / 0x1_0000_0000;
    };
  }

  /** Attitude for a heading (clockwise from north), pitch and bank, body to BF. */
  function attitude(heading: number, pitch: number, bank: number): Float32Array {
    const f: [number, number, number] = [
      Math.cos(pitch) * Math.sin(heading),
      Math.cos(pitch) * Math.cos(heading),
      Math.sin(pitch),
    ];
    // Level right and down for this heading, then banked about the nose.
    const r0: [number, number, number] = [Math.cos(heading), -Math.sin(heading), 0];
    const d0: [number, number, number] = [
      f[1] * r0[2] - f[2] * r0[1],
      f[2] * r0[0] - f[0] * r0[2],
      f[0] * r0[1] - f[1] * r0[0],
    ];
    const [c, s] = [Math.cos(bank), Math.sin(bank)];
    const r: [number, number, number] = [
      r0[0] * c + d0[0] * s,
      r0[1] * c + d0[1] * s,
      r0[2] * c + d0[2] * s,
    ];
    const d: [number, number, number] = [
      d0[0] * c - r0[0] * s,
      d0[1] * c - r0[1] * s,
      d0[2] * c - r0[2] * s,
    ];
    return Float32Array.from(quatFromColumns(f, r, d));
  }

  it('never puts either camera below the terrain plus 1.5 m, and the chase arm never crosses it', () => {
    const rand = lcg(54_6);
    let worstEye = Number.POSITIVE_INFINITY;
    let worstArm = Number.POSITIVE_INFINITY;
    let armSamples = 0;
    let pulled = 0;
    for (let path = 0; path < 1000; path++) {
      const cockpit = new CameraRig(COCKPIT, ground);
      const chase = new CameraRig(COCKPIT, ground);
      const view = newViewState();
      const chaseView: ViewState = {
        ...newViewState(),
        camera: 'chase',
        chaseDistanceM: 40 + rand() * 110,
      };
      let east = (rand() - 0.5) * 20_000;
      let north = centre[1] + (rand() - 0.5) * 20_000;
      let heading = rand() * 2 * Math.PI;
      const speed = 40 + rand() * 260;
      let agl = 3 + rand() * rand() * 150;
      const bank = (rand() - 0.5) * 1.0;
      for (let step = 0; step < 150; step++) {
        heading += (rand() - 0.5) * 0.05;
        east += (Math.sin(heading) * speed) / 60;
        north += (Math.cos(heading) * speed) / 60;
        const g = ground.heightAt(east, north);
        if (Number.isNaN(g)) break;
        // A pilot flying low: holds a height above the ground, never under 3 m.
        agl = Math.max(3, agl + (rand() - 0.5) * 2);
        const z = g + agl;
        const state = aircraft({
          ownshipPosBf: new Float64Array([east, north, z]),
          ownshipQuat: attitude(heading, (rand() - 0.5) * 0.1, bank),
          ownshipVelBf: new Float32Array([Math.sin(heading) * speed, Math.cos(heading) * speed, 0]),
          ownshipSpeed: speed,
        });
        view.mouseYawTotal += (rand() - 0.5) * 0.2; // look around too
        view.mousePitchTotal += (rand() - 0.5) * 0.1;
        for (const [rig, v] of [
          [cockpit, view],
          [chase, chaseView],
        ] as const) {
          rig.step(state, v, 1 / 60);
          const e = rig.pose.eyeBf;
          const h = ground.heightAt(e[0] ?? 0, e[1] ?? 0);
          if (!Number.isNaN(h)) worstEye = Math.min(worstEye, (e[2] ?? 0) - h);
        }
        // The chase arm: from the aircraft to the camera, sampled every half metre.
        const e = chase.pose.eyeBf;
        const [ax, ay, az] = [(e[0] ?? 0) - east, (e[1] ?? 0) - north, (e[2] ?? 0) - z];
        const length = Math.hypot(ax, ay, az);
        if (length < chaseView.chaseDistanceM * 0.99) pulled += 1;
        for (let s = 0; s <= length; s += 0.5) {
          const f = length === 0 ? 0 : s / length;
          const h = ground.heightAt(east + ax * f, north + ay * f);
          if (Number.isNaN(h)) continue;
          armSamples += 1;
          worstArm = Math.min(worstArm, z + az * f - h);
        }
      }
    }
    expect(armSamples).toBeGreaterThan(1_000_000);
    expect(pulled).toBeGreaterThan(0); // the test does reach terrain that gets in the way
    expect(worstEye).toBeGreaterThanOrEqual(MIN_CLEARANCE_M - 1e-6);
    expect(worstArm).toBeGreaterThanOrEqual(-1e-6);
  });
});
