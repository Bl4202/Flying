/**
 * `?selftest=screens&case=<group>`: the screenshot gates of ADR-39 (S1 to S5), measured in this
 * browser on the real render path. The page starts the Sim Worker for the chosen arena (it fills the
 * four terrain levels from the sim's own terrain function and streams them to the renderer, exactly as
 * in the game), the production Render Worker, and publishes fixture snapshots at a steady rate: no
 * flight and no clock, so every case is the same picture every time. Each case reads the frame back,
 * measures it, and reports a row with its numbers:
 *
 * | group | gates |
 * |---|---|
 * | `horizon` | S1 the dip at five heights; S2 the curvature across the frame, under roll and at 100 degrees |
 * | `terrain` | S5 no sky-coloured crack in the terrain across level boundaries |
 * | `hud` | S4 the HUD's zero line sits above the rendered horizon by the dip; the HUD stays in its glass; the overlay in the chase view; the detail levels |
 *
 * Every row is a cell with a test id and a `data-result` of `pass` or `fail`; the numbers are in its
 * text. With `?debug=1&fault=flat-earth` the S1 and S2 rows are expected to fail (S3).
 */
import {
  CLASSIFY_KEYS,
  countKey,
  elevationRow,
  expectedHorizonRow,
  fittedAngleDeg,
  horizonDipDeg,
  lumaAt,
  rgbAt,
  RenderClient,
  rmsAndMax,
  rowElevationDeg,
  strongestStepRow,
  type RgbaImage,
} from '@flying/render';
import { SHADERS } from '@flying/render/shader-sources';
import { attitudeQuat, makeSnapshot } from '@flying/render/snapshot-fixture';
import renderWorkerUrl from '@flying/render/render.worker?worker&url';
import simWorkerUrl from '@flying/sim-host/sim.worker?worker&url';
import { sharedNowMs, type Flags } from '@flying/platform-web';
import { createWakeWord, SimClient } from '@flying/sim-host';
import { ARENA, INPUT_SCRIPT, SNAPSHOT, WORLD } from '@flying/sim-bridge/layout';
import {
  makeInputChannel,
  makeSnapshotChannel,
  newViewState,
  openSnapshotSink,
} from '@flying/sim-bridge';
import { coreContentUrl, wasmUrl } from './selftest.ts';
import { SEED } from './selftest-judge.ts';
import { loadPresentBundle } from './present-bundle.ts';
import { loadPresentation, type LoadedPresentation } from './presentation.ts';
import { mountSelfTestTable, type SelfTestTable } from './selftest-table.ts';

const WIDTH = 1280;
const HEIGHT = 720;
const DEG = Math.PI / 180;

/** Where an eye is and which way it looks (east, north, height above the tangent plane; degrees). */
interface Pose {
  pos: [number, number, number];
  heading: number;
  pitch: number;
  roll: number;
  /** Vertical field of view, degrees (default 70). */
  fovDeg?: number;
}

const sleep = (ms: number): Promise<void> => new Promise((r) => setTimeout(r, ms));

/** The gates' numbers. */
export const GATES = {
  /** S1: the horizon's dip, degrees. */
  dipDeg: 0.05,
  /** S2: the horizon's image, pixels. */
  curveRmsPx: 0.5,
  curveMaxPx: 1,
  /** S2: the fitted angle under roll, degrees. */
  rollDeg: 0.1,
  /** S2: the sag between the middle and the edges, pixels. */
  sagPx: 1,
} as const;

/** The plain's base height, metres (`plain`: 40 m with a few metres of noise). */
const PLAIN_BASE_M = 40;

interface Rig {
  render: RenderClient;
  /** Puts the eye at `pose` and returns the picture once it has stopped changing. */
  capture(pose: Pose): Promise<RgbaImage>;
  stop(): void;
}

function sameImage(a: RgbaImage, b: RgbaImage): boolean {
  if (a.width !== b.width || a.height !== b.height) return false;
  for (let i = 0; i < a.pixels.length; i += 7) if (a.pixels[i] !== b.pixels[i]) return false;
  return true;
}

/**
 * With `&dump=<id>` in the URL, the picture behind that row is put on the page as a PNG (an `<img>`
 * with the test id `dump-<id>`), so a person or a script can look at what was measured.
 */
function dumpIfAsked(id: string, img: RgbaImage): void {
  if (new URLSearchParams(location.search).get('dump') !== id) return;
  const canvas = document.createElement('canvas');
  canvas.width = img.width;
  canvas.height = img.height;
  const context = canvas.getContext('2d');
  if (!context) return;
  context.putImageData(
    new ImageData(new Uint8ClampedArray(img.pixels), img.width, img.height),
    0,
    0,
  );
  const el = document.createElement('img');
  el.dataset['testid'] = `dump-${id}`;
  el.src = canvas.toDataURL('image/png');
  el.alt = id;
  document.body.append(el);
}

async function startRig(flags: Flags, arena: number, cameraSettings: boolean): Promise<Rig> {
  const isolated = typeof crossOriginIsolated !== 'undefined' && crossOriginIsolated;
  const kind = isolated ? 'shared' : 'message';
  const simSnapshots = makeSnapshotChannel(kind);
  const drawSnapshots = makeSnapshotChannel(kind);
  const inputs = makeInputChannel(isolated ? 'shared' : 'message');
  const terrain = new MessageChannel();
  const stage = document.getElementById('stage');
  if (!stage) throw new Error('SYS-010: #stage is missing from the page');

  const sim = await SimClient.create({
    workerUrl: simWorkerUrl,
    init: {
      variant: 'single',
      wasmUrl: wasmUrl('single'),
      threadsInitialPages: __THREADS_INITIAL_PAGES__,
      contentUrl: coreContentUrl,
      seed: SEED,
      script: INPUT_SCRIPT.NONE,
      session: { spawn: 0, assist: 2, gEffects: 2, arena },
      snapshot: simSnapshots.sink,
      input: inputs.source,
      terrainPort: terrain.port1,
      wake: isolated ? createWakeWord() : null,
      forceTimer: true,
      profiling: false,
      allowStall: false,
    },
    onStats: () => undefined,
    onError: (message) => {
      throw new Error(message);
    },
  });
  const render = await RenderClient.create({
    container: stage,
    workerUrl: renderWorkerUrl,
    flags: {
      renderer: flags.renderer,
      mainrender: flags.mainrender,
      stall: false,
      res: { width: WIDTH, height: HEIGHT },
      quality: 'low',
      fault: flags.fault,
      view: flags.view,
    },
    snapshot: drawSnapshots.source,
    terrainPort: terrain.port2,
    viewBuffer: null,
    onEvent: () => undefined,
    shaders: SHADERS,
  });

  const sink = openSnapshotSink(drawSnapshots.sink);
  let pose: Pose = { pos: [0, 0, 1000], heading: 0, pitch: 0, roll: 0 };
  let tick = 1;
  const publish = (): void => {
    sink.publish(
      makeSnapshot({
        tick: tick++,
        arena,
        ownship: {
          pos: pose.pos,
          quat: attitudeQuat(pose.heading, pose.pitch, pose.roll),
          speed: 0,
          assist: cameraSettings ? 2 : 0,
        },
      }),
      (copy) => {
        new DataView(copy.buffer, copy.byteOffset).setFloat64(
          SNAPSHOT.H_PUBLISH_TIME_MS,
          sharedNowMs(),
          true,
        );
      },
    );
  };
  publish();
  const timer = window.setInterval(publish, 33);

  // The sim fills every level at start-up; give them time to reach the renderer.
  await sleep(1500);

  return {
    render,
    async capture(next: Pose): Promise<RgbaImage> {
      pose = next;
      render.setSettings({ gfxStrength: 100, debugFovDeg: next.fovDeg ?? 70 });
      await sleep(400);
      let prev = await render.readImage();
      for (let i = 0; i < 25; i++) {
        await sleep(120);
        const now = await render.readImage();
        if (sameImage(prev, now)) return now;
        prev = now;
      }
      return prev;
    },
    stop(): void {
      window.clearInterval(timer);
      render.stop();
      sim.stop();
      sink.close();
    },
  };
}

const fixed = (v: number, digits = 3): string => v.toFixed(digits);

// ---------------------------------------------------------------------------------------------
// S1 and S2.

async function horizon(rig: Rig, table: SelfTestTable): Promise<void> {
  const radius = WORLD.EARTH_RADIUS_M;
  const pose = (h: number, over: Partial<Pose> = {}): Pose => ({
    pos: [0, 0, PLAIN_BASE_M + h],
    heading: 0,
    pitch: 0,
    roll: 0,
    ...over,
  });

  // S1: the dip at the centre, at five heights above the plain's base.
  for (const h of [100, 1000, 5000, 10_000, 15_000]) {
    const img = await rig.capture(pose(h));
    dumpIfAsked(`s1-${String(h)}`, img);
    const dip = horizonDipDeg(radius + PLAIN_BASE_M, h);
    const fov = 70 * DEG;
    const near = elevationRow(-dip, HEIGHT, fov);
    const window = 45;
    const a = strongestStepRow(img, WIDTH / 2 - 1, near - window, near + window);
    const b = strongestStepRow(img, WIDTH / 2, near - window, near + window);
    if (a === undefined || b === undefined) {
      const above = Math.round(near - 12);
      const below = Math.round(near + 12);
      table.row(
        `s1-${String(h)}`,
        `S1 dip at ${String(h)} m`,
        `fail: no horizon found (image ${String(img.width)}x${String(img.height)}, ${String(img.pixels.length)} bytes, rows ${String(above)}/${String(below)} luma ${fixed(lumaAt(img, WIDTH / 2, above), 1)}/${fixed(lumaAt(img, WIDTH / 2, below), 1)}, window ${String(near - window)}..${String(near + window)})`,
        'fail',
      );
      continue;
    }
    const measured = -rowElevationDeg((a + b) / 2, HEIGHT, fov);
    const error = measured - dip;
    const ok = Math.abs(error) <= GATES.dipDeg;
    table.row(
      `s1-${String(h)}`,
      `S1 dip at ${String(h)} m`,
      `${ok ? 'pass' : 'fail'}: measured ${fixed(measured)} deg, expected ${fixed(dip)} deg, error ${fixed(error)} deg (limit ${String(GATES.dipDeg)})`,
      ok ? 'pass' : 'fail',
    );
  }

  // S2: the horizon across the frame matches the image of the horizon circle.
  const curve = async (
    id: string,
    label: string,
    h: number,
    over: Partial<Pose>,
  ): Promise<void> => {
    const p = pose(h, over);
    const img = await rig.capture(p);
    dumpIfAsked(id, img);
    const fovYRad = (p.fovDeg ?? 70) * DEG;
    const cam = {
      width: WIDTH,
      height: HEIGHT,
      fovYRad,
      dipDeg: horizonDipDeg(radius + PLAIN_BASE_M, h),
      rollDeg: p.roll,
    };
    const errors: number[] = [];
    const measuredPoints: [number, number][] = [];
    const expectedPoints: [number, number][] = [];
    for (let k = 0; k < 33; k++) {
      const x = Math.round(40 + (k * (WIDTH - 80)) / 32);
      const want = expectedHorizonRow(cam, x);
      if (want === undefined || want < 30 || want > HEIGHT - 30) continue;
      const got = strongestStepRow(img, x, want - 30, want + 30);
      if (got === undefined) continue;
      errors.push(got - want);
      measuredPoints.push([x, got]);
      expectedPoints.push([x, want]);
    }
    if (errors.length < 20) {
      table.row(
        id,
        label,
        `fail: the horizon was found in only ${String(errors.length)} of 33 columns`,
        'fail',
      );
      return;
    }
    const { rms, max } = rmsAndMax(errors);
    const middle = measuredPoints.filter(([x]) => Math.abs(x - WIDTH / 2) < 250);
    const wantMiddle = expectedPoints.filter(([x]) => Math.abs(x - WIDTH / 2) < 250);
    const angle = fittedAngleDeg(middle);
    const wantAngle = fittedAngleDeg(wantMiddle);
    const rollOk = Math.abs(angle - wantAngle) <= GATES.rollDeg;
    const first = measuredPoints[0];
    const last = measuredPoints.at(-1);
    const centre = measuredPoints.reduce((best, pt) =>
      Math.abs(pt[0] - WIDTH / 2) < Math.abs(best[0] - WIDTH / 2) ? pt : best,
    );
    const wantCentre = expectedPoints.find(([x]) => x === centre[0]);
    const wantFirst = expectedPoints[0];
    const wantLast = expectedPoints.at(-1);
    let sagOk = true;
    let sagText = '';
    if (p.roll === 0 && first && last && wantCentre && wantFirst && wantLast) {
      const sag = (first[1] + last[1]) / 2 - centre[1];
      const wantSag = (wantFirst[1] + wantLast[1]) / 2 - wantCentre[1];
      sagOk = Math.abs(sag - wantSag) <= GATES.sagPx;
      sagText = `, sag ${fixed(sag, 2)} px (expected ${fixed(wantSag, 2)})`;
    }
    const ok = rms <= GATES.curveRmsPx && max <= GATES.curveMaxPx && rollOk && sagOk;
    table.row(
      id,
      label,
      `${ok ? 'pass' : 'fail'}: ${String(errors.length)} columns, rms ${fixed(rms, 2)} px, max ${fixed(max, 2)} px${sagText}, angle ${fixed(angle, 2)} deg (expected ${fixed(wantAngle, 2)}, limit ${String(GATES.rollDeg)})`,
      ok ? 'pass' : 'fail',
    );
  };
  await curve('s2-curve-10000', 'S2 curvature at 10 km', 10_000, {});
  await curve('s2-curve-15000', 'S2 curvature at 15 km', 15_000, {});
  await curve('s2-roll-10000', 'S2 under 30 degrees of roll at 10 km', 10_000, { roll: 30 });
  await curve('s2-fov-10000', 'S2 at 100 degrees of field of view at 10 km', 10_000, {
    fovDeg: 100,
  });
}

// ---------------------------------------------------------------------------------------------
// S5.

/** The edges of the terrain levels' footprints, metres from the middle (`terrain-levels.ts`). */
const LEVEL_EDGES_M = [16_000, 64_000, 256_000] as const;
/** How far inside the edge the eye stands, and how high, metres. */
const SEAM_AHEAD_M = 2500;
const SEAM_HEIGHT_M = 2500;

async function terrain(rig: Rig, table: SelfTestTable): Promise<void> {
  let total = 0;
  let views = 0;
  for (const edge of LEVEL_EDGES_M) {
    for (const heading of [0, 90, 180, 270]) {
      // Steeply down, so that the seam 2.5 km ahead (45 degrees below the level at 2.5 km up) is
      // inside the picture, with ground on both sides of it.
      for (const pitch of [-30, -45]) {
        const out = edge - SEAM_AHEAD_M;
        const east = Math.sin(heading * DEG) * out;
        const north = Math.cos(heading * DEG) * out;
        // BF height is above the plane the sphere touches at the anchor: out there the ground has
        // fallen away by `out^2 / 2R` (5 km at 250 km), so the eye is lowered by that to be 2.5 km
        // above the ground.
        const drop = WORLD.EARTH_RADIUS_M - Math.sqrt(WORLD.EARTH_RADIUS_M ** 2 - out ** 2);
        const img = await rig.capture({
          pos: [east, north, SEAM_HEIGHT_M - drop],
          heading,
          pitch,
          roll: 0,
        });
        const globe = countKey(img.pixels, CLASSIFY_KEYS.globe);
        const id = `s5-${String(edge / 1000)}-${String(heading)}-${String(-pitch)}`;
        dumpIfAsked(id, img);
        // Both levels either side of the seam are in the picture: two ground colours.
        const perLevel = CLASSIFY_KEYS.terrain.map((key) => countKey(img.pixels, key));
        const levels = perLevel.filter((n) => n > 0).length;
        views += 1;
        total += globe;
        table.row(
          id,
          `S5 ${String(edge / 1000)} km edge, heading ${String(heading)}, pitch ${String(pitch)}`,
          `${globe === 0 ? 'pass' : 'fail'}: ${String(globe)} globe pixels, ${String(levels)} terrain levels in view (L0..L3 pixels ${perLevel.join('/')})`,
          globe === 0 ? 'pass' : 'fail',
        );
      }
    }
  }
  table.row(
    's5-total',
    'S5 no crack in any view',
    `${total === 0 ? 'pass' : 'fail'}: ${String(total)} globe pixels in ${String(views)} views`,
    total === 0 ? 'pass' : 'fail',
  );
}

// ---------------------------------------------------------------------------------------------
// S4.

/** Whether pixel `(x, y)` is exactly `key`. */
function isKeyAt(
  img: RgbaImage,
  x: number,
  y: number,
  key: readonly [number, number, number],
): boolean {
  const [r, g, b] = rgbAt(img, x, y);
  return r === key[0] && g === key[1] && b === key[2];
}

/** Whether `(x, y)` is a terrain or globe key: the ground side of the horizon. */
function isGround(img: RgbaImage, x: number, y: number): boolean {
  return (
    isKeyAt(img, x, y, CLASSIFY_KEYS.globe) ||
    CLASSIFY_KEYS.terrain.some((key) => isKeyAt(img, x, y, key))
  );
}

/**
 * The row where sky turns to ground in column `x`, searching `from` to `to` (continuous rows). HUD
 * pixels between the two (the line can sit right on the horizon) are looked through.
 */
function skyToGroundRow(img: RgbaImage, x: number, from: number, to: number): number | undefined {
  for (let y = Math.max(1, Math.floor(from)); y < Math.min(img.height - 1, Math.ceil(to)); y++) {
    if (!isGround(img, x, y)) continue;
    let k = y - 1;
    while (k > 0 && isKeyAt(img, x, k, CLASSIFY_KEYS.hud)) k -= 1;
    if (isKeyAt(img, x, k, CLASSIFY_KEYS.sky)) return y;
  }
  return undefined;
}

/** The centre row of the HUD pixels in column `x` nearest `near` (within `reach` rows), or undefined. */
function hudRow(img: RgbaImage, x: number, near: number, reach: number): number | undefined {
  const rows: number[] = [];
  for (
    let y = Math.max(0, Math.floor(near - reach));
    y <= Math.min(img.height - 1, near + reach);
    y++
  ) {
    if (isKeyAt(img, x, y, CLASSIFY_KEYS.hud)) rows.push(y);
  }
  if (rows.length === 0) return undefined;
  // Group contiguous rows; take the group whose centre is nearest.
  const groups: number[][] = [];
  for (const y of rows) {
    const last = groups.at(-1);
    if (last !== undefined && y - (last.at(-1) ?? y) <= 1) last.push(y);
    else groups.push([y]);
  }
  // A group of rows `a` to `b` spans `a` to `b + 1`; its middle is the centre.
  const centre = (g: number[]): number => ((g[0] ?? 0) + (g.at(-1) ?? 0) + 1) / 2;
  let best = groups[0] ?? [];
  for (const g of groups) {
    if (Math.abs(centre(g) - near) < Math.abs(centre(best) - near)) best = g;
  }
  return centre(best);
}

/** The HUD key pixels of an image: their count and bounding box. */
function hudExtent(img: RgbaImage): {
  count: number;
  x0: number;
  x1: number;
  y0: number;
  y1: number;
} {
  let count = 0;
  let x0 = img.width;
  let x1 = -1;
  let y0 = img.height;
  let y1 = -1;
  for (let y = 0; y < img.height; y++) {
    for (let x = 0; x < img.width; x++) {
      if (!isKeyAt(img, x, y, CLASSIFY_KEYS.hud)) continue;
      count += 1;
      x0 = Math.min(x0, x);
      x1 = Math.max(x1, x);
      y0 = Math.min(y0, y);
      y1 = Math.max(y1, y);
    }
  }
  return { count, x0, x1, y0, y1 };
}

/** Rotates `v` by the quaternion `[w, x, y, z]`. */
function rotate(
  q: [number, number, number, number],
  v: [number, number, number],
): [number, number, number] {
  const [w, x, y, z] = q;
  const [vx, vy, vz] = v;
  const tx = 2 * (y * vz - z * vy);
  const ty = 2 * (z * vx - x * vz);
  const tz = 2 * (x * vy - y * vx);
  return [
    vx + w * tx + (y * tz - z * ty),
    vy + w * ty + (z * tx - x * tz),
    vz + w * tz + (x * ty - y * tx),
  ];
}

/** The HUD's gates, degrees. */
const HUD_GATES = { lineDeg: 0.1, dipDeg: 0.1 } as const;
/** The eye's height above the ground for the HUD cases, metres. */
const HUD_EYE_HEIGHT_M = 30;

async function hud(rig: Rig, table: SelfTestTable, loaded: LoadedPresentation): Promise<void> {
  const presentation = loaded.render;
  const cockpit = loaded.cockpit;
  if (presentation === undefined || cockpit === undefined) {
    table.row('s4-load', 'S4 the presentation', `fail: ${loaded.error ?? 'not loaded'}`, 'fail');
    return;
  }
  rig.render.setPresentation(presentation);
  rig.render.setCockpit(cockpit);
  // No shake: the cockpit camera buffets and rattles by design, and both the HUD and the horizon
  // would move with it, so the absolute position of the HUD line could not be measured.
  const view = { ...newViewState(), shake: 0 };
  rig.render.setView(view);
  const fovYRad = view.fovDeg * DEG;
  const radius = WORLD.EARTH_RADIUS_M;

  // S4: the HUD's zero line against the rendered horizon, bank zero, heading and pitch varied.
  const cases: [number, number][] = [
    [0, 0],
    [140, 0],
    [280, 0],
    [0, -3],
    [140, -3],
    [280, -3],
    [0, -6],
    [140, -6],
    [0, 2],
    [280, 2],
  ];
  let worst = 0;
  for (const [heading, pitch] of cases) {
    const q = attitudeQuat(heading, pitch, 0);
    const offset = rotate(q, [...cockpit.eyeBodyM] as [number, number, number]);
    // The eye is `HUD_EYE_HEIGHT_M` above the ground wherever the pitch puts it.
    const z = PLAIN_BASE_M + HUD_EYE_HEIGHT_M - offset[2];
    const id = `s4-${String(heading)}-${String(pitch)}`;
    const img = await rig.capture({ pos: [0, 0, z], heading, pitch, roll: 0 });
    dumpIfAsked(id, img);
    const dip = horizonDipDeg(radius + PLAIN_BASE_M, HUD_EYE_HEIGHT_M);
    const wantHud = elevationRow(-pitch, HEIGHT, fovYRad);
    const wantHorizon = elevationRow(-pitch - dip, HEIGHT, fovYRad);
    const lines: number[] = [];
    const edges: number[] = [];
    for (const x of [WIDTH / 2 - 45, WIDTH / 2 - 30, WIDTH / 2 + 30, WIDTH / 2 + 45]) {
      const h = hudRow(img, x, wantHud, 8);
      const e = skyToGroundRow(img, x, wantHorizon - 12, wantHorizon + 12);
      if (h !== undefined && e !== undefined) {
        lines.push(h);
        edges.push(e);
      }
    }
    if (lines.length < 3) {
      table.row(
        id,
        `S4 heading ${String(heading)}, pitch ${String(pitch)}`,
        `fail: found the HUD line and the horizon in only ${String(lines.length)} of 4 columns`,
        'fail',
      );
      continue;
    }
    const mean = (v: number[]): number => v.reduce((a, b) => a + b, 0) / v.length;
    const hudAt = mean(lines);
    const horizonAt = mean(edges);
    const lineErr = rowElevationDeg(hudAt, HEIGHT, fovYRad) - -pitch;
    const gap =
      rowElevationDeg(hudAt, HEIGHT, fovYRad) - rowElevationDeg(horizonAt, HEIGHT, fovYRad);
    const dipErr = gap - dip;
    worst = Math.max(worst, Math.abs(dipErr));
    const ok = Math.abs(lineErr) <= HUD_GATES.lineDeg && Math.abs(dipErr) <= HUD_GATES.dipDeg;
    table.row(
      id,
      `S4 heading ${String(heading)}, pitch ${String(pitch)}`,
      `${ok ? 'pass' : 'fail'}: HUD line ${fixed(lineErr)} deg from the level, above the horizon by ${fixed(gap)} deg (dip ${fixed(dip)}, error ${fixed(dipErr)}, limit ${String(HUD_GATES.dipDeg)})`,
      ok ? 'pass' : 'fail',
    );
  }
  table.row('s4-worst', 'S4 worst dip error', `info: ${fixed(worst)} deg`, 'info');

  // The HUD stays inside its glass: no HUD pixel further from the boresight than the glass allows.
  const level = await rig.capture({
    pos: [0, 0, PLAIN_BASE_M + HUD_EYE_HEIGHT_M],
    heading: 0,
    pitch: 0,
    roll: 0,
  });
  const tfov = presentation.hud.tfovDeg;
  const e = hudExtent(level);
  const halfW =
    (Math.atan(((e.x1 - e.x0 + 1) / 2 / (HEIGHT / 2)) * Math.tan(fovYRad / 2)) / DEG) * 1;
  const halfH =
    (Math.atan(((e.y1 - e.y0 + 1) / 2 / (HEIGHT / 2)) * Math.tan(fovYRad / 2)) / DEG) * 1;
  const inside = e.count > 200 && halfW * 2 <= tfov[0] + 1 && halfH * 2 <= tfov[1] + 1;
  table.row(
    's4-bounds',
    'S4 the HUD is inside its true field of view',
    `${inside ? 'pass' : 'fail'}: ${String(e.count)} HUD pixels, ${fixed(halfW * 2, 1)} x ${fixed(halfH * 2, 1)} deg (field ${String(tfov[0])} x ${String(tfov[1])})`,
    inside ? 'pass' : 'fail',
  );

  // Detail levels: fewer symbols at each step.
  const counts: number[] = [];
  for (const level of [0, 1, 2]) {
    rig.render.setView({ ...view, hudDeclutter: level });
    counts.push(
      hudExtent(
        await rig.capture({
          pos: [0, 0, PLAIN_BASE_M + HUD_EYE_HEIGHT_M],
          heading: 0,
          pitch: 0,
          roll: 0,
        }),
      ).count,
    );
  }
  const stepped =
    (counts[0] ?? 0) > (counts[1] ?? 0) &&
    (counts[1] ?? 0) > (counts[2] ?? 0) &&
    (counts[2] ?? 0) > 0;
  table.row(
    's4-declutter',
    'S4 the detail levels take symbols away',
    `${stepped ? 'pass' : 'fail'}: HUD pixels ${counts.join(' > ')}`,
    stepped ? 'pass' : 'fail',
  );
  rig.render.setView(view);

  // The chase view: the HUD is drawn over the picture, centred.
  rig.render.setView({ ...view, camera: 'chase' });
  const chase = hudExtent(
    await rig.capture({ pos: [0, 0, PLAIN_BASE_M + 500], heading: 0, pitch: 0, roll: 0 }),
  );
  const centred = Math.abs((chase.x0 + chase.x1) / 2 - WIDTH / 2) < WIDTH * 0.1;
  const overlay = chase.count > 200 && centred;
  table.row(
    's4-overlay',
    'S4 the HUD is drawn over the chase view',
    `${overlay ? 'pass' : 'fail'}: ${String(chase.count)} HUD pixels, centred at column ${fixed((chase.x0 + chase.x1) / 2, 0)}`,
    overlay ? 'pass' : 'fail',
  );
  rig.render.setView(view);
}

// ---------------------------------------------------------------------------------------------

export interface ScreensSelfTestArgs {
  root: HTMLElement;
  flags: Flags;
}

export async function runScreensSelfTest(args: ScreensSelfTestArgs): Promise<void> {
  const table = mountSelfTestTable(args.root);
  const group = new URLSearchParams(location.search).get('case') ?? 'horizon';
  // Only a classify view tells the globe from the sky by colour.
  const flags: Flags =
    (group === 'terrain' || group === 'hud') && args.flags.view === false
      ? { ...args.flags, view: 'classify' }
      : args.flags;
  let rig: Rig | undefined;
  try {
    const arena = args.flags.arena === 'rugged' || group === 'terrain' ? ARENA.RUGGED : ARENA.PLAIN;
    rig = await startRig(flags, arena, group === 'hud');
    if (group === 'terrain') await terrain(rig, table);
    else if (group === 'hud') {
      const loaded = await loadPresentation({
        bundle: () => loadPresentBundle(),
        pack: async () => new Uint8Array(await (await fetch(coreContentUrl)).arrayBuffer()),
      });
      await hud(rig, table, loaded);
    } else await horizon(rig, table);
  } catch (e) {
    table.row('error', 'Error', `fail: ${e instanceof Error ? e.message : String(e)}`, 'fail');
  } finally {
    rig?.stop();
  }
  table.done();
}
