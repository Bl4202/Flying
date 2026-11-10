import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { quatRotate } from '@flying/sim-bridge/frame-math';
import { COMBAT_FLAG, SEEKER_MODE, WORLD } from '@flying/sim-bridge/layout';
import { WARNING } from '@flying/sim-bridge';
import { bodyToRender, FOV_Y, mul4, projection, viewRotation } from '../camera.ts';
import { bytesPerCall } from '../test-support.ts';
import { SceneBuilder } from '../scene.ts';
import { SnapshotInterpolator } from '../interp.ts';
import { makeSnapshot } from '../snapshot-fixture.ts';
import { TerrainSet } from '../terrain-levels.ts';
import { SEGMENT_FLOATS } from './graph.ts';
import { Hud, HUD_UNIFORM_FLOATS, type HudOptions } from './hud.ts';
import { DEFAULT_HUD_TEXTS } from './texts.ts';
import { attitude, DEG, input, kestrel, segmentsOf, type Scenario } from './test-support.ts';

const p = kestrel();
const rot = (q: ArrayLike<number>): Float32Array => {
  const out = new Float32Array(16);
  bodyToRender(out, q as Float64Array);
  return out;
};
const options = (over: Partial<HudOptions> = {}): HudOptions => ({
  declutter: 0,
  brightness: 1,
  palette: 'green',
  overlay: false,
  overlayScale: 1,
  widthPx: 1920,
  heightPx: 1080,
  fovY: 75 * DEG,
  eyeBf: [0, 0, 500],
  ...over,
});
const build = (hud: Hud, s: Scenario = {}, o: Partial<HudOptions> = {}): void => {
  const i = input(s);
  hud.build(i, rot(i.quatBf), options({ eyeBf: i.posBf, ...o }));
};

/** The body direction of a plane point, in BF, by the attitude. */
function bfDirection(q: ArrayLike<number>, u: number, v: number): [number, number, number] {
  const n = Math.hypot(1, u / 1000, v / 1000);
  return quatRotate(q as [number, number, number, number], [1 / n, u / 1000 / n, -v / 1000 / n]);
}
/** Local up at a BF position, independently of the HUD's own code. */
function localUp(pos: ArrayLike<number>): [number, number, number] {
  const x = pos[0] ?? 0;
  const y = pos[1] ?? 0;
  const z = (pos[2] ?? 0) + WORLD.EARTH_RADIUS_M;
  const n = Math.hypot(x, y, z);
  return [x / n, y / n, z / n];
}
type V3 = readonly [number, number, number];
const dot3 = (a: V3, b: V3): number => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
const cross3 = (a: V3, b: V3): V3 => [
  a[1] * b[2] - a[2] * b[1],
  a[2] * b[0] - a[0] * b[2],
  a[0] * b[1] - a[1] * b[0],
];
const unit3 = (a: V3): V3 => {
  const n = Math.hypot(a[0], a[1], a[2]);
  return [a[0] / n, a[1] / n, a[2] / n];
};
const elevation = (d: V3, up: V3): number => Math.asin(Math.min(Math.max(dot3(d, up), -1), 1));

function lcg(seed: number): () => number {
  let s = seed >>> 0;
  return () => {
    s = (Math.imul(s, 1664525) + 1013904223) >>> 0;
    return s / 0x1_0000_0000;
  };
}

describe('the font covers everything the HUD can write (U1)', () => {
  it('draws every character of every state of a sweep, with no glyph missing', () => {
    const hud = new Hud(p.hud, p.cockpit, p.font);
    const rand = lcg(54);
    let drawn = 0;
    for (let n = 0; n < 600; n++) {
      const s: Scenario = {
        heading: rand() * 2 * Math.PI,
        pitch: (rand() - 0.5) * Math.PI * 0.9,
        bank: (rand() - 0.5) * 2 * Math.PI,
        pos: [(rand() - 0.5) * 1e5, (rand() - 0.5) * 1e5, rand() * 20_000],
        warnings: Math.floor(rand() * 1024),
        timeS: rand() * 10,
        status: {
          cas: rand() * 400,
          mach: rand() * 2,
          altitude: rand() * 20_000 - 100,
          agl: rand() * 3000,
          vs: (rand() - 0.5) * 100,
          nz: (rand() - 0.3) * 12,
          maxNz: rand() * 9,
          nMax: 4 + rand() * 4,
          alpha: (rand() - 0.2) * 0.6,
          throttle: rand(),
          abLevel: rand() < 0.3 ? rand() : 0,
          fuel: rand() * 3000,
          fuelFlow: rand() * 3,
          gearPos: rand() < 0.5 ? 1 : 0,
          flapDetent: Math.floor(rand() * 3),
          brakeL: rand(),
          brakeR: rand(),
          assist: 1 + Math.floor(rand() * 4),
        },
        // Half the states are a duel, with every flag, seeker mode and refusal in turn (HU4).
        ...(n % 2 === 0
          ? {}
          : {
              combat: {
                combatFlags: Math.floor(rand() * 0x1000),
                seeker: Math.floor(rand() * 4),
                inhibit: Math.floor(rand() * 8),
                inhibitAge: rand() * 3,
                rounds: Math.floor(rand() * 151),
                missilesLeft: Math.floor(rand() * 3),
                tgtRange: rand() * 25_000,
                tgtClosure: (rand() - 0.5) * 600,
                pipperBf: [(rand() - 0.5) * 3000, (rand() - 0.5) * 3000, rand() * 20_000],
                banditPosBf: [(rand() - 0.5) * 3e4, (rand() - 0.5) * 3e4, rand() * 20_000],
              },
            }),
      };
      build(hud, s, { declutter: (n % 3) as 0 | 1 | 2 });
      expect(hud.graph.missing, `state ${String(n)}`).toBe(0);
      drawn += hud.graph.count;
    }
    expect(drawn).toBeGreaterThan(20_000);
  });

  it('has a glyph for every character of every fixed text', () => {
    const hud = new Hud(p.hud, p.cockpit, p.font);
    const t = DEFAULT_HUD_TEXTS;
    const words = [
      ...Object.values(t.warnings),
      ...t.assist,
      t.gear,
      t.flapsTakeOff,
      t.flapsLanding,
      t.brakes,
      t.parking,
      t.max,
      t.radar,
      t.throttle,
      t.afterburner,
      t.fuel,
      t.fuelFlow,
      t.endurance,
      t.gun,
      t.srm,
      t.arm,
      t.safe,
      t.tone,
      t.lock,
      t.shoot,
      t.inRange,
      t.hit,
      t.splash,
      ...t.inhibit,
      t.rangeLabel,
      t.closureLabel,
      'G',
      'M',
      '0123456789-.',
    ];
    for (const word of words) {
      for (const ch of word)
        expect(hud.graph.has(ch.codePointAt(0) ?? 0), `"${ch}" of "${word}"`).toBe(true);
    }
    expect(hud.graph.has(945)).toBe(true); // alpha
  });
});

describe('the HUD agrees with the world (S4 twin, AC-54.1)', () => {
  const ladderOnly = { ...p.hud, layers: p.hud.layers.filter((l) => l.type === 'pitch_ladder') };

  it('puts the ladder on the world: every line is at its elevation above the local horizon (10 000 states)', () => {
    const hud = new Hud(ladderOnly, p.cockpit, p.font);
    const rand = lcg(541);
    let checked = 0;
    let worst = 0;
    let worstHorizon = 0;
    for (let n = 0; n < 10_000; n++) {
      const s: Scenario = {
        heading: rand() * 2 * Math.PI,
        pitch: (rand() - 0.5) * Math.PI * 0.8,
        bank: (rand() - 0.5) * 2 * Math.PI,
        pos: [(rand() - 0.5) * 2e5, (rand() - 0.5) * 2e5, 100 + rand() * 12_000],
      };
      const i = input(s);
      hud.build(i, rot(i.quatBf), options({ declutter: 1, eyeBf: i.posBf }));
      const up = localUp(i.posBf);
      for (const seg of segmentsOf(hud.graph)) {
        // Only the line pieces: both ends on a 5 degree line (ticks end 6 mrad off one).
        const ends = [
          bfDirection(i.quatBf, seg[0], seg[1]),
          bfDirection(i.quatBf, seg[2], seg[3]),
        ] as const;
        const elev = ends.map((d) => elevation(d, up) / DEG);
        const nearest = elev.map((e) => Math.round(e / 5) * 5);
        const err = elev.map((e, k) => Math.abs(e - (nearest[k] ?? 0)));
        if (err.every((e) => e < 0.1) && nearest[0] === nearest[1]) {
          checked += 1;
          worst = Math.max(worst, ...err);
          if (nearest[0] === 0) worstHorizon = Math.max(worstHorizon, ...err);
        }
      }
    }
    expect(checked).toBeGreaterThan(20_000);
    // The ends of every line are on their elevation to 0.1 degrees; the horizon to 0.01.
    expect(worst).toBeLessThan(0.1);
    expect(worstHorizon).toBeLessThan(0.01);
  });

  it('centres the ladder on the aircraft heading: the horizon line points at the true heading', () => {
    const hud = new Hud(ladderOnly, p.cockpit, p.font);
    for (const heading of [0, 37, 90, 181, 300]) {
      const s: Scenario = {
        heading: heading * DEG,
        pitch: 12 * DEG,
        bank: 20 * DEG,
        pos: [0, 0, 1000],
      };
      const i = input(s);
      hud.build(i, rot(i.quatBf), options({ declutter: 1, eyeBf: i.posBf }));
      // The two innermost horizon ends (the gap's edges): their middle is the heading direction.
      const horizon = segmentsOf(hud.graph).filter((g) => {
        const e = bfDirection(i.quatBf, g[0], g[1]);
        return (
          Math.abs(elevation(e, localUp(i.posBf)) / DEG) < 0.01 &&
          Math.hypot(g[2] - g[0], g[3] - g[1]) > 50
        );
      });
      expect(horizon.length).toBeGreaterThanOrEqual(2);
      const inner = horizon.map((g) =>
        Math.hypot(g[0], g[1]) < Math.hypot(g[2], g[3]) ? [g[0], g[1]] : [g[2], g[3]],
      );
      const mid = bfDirection(
        i.quatBf,
        ((inner[0]?.[0] ?? 0) + (inner[1]?.[0] ?? 0)) / 2,
        ((inner[0]?.[1] ?? 0) + (inner[1]?.[1] ?? 0)) / 2,
      );
      // Azimuth clockwise from north in the local horizontal, with east = axis x up.
      const up = localUp(i.posBf);
      const axis: V3 = [0, Math.cos(WORLD.ANCHOR_LAT_RAD), Math.sin(WORLD.ANCHOR_LAT_RAD)];
      const east = unit3(cross3(axis, up));
      const north = cross3(up, east);
      const az = (Math.atan2(dot3(mid, east), dot3(mid, north)) / DEG + 360) % 360;
      const diff = Math.min(Math.abs(az - heading), 360 - Math.abs(az - heading));
      expect(diff, `heading ${String(heading)}`).toBeLessThan(0.05);
    }
  });

  it('puts the flight-path marker on the velocity vector (0.01 degrees, 10 000 states)', () => {
    const hud = new Hud(
      { ...p.hud, layers: p.hud.layers.filter((l) => l.type === 'flight_path_marker') },
      p.cockpit,
      p.font,
    );
    const rand = lcg(7);
    let worst = 0;
    let n = 0;
    while (n < 10_000) {
      const heading = rand() * 2 * Math.PI;
      const pitch = (rand() - 0.5) * 1.2;
      // A velocity within 12 degrees of the nose, any speed above the cage.
      const q = attitude(heading, pitch, (rand() - 0.5) * 3);
      const off: [number, number, number] = [1, (rand() - 0.5) * 0.35, (rand() - 0.5) * 0.35];
      const d = quatRotate(q, off);
      const speed = 30 + rand() * 300;
      const nrm = Math.hypot(...d);
      const vel: [number, number, number] = [
        (d[0] / nrm) * speed,
        (d[1] / nrm) * speed,
        (d[2] / nrm) * speed,
      ];
      const i = input({ heading, pitch, pos: [0, 0, 500] });
      const inp = { ...i, quatBf: q, velBf: vel };
      hud.build(inp, rot(q), options({ eyeBf: inp.posBf }));
      // The marker is the circle: its centre is the mean of its first points... use the state.
      const u = hud.state.at(15);
      const v = hud.state.at(16);
      const dir = bfDirection(q, u, v);
      const cos = (dir[0] * vel[0] + dir[1] * vel[1] + dir[2] * vel[2]) / speed;
      worst = Math.max(worst, Math.acos(Math.min(cos, 1)) / DEG);
      n += 1;
    }
    expect(worst).toBeLessThan(0.01);
  });
});

describe('collimation (S4 twin, AC-54.4)', () => {
  it('is what the shader does: a symbol is projected as a direction (w = 0), never as a point', () => {
    // The twin below models the vertex shader. This ties it to the shader's source, so that a
    // shader that put the symbols at a finite distance (w = 1) would fail here, and not only in the
    // model of it.
    const source = readFileSync(new URL('../../shaders/hud.wgsl', import.meta.url), 'utf8');
    expect(source).toContain('frame.view_proj * vec4<f32>(dir, 0.0)');
    expect(source).not.toMatch(/view_proj \* vec4<f32>\(dir, 1\.0\)/);
  });

  /** The NDC of a plane point as the shader finds it: view-projection of a direction (w = 0), or of a point at `distance` metres from the aircraft (the fault). */
  function ndc(
    viewProj: ArrayLike<number>,
    rotation: Float32Array,
    uMrad: number,
    vMrad: number,
    eyeShift: readonly number[],
    distance: number,
  ): [number, number] {
    const d: V3 = [1, uMrad / 1000, -vMrad / 1000];
    const r = [0, 1, 2].map(
      (row) =>
        (rotation[row] ?? 0) * d[0] +
        (rotation[4 + row] ?? 0) * d[1] +
        (rotation[8 + row] ?? 0) * d[2],
    ) as [number, number, number];
    // A direction has w = 0; a nearby point has w = 1 and is `distance` along it, seen from the shifted eye.
    const w = distance === 0 ? 0 : 1;
    const pos: V3 =
      distance === 0
        ? r
        : [
            r[0] * distance - (eyeShift[0] ?? 0),
            r[1] * distance - (eyeShift[1] ?? 0),
            r[2] * distance - (eyeShift[2] ?? 0),
          ];
    const c = [0, 1, 2, 3].map(
      (row) =>
        (viewProj[row] ?? 0) * pos[0] +
        (viewProj[4 + row] ?? 0) * pos[1] +
        (viewProj[8 + row] ?? 0) * pos[2] +
        (viewProj[12 + row] ?? 0) * w,
    ) as [number, number, number, number];
    return [c[0] / c[3], c[1] / c[3]];
  }

  const frameFor = () => {
    // A view-projection built exactly as the scene builds it: rotation only, whatever the eye.
    const q = attitude(0.7, 0.1, 0.2);
    const view = new Float64Array(16);
    viewRotation(view, q);
    const proj = new Float64Array(16);
    projection(proj, 75 * DEG, 16 / 9, 0.3, '0to1');
    const vp = new Float64Array(16);
    mul4(vp, proj, view);
    return { q, vp, rotation: rot(q) };
  };

  it('moves no conformal symbol by 0.05 degrees as the eye moves round the eye box (100 positions)', () => {
    const rand = lcg(99);
    const { vp, rotation } = frameFor();
    // The eye box of the Kestrel: 120 x 80 x 160 mm, so +-60 mm sideways, +-40 vertically, +-80 along.
    const box = p.hud.eyeBoxMm;
    const points: [number, number][] = [
      [0, 0],
      [60, 40],
      [-120, 80],
      [200, -100],
      [0, 120],
      [-150, -90],
    ];
    const reference = points.map(([u, v]) => ndc(vp, rotation, u, v, [0, 0, 0], 0));
    // One pixel of a 1080-line picture at 75 degrees is 0.07 degrees: 0.05 degrees in NDC is:
    const tolerance = ((0.05 * DEG) / Math.tan((75 * DEG) / 2)) * 1.0;
    let worst = 0;
    for (let n = 0; n < 100; n++) {
      const shift = [
        (rand() - 0.5) * (box[2] / 1000),
        (rand() - 0.5) * (box[0] / 1000),
        (rand() - 0.5) * (box[1] / 1000),
      ];
      points.forEach(([u, v], k) => {
        const [x, y] = ndc(vp, rotation, u, v, shift, 0);
        worst = Math.max(
          worst,
          Math.abs(x - (reference[k]?.[0] ?? 0)),
          Math.abs(y - (reference[k]?.[1] ?? 0)),
        );
      });
    }
    expect(worst).toBeLessThan(tolerance);
    expect(worst).toBe(0); // a direction at infinity does not depend on the eye at all
  });

  it('would catch symbols drawn at a finite distance: 10 m away they swim by 0.3 degrees', () => {
    const { vp, rotation } = frameFor();
    const tolerance = ((0.05 * DEG) / Math.tan((75 * DEG) / 2)) * 1.0;
    const near = ndc(vp, rotation, 0, 0, [0, 0, 0], 10);
    const moved = ndc(vp, rotation, 0, 0, [0.04, 0.04, 0.04], 10);
    expect(Math.abs(moved[0] - near[0]) + Math.abs(moved[1] - near[1])).toBeGreaterThan(
      tolerance * 2,
    );
  });

  it('builds the real frame with a rotation-only view whatever the eye does', () => {
    const interp = new SnapshotInterpolator();
    interp.push(makeSnapshot({ publishMs: 1000, ownship: { pos: [100, -3000, 800] } }));
    const state = interp.sample(2000);
    if (!state) throw new Error('no state');
    const scene = new SceneBuilder();
    const terrain = new TerrainSet();
    const a = Float32Array.from(
      scene
        .build(state, { aspect: 16 / 9, zRange: '0to1', timeS: 0, terrain })
        .uniform.subarray(0, 16),
    );
    const b = Float32Array.from(
      scene
        .build(state, {
          aspect: 16 / 9,
          zRange: '0to1',
          timeS: 0,
          terrain,
          camera: {
            mode: 'cockpit',
            eyeBf: new Float64Array([100.05, -2999.97, 800.03]),
            quatBf: new Float64Array([0, Math.SQRT1_2, Math.SQRT1_2, 0]),
            fovY: FOV_Y,
            near: 0.5,
          },
        })
        .uniform.subarray(0, 16),
    );
    for (let k = 0; k < 16; k++) expect(b[k]).toBeCloseTo(a[k] ?? 0, 5);
  });
});

describe('the uniform block', () => {
  it('is 48 floats, with the colour, size, mode, eye and glass where the shader reads them', () => {
    expect(HUD_UNIFORM_FLOATS).toBe(48);
    const hud = new Hud(p.hud, p.cockpit, p.font);
    build(hud, { heading: 0.4, pitch: 0.1 }, { widthPx: 1920, heightPx: 1080, brightness: 0.8 });
    const u = hud.uniform;
    // The rotation is body to render: a unit quaternion's matrix.
    expect(Math.hypot(u[0] ?? 0, u[1] ?? 0, u[2] ?? 0)).toBeCloseTo(1, 5);
    // The Kestrel's green, at 80 per cent.
    expect([u[16], u[17], u[18]]).toEqual([
      expect.closeTo(0x35 / 255, 5),
      1,
      expect.closeTo(0x6a / 255, 5),
    ]);
    expect(u[19]).toBeCloseTo(0.8, 6);
    expect([u[20], u[21]]).toEqual([1920, 1080]);
    // 1.2 mrad at 75 degrees over 1080 lines: 1.2e-3 * 540 / tan(37.5) = 0.84 px, floored at 1.5.
    expect(u[22]).toBeCloseTo(1.5, 6);
    expect(u[23]).toBeCloseTo(0.6, 6);
    expect(u[24]).toBe(0);
    expect(u[26]).toBeCloseTo(1920 / 1080, 6);
    // The glass: 0.275 m wide, 0.195 m tall, 4.15 m ahead of the reference point.
    expect(u[32]).toBeCloseTo(4.15, 6);
    expect(u[35]).toBeCloseTo(0.275, 6);
    expect(u[43]).toBeCloseTo(0.195, 6);
    expect(Math.hypot(u[44] ?? 0, u[45] ?? 0, u[46] ?? 0)).toBeCloseTo(1, 6);
  });

  it('puts the eye in body axes: the design eye when the camera is at it', () => {
    const hud = new Hud(p.hud, p.cockpit, p.font);
    const i = input({ heading: 1, pitch: 0.2, bank: 0.3, pos: [10, 20, 600] });
    // The camera at the design eye, moved by the attitude.
    const eyeBody = p.cockpit.eyePointM;
    const eyeBf = quatRotate(i.quatBf as [number, number, number, number], [
      eyeBody[0],
      eyeBody[1],
      eyeBody[2],
    ]);
    hud.build(i, rot(i.quatBf), options({ eyeBf: [10 + eyeBf[0], 20 + eyeBf[1], 600 + eyeBf[2]] }));
    expect(hud.uniform[28]).toBeCloseTo(eyeBody[0], 5);
    expect(hud.uniform[29]).toBeCloseTo(eyeBody[1], 5);
    expect(hud.uniform[30]).toBeCloseTo(eyeBody[2], 5);
    expect(hud.uniform[27]).toBeCloseTo(1, 6); // in the middle of the eye box
  });

  it('fades the symbols as the eye leaves the eye box (120 x 80 x 160 mm), and not inside it', () => {
    const hud = new Hud(p.hud, p.cockpit, p.font);
    const fade = (dy: number, dz = 0, dx = 0): number => {
      const i = input({ pos: [0, 0, 500] });
      // Heading north, level: body axes are (north, east, down) = BF (y, x, -z).
      const e = p.cockpit.eyePointM;
      hud.build(i, rot(i.quatBf), options({ eyeBf: [e[1] + dy, e[0] + dx, 500 - e[2] - dz] }));
      return hud.uniform[27] ?? Number.NaN;
    };
    expect(fade(0)).toBeCloseTo(1, 6);
    expect(fade(0.04)).toBeCloseTo(1, 6); // 40 mm of 60 sideways: inside
    expect(fade(0.12)).toBe(0); // 120 mm: well out
    const edge = fade(0.065);
    expect(edge).toBeGreaterThan(0);
    expect(edge).toBeLessThan(1);
    expect(fade(0, 0.03)).toBeCloseTo(1, 6); // 30 mm of 40 vertically: inside
    expect(fade(0, 0.09)).toBe(0);
  });

  it('draws as an overlay in the chase view, scaled by the setting', () => {
    const hud = new Hud(p.hud, p.cockpit, p.font);
    build(hud, {}, { overlay: true, overlayScale: 1 });
    expect(hud.uniform[24]).toBe(1);
    const one = hud.uniform[25] ?? 0;
    // The field's half-height is tan(10 degrees); it fills 45 per cent of the screen's half-height.
    expect(one).toBeCloseTo(0.45 / Math.tan(10 * DEG), 4);
    build(hud, {}, { overlay: true, overlayScale: 2 });
    expect(hud.uniform[25]).toBeCloseTo(2 * one, 5);
    build(hud, {}, { overlay: false });
    expect(hud.uniform[24]).toBe(0);
  });

  it('has five palettes: the data green, amber, white, cyan and a thick high contrast', () => {
    const hud = new Hud(p.hud, p.cockpit, p.font);
    const colour = (palette: HudOptions['palette']): number[] => {
      build(hud, {}, { palette });
      return [hud.uniform[16] ?? 0, hud.uniform[17] ?? 0, hud.uniform[18] ?? 0];
    };
    expect(colour('amber')).toEqual([1, expect.closeTo(0.7, 5), expect.closeTo(0.1, 5)]);
    expect(colour('white')).toEqual([1, 1, 1]);
    expect(colour('cyan')).toEqual([expect.closeTo(0.2, 5), expect.closeTo(0.9, 5), 1]);
    build(hud, {}, { palette: 'high-contrast', widthPx: 3840, heightPx: 2160 });
    const thick = hud.uniform[22] ?? 0;
    build(hud, {}, { palette: 'white', widthPx: 3840, heightPx: 2160 });
    expect(thick).toBeCloseTo((hud.uniform[22] ?? 0) * 1.6, 5);
  });

  it('keeps the brightness between 0 and 1', () => {
    const hud = new Hud(p.hud, p.cockpit, p.font);
    build(hud, {}, { brightness: 4 });
    expect(hud.uniform[19]).toBe(1);
    build(hud, {}, { brightness: -1 });
    expect(hud.uniform[19]).toBe(0);
  });
});

describe('a whole frame', () => {
  it('stays within its segment budget in the busiest state: every warning, every mode, all lit', () => {
    const hud = new Hud(p.hud, p.cockpit, p.font);
    build(hud, {
      pitch: 10 * DEG,
      bank: 25 * DEG,
      warnings: (1 << 10) - 1,
      timeS: 0.1,
      status: {
        cas: 255,
        mach: 0.9,
        altitude: 1000,
        agl: 400,
        nz: 7,
        nMax: 7.5,
        maxNz: 7.1,
        gearPos: 1,
        flapDetent: 2,
        brakeL: 1,
        brakeR: 1,
        abLevel: 1,
        throttle: 1,
        fuel: 1800,
        fuelFlow: 2,
        assist: 2,
      },
    });
    expect(hud.graph.count).toBeGreaterThan(300);
    expect(hud.graph.count).toBeLessThan(2500);
    expect(hud.graph.dropped).toBe(0);
  });

  it('builds in well under 0.3 ms on average (AC-54.3) and makes almost no garbage (U3)', async () => {
    const hud = new Hud(p.hud, p.cockpit, p.font);
    // In a duel (HU4), with every weapon symbol up: pipper, target box, locked seeker, status.
    const fwd: [number, number, number] = [Math.sin(1) * 0.98, Math.cos(1) * 0.98, 0.15];
    const i = input({
      heading: 1,
      pitch: 0.15,
      bank: 0.3,
      pos: [500, -4000, 900],
      warnings: WARNING.STALL | WARNING.G,
      status: { cas: 150, altitude: 900, agl: 400, nz: 3, gearPos: 1, assist: 2 },
      combat: {
        combatFlags:
          COMBAT_FLAG.ARMED |
          COMBAT_FLAG.TARGET |
          COMBAT_FLAG.PIPPER |
          COMBAT_FLAG.SHOOT_GUN |
          COMBAT_FLAG.IN_RANGE |
          COMBAT_FLAG.HIT_RECENT,
        seeker: SEEKER_MODE.LOCKED,
        rounds: 87,
        tgtRange: 812.4,
        tgtClosure: 41.2,
        pipperBf: [500 + fwd[0] * 800, -4000 + fwd[1] * 800, 900 + fwd[2] * 800 + 3.5],
        banditPosBf: [500 + fwd[0] * 812, -4000 + fwd[1] * 812, 900 + fwd[2] * 812 + 6],
      },
    });
    const r = rot(i.quatBf);
    const o = options({ eyeBf: i.posBf });
    for (let n = 0; n < 3000; n++) hud.build(i, r, o); // warm up
    const t0 = performance.now();
    const frames = 4000;
    for (let n = 0; n < frames; n++) {
      i.timeS = n / 60;
      hud.build(i, r, o);
    }
    const ms = (performance.now() - t0) / frames;
    expect(ms).toBeLessThan(0.3);
    const bytes = await bytesPerCall(
      () => {
        hud.build(i, r, o);
      },
      { rounds: 3, chunks: 21, chunkSize: 500, warmup: 20_000 },
    );
    // No object, array or string is made per frame. What is left is V8 boxing a fractional number
    // that crosses a call it does not inline (16 bytes, gone at the next young-generation
    // collection): about 250 of them a frame, 4 KB, which is 240 KB a second at 60 Hz. Measured per
    // layer on the development machine: the ladder 1.4 KB, the tapes 0.4 to 0.7 KB each, the engine
    // block 0.7 KB, the rest almost nothing. An array or an object per frame would show as a
    // multiple of that, and a string per label as much more.
    expect(bytes).toBeLessThan(8192);
  }, 30_000);

  it('writes segments the shader can read: eight floats each, the first four in milliradians', () => {
    const hud = new Hud(p.hud, p.cockpit, p.font);
    build(hud);
    const s = hud.graph.segments;
    for (let k = 0; k < hud.graph.count; k += 37) {
      const at = k * SEGMENT_FLOATS;
      for (let c = 0; c < 4; c++) expect(Math.abs(s[at + c] ?? 0)).toBeLessThan(600);
      expect(s[at + 4]).toBeGreaterThan(0);
      expect(s[at + 5]).toBeGreaterThanOrEqual(0);
      expect(s[at + 5]).toBeLessThanOrEqual(1);
    }
  });
});
