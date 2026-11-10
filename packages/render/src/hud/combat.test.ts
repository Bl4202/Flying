/**
 * The duel's HUD symbols (ADR-41 amendment): HU1's worked examples per element, and HU2's pipper
 * direction against the interpolated aim point seen from the eye.
 */
import { describe, expect, it } from 'vitest';
import { quatRotate } from '@flying/sim-bridge/frame-math';
import { COMBAT_FLAG, SEEKER_MODE } from '@flying/sim-bridge/layout';
import { SnapshotInterpolator } from '../interp.ts';
import { makeSnapshot, NORTH } from '../snapshot-fixture.ts';
import { HudGraph } from './graph.ts';
import { drawLayer, type HudContext } from './elements.ts';
import { HS, HudState, type HudCombatInput } from './state.ts';
import { DEFAULT_HUD_TEXTS } from './texts.ts';
import { input, kestrel, segmentsOf, type Scenario, type Segment } from './test-support.ts';

const p = kestrel();
const layerOf = (type: string) => {
  const l = p.hud.layers.find((x) => x.type === type);
  if (!l) throw new Error(`no ${type}`);
  return l;
};

/** Draws one layer from a scenario, keeping the words and numbers it writes. */
function drawn(
  type: string,
  s: Scenario,
  state = new HudState(),
): { segments: Segment[]; words: string[]; numbers: number[]; v: Float64Array } {
  const g = new HudGraph(p.font);
  const words: string[] = [];
  const numbers: number[] = [];
  const text = g.text.bind(g);
  const integer = g.integer.bind(g);
  const fixed = g.fixed.bind(g);
  g.text = (w, ...rest) => {
    words.push(w);
    text(w, ...rest);
  };
  g.integer = (n, ...rest) => {
    numbers.push(Math.round(n));
    integer(n, ...rest);
  };
  g.fixed = (n, d, ...rest) => {
    numbers.push(Number(n.toFixed(d)));
    fixed(n, d, ...rest);
  };
  state.update(input(s));
  const ctx: HudContext = {
    g,
    s: state.v,
    spec: p.hud,
    texts: DEFAULT_HUD_TEXTS,
    declutter: 0,
    halfU: Math.tan((14 * Math.PI) / 180) * 1000,
    halfV: Math.tan((10 * Math.PI) / 180) * 1000,
  };
  drawLayer(ctx, layerOf(type));
  return { segments: segmentsOf(g), words, numbers, v: state.v };
}

/** Level, heading north, the eye at the reference point 500 m up: a BF offset (east, north, up) of the eye is at (east / north, up / north) rad in the HUD. */
const duel = (combat: Partial<HudCombatInput>, extra: Scenario = {}): Scenario => ({
  pos: [0, 0, 500],
  combat,
  ...extra,
});
const near = (a: number, b: number, eps = 1e-6): boolean => Math.abs(a - b) <= eps;
const hasPoint = (segs: readonly Segment[], x: number, y: number, eps = 1e-6): boolean =>
  segs.some(
    (s) => (near(s[0], x, eps) && near(s[1], y, eps)) || (near(s[2], x, eps) && near(s[3], y, eps)),
  );

describe('the gun reticle (HU1)', () => {
  const pip = (flags: number, tgtRange: number) =>
    drawn(
      'gun_reticle',
      duel({ combatFlags: COMBAT_FLAG.PIPPER | flags, tgtRange, pipperBf: [10, 1000, 505] }),
    );

  it('puts the pipper on the aim point seen from the eye, and the ring round it', () => {
    const { segments, v } = pip(0, 900);
    expect(v[HS.PIP_U]).toBeCloseTo(10, 9);
    expect(v[HS.PIP_V]).toBeCloseTo(5, 9);
    // The pipper: an 8-sided circle of radius 3 about (10, 5), and its cross.
    const circle = segments.slice(0, 8);
    // Segments are f32: a micro-radian of rounding.
    for (const s of circle) expect(Math.hypot(s[0] - 10, s[1] - 5)).toBeCloseTo(3, 4);
    expect(hasPoint(segments, 8.5, 5)).toBe(true);
  });

  it('gives the ring an arc of range / ring_max_m of a turn, clockwise from the top', () => {
    // 900 of 1800 m: half a turn, from the top round the right to the bottom.
    const half = pip(0, 900).segments.slice(10);
    expect(half).toHaveLength(16);
    expect([half[0]?.[0], half[0]?.[1]]).toEqual([10, 30]);
    const last = half.at(-1);
    expect(last?.[2]).toBeCloseTo(10, 4);
    expect(last?.[3]).toBeCloseTo(-20, 4);
    for (const s of half) expect(s[2]).toBeGreaterThanOrEqual(10 - 1e-4);
    // No range: the whole ring.
    expect(pip(0, 0).segments.slice(10)).toHaveLength(32);
    // Past ring_max_m: still one turn.
    expect(pip(0, 2500).segments.slice(10)).toHaveLength(32);
  });

  it('writes SHOOT under the ring while the core says the bandit is on the pipper in range', () => {
    expect(pip(0, 900).words).toEqual([]);
    expect(pip(COMBAT_FLAG.SHOOT_GUN, 900).words).toEqual(['SHOOT']);
  });

  it('draws nothing without a solution, and nothing outside a duel', () => {
    expect(
      drawn('gun_reticle', duel({ tgtRange: 900, pipperBf: [10, 1000, 505] })).segments,
    ).toEqual([]);
    expect(drawn('gun_reticle', { pos: [0, 0, 500] }).segments).toEqual([]);
  });
});

describe('the target box (HU1)', () => {
  const box = (bandit: [number, number, number], range: number, closure = 0) =>
    drawn(
      'target_box',
      duel({
        combatFlags: COMBAT_FLAG.TARGET,
        tgtRange: range,
        tgtClosure: closure,
        banditPosBf: bandit,
      }),
    );

  it('boxes the bandit inside the field, with R in km under it and VC in knots beside it', () => {
    // 20 m east and 10 m up at 1 km: (20, 10) mrad; the box is 40 mrad a side.
    const { segments, words, numbers } = box([20, 1000, 510], 1000.4, 51.44);
    expect(hasPoint(segments, 0, -10)).toBe(true);
    expect(hasPoint(segments, 40, 30)).toBe(true);
    expect(words).toEqual(['R', 'VC']);
    expect(numbers).toEqual([1, 100]);
  });

  it('points an arrowhead at it on the locator circle when it is outside the field', () => {
    // Due east (abeam to the right): the arrow's tip at (130, 0) on the circle.
    const right = box([2000, 0, 500], 2000);
    expect(hasPoint(right.segments, 130, 0)).toBe(true);
    expect(right.numbers).toEqual([2]);
    // Straight behind and a little above: the tip at the top of the circle.
    const behind = box([0, -3000, 1500], 3200);
    expect(hasPoint(behind.segments, 0, 130, 1e-6)).toBe(true);
  });

  it('draws nothing beyond show_within_m or without a target', () => {
    expect(box([20, 21000, 510], 21000).segments).toEqual([]);
    expect(
      drawn('target_box', duel({ tgtRange: 1000, banditPosBf: [20, 1000, 510] })).segments,
    ).toEqual([]);
  });
});

describe('the seeker symbol (HU1)', () => {
  const seek = (seeker: number) =>
    drawn('seeker', duel({ seeker, banditPosBf: [20, 1000, 510] })).segments;
  const radii = (segs: readonly Segment[]) => segs.map((s) => Math.hypot(s[0], s[1]));

  it('cages a circle at the boresight while searching, doubled with the tone', () => {
    const search = seek(SEEKER_MODE.SEARCH);
    expect(search).toHaveLength(32);
    for (const r of radii(search)) expect(r).toBeCloseTo(35, 4);
    const tone = seek(SEEKER_MODE.TONE);
    expect(tone).toHaveLength(64);
    expect(Math.max(...radii(tone))).toBeGreaterThan(35.5);
    expect(seek(SEEKER_MODE.OFF)).toEqual([]);
  });

  it('puts a diamond on the bandit once locked', () => {
    const locked = seek(SEEKER_MODE.LOCKED);
    expect(locked).toHaveLength(36);
    const diamond = locked.slice(32);
    for (const [x, y] of [
      [20, 22],
      [32, 10],
      [20, -2],
      [8, 10],
    ] as const) {
      expect(hasPoint(diamond, x, y, 1e-9)).toBe(true);
    }
  });
});

describe('the weapon status (HU1)', () => {
  const status = (c: Partial<HudCombatInput>) => drawn('weapon_status', duel(c));

  it('counts the rounds and missiles, and says SAFE until armed', () => {
    const s = status({ rounds: 150, missilesLeft: 2 });
    expect(s.words).toEqual(['GUN', 'SRM', 'SAFE']);
    expect(s.numbers).toEqual([150, 2]);
  });

  it('adds the tone or the lock, then SHOOT or IN RNG', () => {
    const armed = COMBAT_FLAG.ARMED;
    expect(status({ combatFlags: armed, seeker: SEEKER_MODE.TONE }).words.slice(2)).toEqual([
      'ARM',
      'TONE',
    ]);
    expect(
      status({ combatFlags: armed | COMBAT_FLAG.IN_RANGE, seeker: SEEKER_MODE.LOCKED }).words.slice(
        2,
      ),
    ).toEqual(['ARM', 'LOCK', 'IN RNG']);
    expect(
      status({
        combatFlags: armed | COMBAT_FLAG.IN_RANGE | COMBAT_FLAG.SHOOT_MISSILE,
        seeker: SEEKER_MODE.LOCKED,
      }).words.slice(2),
    ).toEqual(['ARM', 'LOCK', 'SHOOT']);
  });

  it('names a refused launch for 1.5 s, in place of SHOOT', () => {
    const refused = (inhibitAge: number) =>
      status({
        combatFlags: COMBAT_FLAG.ARMED | COMBAT_FLAG.SHOOT_MISSILE,
        seeker: SEEKER_MODE.LOCKED,
        inhibit: 4,
        inhibitAge,
      }).words.slice(2);
    expect(refused(0.4)).toEqual(['ARM', 'LOCK', 'TOO CLOSE']);
    expect(refused(1.6)).toEqual(['ARM', 'LOCK', 'SHOOT']);
    const words = [1, 2, 3, 4, 5, 6, 7].map((inhibit) => status({ inhibit }).words[3]);
    expect(words).toEqual([
      'SAFE',
      'EMPTY',
      'NO LOCK',
      'TOO CLOSE',
      'G LIMIT',
      'AOA LIMIT',
      'SLOW',
    ]);
  });
});

describe('the kill cue (HU1)', () => {
  it('shows HIT for 0.3 s from a hit and SPLASH for 2.0 s from a kill', () => {
    const state = new HudState();
    const at = (timeS: number, combatFlags: number): string[] =>
      drawn('kill_cue', duel({ combatFlags }, { timeS }), state).words;
    const hit = COMBAT_FLAG.HIT_RECENT;
    const kill = COMBAT_FLAG.KILL_RECENT;
    expect(at(0, 0)).toEqual([]);
    expect(at(1, hit)).toEqual(['HIT']);
    expect(at(1.29, hit)).toEqual(['HIT']);
    expect(at(1.31, hit)).toEqual([]);
    expect(at(1.4, 0)).toEqual([]);
    expect(at(2, hit | kill)).toEqual(['SPLASH']);
    expect(at(3.99, kill)).toEqual(['SPLASH']);
    expect(at(4.01, kill)).toEqual([]);
    // A new hit after the latch dropped starts again.
    expect(at(4.5, hit)).toEqual(['HIT']);
  });
});

describe('the pipper follows the interpolated aim point (HU2)', () => {
  it('is drawn in the direction from the eye to C_PIPPER_BF within 0.05 mrad', () => {
    // Far from the origin, between two snapshots, from a cockpit eye off the reference point.
    const base: [number, number, number] = [380_000, -260_000, 4200];
    const snap = (k: number, publishMs: number) =>
      makeSnapshot({
        tick: k,
        publishMs,
        ownship: { pos: [base[0], base[1] + 3.3 * k, base[2] + 0.1 * k], quat: NORTH },
        combat: {
          flags: COMBAT_FLAG.ACTIVE | COMBAT_FLAG.PIPPER,
          pipper: [base[0] + 12 + 0.4 * k, base[1] + 900 + 5 * k, base[2] + 31 - 0.2 * k],
        },
      });
    const interp = new SnapshotInterpolator();
    interp.push(snap(1, 1000));
    interp.push(snap(2, 1000 + 1000 / 60));
    const state = interp.sample(1000 + 1000 / 60 + 1000 / 120);
    if (!state) throw new Error('no state');
    expect(interp.status.alpha).toBeCloseTo(0.5, 6);
    const eye: [number, number, number] = [
      (state.ownshipPosBf[0] ?? 0) + 0.2,
      (state.ownshipPosBf[1] ?? 0) + 4.1,
      (state.ownshipPosBf[2] ?? 0) + 0.9,
    ];
    const hud = new HudState();
    hud.update({
      posBf: state.ownshipPosBf,
      quatBf: state.ownshipQuat,
      velBf: [0, 200, 0],
      status: input().status,
      warnings: 0,
      timeS: 0,
      eyeBf: eye,
      combat: state,
    });
    // Independently: the BF offset in f64, into body axes by the conjugate attitude.
    const q = state.ownshipQuat;
    const conj: [number, number, number, number] = [
      q[0] ?? 1,
      -(q[1] ?? 0),
      -(q[2] ?? 0),
      -(q[3] ?? 0),
    ];
    const d = quatRotate(conj, [
      (state.pipperBf[0] ?? 0) - eye[0],
      (state.pipperBf[1] ?? 0) - eye[1],
      (state.pipperBf[2] ?? 0) - eye[2],
    ]);
    const u = (d[1] / d[0]) * 1000;
    const v = (-d[2] / d[0]) * 1000;
    expect(Math.abs((hud.v[HS.PIP_U] ?? 0) - u)).toBeLessThan(0.05);
    expect(Math.abs((hud.v[HS.PIP_V] ?? 0) - v)).toBeLessThan(0.05);
  });
});
