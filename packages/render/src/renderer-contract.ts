/**
 * What every renderer backend must do (`AC-61.5`, `AC-66.2`), as a function that returns its
 * results instead of asserting, so the same checks run under Vitest against the null renderer and
 * in the browser against WebGPU and WebGL2 (the `renderer` end-to-end spec, and `?selftest=renderer`).
 *
 * The drawing checks look at pixels: sky only when looking up; terrain, fog and markers when
 * looking at the world; the same picture twice; and, the one that matters for the exit line, the
 * same picture when the whole scene is moved 400 km, which holds only if positions are made
 * relative to the eye before they are rounded to f32. The pilot's body has its checks too: a
 * stressed pilot sees a dark picture, a pilot in red-out a red one, the effect strength switches
 * them off; and the runway paints its asphalt and its paint, and the mouse-aim cues show.
 */
import { SnapshotInterpolator } from './interp.ts';
import { CLEAR_COLOUR, type Renderer } from './renderer.ts';
import { airfieldShifted, SceneBuilder, type FrameData } from './scene.ts';
import {
  LOOKING_DOWN,
  LOOKING_UP,
  makeSnapshot,
  NORTH,
  type FixtureBody,
  type FixtureSnapshot,
} from './snapshot-fixture.ts';
import { OWNSHIP_FLAG2 } from '@flying/sim-bridge/layout';
import { buildTerrainMesh } from './terrain-mesh.ts';
import { makeTestPatch } from './terrain-test-patch.ts';

export interface ContractCheck {
  name: string;
  ok: boolean;
  detail: string;
}

export interface ContractOptions {
  /** False for the null renderer: skip the checks that look at pixels. */
  draws: boolean;
  width?: number;
  height?: number;
}

const SKY = CLEAR_COLOUR.map((c) => Math.round(c * 255));

/**
 * Eye position and the six bodies in front of it, all offset by `shift` metres east and north.
 * `eye` and `ownship` replace the default eye position and add ownship fields (g stress, aim).
 */
function world(
  shift: number,
  looking: 'ahead' | 'up' | 'down',
  bodies: boolean,
  options: {
    eye?: [number, number, number];
    ownship?: NonNullable<FixtureSnapshot['ownship']>;
  } = {},
) {
  const eye: [number, number, number] = options.eye ?? [shift, shift - 8000, 1500];
  const list: FixtureBody[] = bodies
    ? [
        { pos: [shift - 600, shift - 4500, 1700] },
        { pos: [shift + 400, shift - 3800, 1450] },
        { pos: [shift + 1400, shift - 3000, 1900] },
        { pos: [shift - 1500, shift - 2500, 1300] },
        { pos: [shift + 100, shift - 5200, 1600] },
        { pos: [shift + 2600, shift - 4200, 1200] },
      ]
    : [];
  const interp = new SnapshotInterpolator();
  interp.push(
    makeSnapshot({
      publishMs: 1000,
      ownship: {
        ...options.ownship,
        pos: eye,
        quat: looking === 'up' ? LOOKING_UP : looking === 'down' ? LOOKING_DOWN : NORTH,
      },
      bodies: list,
    }),
  );
  const state = interp.sample(2000);
  if (!state) throw new Error('no scene state');
  return { state, shift };
}

function distinctColours(rgba: Uint8Array): number {
  const seen = new Set<number>();
  for (let i = 0; i < rgba.length; i += 4) {
    seen.add(((rgba[i] ?? 0) << 16) | ((rgba[i + 1] ?? 0) << 8) | (rgba[i + 2] ?? 0));
  }
  return seen.size;
}

/** How many pixels satisfy `test(r, g, b)`. */
function countWhere(rgba: Uint8Array, test: (r: number, g: number, b: number) => boolean): number {
  let n = 0;
  for (let i = 0; i < rgba.length; i += 4) {
    if (test(rgba[i] ?? 0, rgba[i + 1] ?? 0, rgba[i + 2] ?? 0)) n += 1;
  }
  return n;
}

/** The fraction of pixels that satisfy `test(r, g, b)`. */
function fractionWhere(
  rgba: Uint8Array,
  test: (r: number, g: number, b: number) => boolean,
): number {
  return countWhere(rgba, test) / Math.max(1, rgba.length / 4);
}

/** The mean red, green and blue of a picture. */
function meanChannels(rgba: Uint8Array): [number, number, number] {
  const sum = [0, 0, 0];
  for (let i = 0; i < rgba.length; i += 4) {
    for (let c = 0; c < 3; c++) sum[c] = (sum[c] ?? 0) + (rgba[i + c] ?? 0);
  }
  const n = Math.max(1, rgba.length / 4);
  return [(sum[0] ?? 0) / n, (sum[1] ?? 0) / n, (sum[2] ?? 0) / n];
}

/** Fraction of pixels that differ by more than `tolerance` in any channel. */
function differingFraction(a: Uint8Array, b: Uint8Array, tolerance: number): number {
  if (a.length !== b.length) return 1;
  let differing = 0;
  for (let i = 0; i < a.length; i += 4) {
    for (let c = 0; c < 3; c++) {
      if (Math.abs((a[i + c] ?? 0) - (b[i + c] ?? 0)) > tolerance) {
        differing += 1;
        break;
      }
    }
  }
  return differing / (a.length / 4);
}

export async function runRendererContract(
  make: () => Promise<Renderer>,
  opts: ContractOptions,
): Promise<ContractCheck[]> {
  const width = opts.width ?? 160;
  const height = opts.height ?? 90;
  const checks: ContractCheck[] = [];
  const check = async (
    name: string,
    body: () => Promise<string | undefined> | string | undefined,
  ) => {
    try {
      const problem = await body();
      checks.push({ name, ok: problem === undefined, detail: problem ?? 'ok' });
    } catch (e) {
      checks.push({ name, ok: false, detail: e instanceof Error ? e.message : String(e) });
    }
  };

  const renderer = await make();
  const scene = new SceneBuilder();
  const patchAt = (shift: number) =>
    buildTerrainMesh(makeTestPatch({ centreEast: shift, centreNorth: shift }));
  const frameFor = (
    shift: number,
    looking: 'ahead' | 'up' | 'down',
    bodies: boolean,
    options: Parameters<typeof world>[3] & { gfxStrength?: number; aspect?: number } = {},
  ): FrameData => {
    const mesh = patchAt(shift);
    const { state } = world(shift, looking, bodies, options);
    return scene.build(state, {
      aspect: options.aspect ?? width / height,
      zRange: renderer.zRange,
      timeS: 0,
      patchOriginBf: mesh.originBf,
      spacingM: mesh.spacingM,
      // The airfield is part of the scene: it moves with it.
      airfield: airfieldShifted(shift, shift),
      ...(options.gfxStrength === undefined ? {} : { gfxStrength: options.gfxStrength }),
    });
  };
  const pixelsFor = async (
    shift: number,
    looking: 'ahead' | 'up' | 'down',
    bodies: boolean,
    options: Parameters<typeof frameFor>[3] = {},
  ): Promise<Uint8Array> => {
    renderer.setTerrainPatch(patchAt(shift));
    renderer.render(frameFor(shift, looking, bodies, options));
    return renderer.debugReadPixels();
  };

  try {
    renderer.resize(width, height);
    await check('reads back width x height x 4 bytes', async () => {
      const px = await pixelsFor(0, 'ahead', true);
      return px.length === width * height * 4
        ? undefined
        : `got ${String(px.length)} bytes, expected ${String(width * height * 4)}`;
    });

    await check('resize changes the size of the readback', async () => {
      renderer.resize(width * 2, height);
      const px = await pixelsFor(0, 'ahead', true);
      renderer.resize(width, height);
      return px.length === width * 2 * height * 4
        ? undefined
        : `got ${String(px.length)} bytes after resize`;
    });

    if (opts.draws) {
      await check('looking at the sky shows only the sky colour', async () => {
        const px = await pixelsFor(0, 'up', false);
        const colours = distinctColours(px);
        const first = [px[0], px[1], px[2]];
        const near = first.every((c, k) => Math.abs((c ?? 0) - (SKY[k] ?? 0)) <= 1);
        return colours === 1 && near
          ? undefined
          : `${String(colours)} colours, first pixel ${first.join(',')}, sky ${SKY.join(',')}`;
      });

      await check(
        'looking at the world shows terrain, fog and markers (at least 8 colours)',
        async () => {
          const px = await pixelsFor(0, 'ahead', true);
          const colours = distinctColours(px);
          return colours >= 8 ? undefined : `only ${String(colours)} colours`;
        },
      );

      await check('terrain fills the lower part of the picture and sky the top', async () => {
        const px = await pixelsFor(0, 'ahead', false);
        const at = (x: number, y: number): number[] =>
          [0, 1, 2].map((c) => px[(y * width + x) * 4 + c] ?? 0);
        const top = at(width >> 1, 2);
        const bottom = at(width >> 1, height - 3);
        const topIsSky = top.every((c, k) => Math.abs(c - (SKY[k] ?? 0)) <= 1);
        const bottomIsSky = bottom.every((c, k) => Math.abs(c - (SKY[k] ?? 0)) <= 1);
        return topIsSky && !bottomIsSky
          ? undefined
          : `top ${top.join(',')} bottom ${bottom.join(',')}`;
      });

      await check('markers change the picture', async () => {
        const [a, b] = [await pixelsFor(0, 'ahead', false), await pixelsFor(0, 'ahead', true)];
        const f = differingFraction(a, b, 2);
        return f > 0.0005 ? undefined : `only ${(f * 100).toFixed(3)} % of pixels changed`;
      });

      await check('drawing the same frame twice gives the same picture', async () => {
        const [a, b] = [await pixelsFor(0, 'ahead', true), await pixelsFor(0, 'ahead', true)];
        const f = differingFraction(a, b, 0);
        return f === 0 ? undefined : `${(f * 100).toFixed(3)} % of pixels differ`;
      });

      await check(
        'the same scene moved 400 km draws the same picture (relative-to-eye)',
        async () => {
          const [near, far] = [
            await pixelsFor(0, 'ahead', true),
            await pixelsFor(400_000, 'ahead', true),
          ];
          const f = differingFraction(near, far, 2);
          return f < 0.0005
            ? undefined
            : `${(f * 100).toFixed(3)} % of pixels differ after a 400 km shift`;
        },
      );
    }

    await check('finish resolves, also with nothing drawn yet and after drawing', async () => {
      await renderer.finish();
      renderer.setTerrainPatch(patchAt(0));
      renderer.render(frameFor(0, 'ahead', true));
      await renderer.finish();
      return undefined;
    });

    if (opts.draws) {
      await check('many patch instances draw without breaking the frame', async () => {
        renderer.setTerrainPatch(patchAt(0));
        const frame = frameFor(0, 'ahead', true);
        frame.patchInstances = 64;
        renderer.render(frame);
        await renderer.finish();
        const px = await renderer.debugReadPixels();
        const colours = distinctColours(px);
        return colours >= 8 ? undefined : `only ${String(colours)} colours with 64 instances`;
      });

      await check('a pilot at full g stress sees a dark picture', async () => {
        const px = await pixelsFor(0, 'ahead', true, { ownship: { pos: [0, 0, 0], gStress: 1 } });
        const dark = fractionWhere(px, (r, g, b) => r < 24 && g < 24 && b < 24);
        return dark >= 0.95 ? undefined : `only ${(dark * 100).toFixed(1)} % of pixels are dark`;
      });

      await check(
        'a pilot greying out sees a duller, narrower picture than a rested one',
        async () => {
          const rested = await pixelsFor(0, 'ahead', true);
          const grey = await pixelsFor(0, 'ahead', true, {
            ownship: { pos: [0, 0, 0], gStress: 0.55 },
          });
          const [a, b] = [meanChannels(rested), meanChannels(grey)];
          const darker = b.reduce((x, y) => x + y, 0) < a.reduce((x, y) => x + y, 0) * 0.8;
          const corner = (px: Uint8Array): number => (px[0] ?? 0) + (px[1] ?? 0) + (px[2] ?? 0);
          const edgesClose = corner(grey) < corner(rested) * 0.6;
          return darker && edgesClose
            ? undefined
            : `mean ${a.map(Math.round).join(',')} -> ${b.map(Math.round).join(',')}, corner ${String(corner(rested))} -> ${String(corner(grey))}`;
        },
      );

      await check('a pilot in red-out sees a red picture', async () => {
        const px = await pixelsFor(0, 'ahead', true, { ownship: { pos: [0, 0, 0], redout: 0.6 } });
        const [r, g, b] = meanChannels(px);
        return r > g + 20 && r > b + 20
          ? undefined
          : `mean colour ${[r, g, b].map(Math.round).join(',')} is not red`;
      });

      await check('the effect strength setting switches the effects off', async () => {
        const rested = await pixelsFor(0, 'ahead', true);
        const off = await pixelsFor(0, 'ahead', true, {
          ownship: { pos: [0, 0, 0], gStress: 1, redout: 1 },
          gfxStrength: 0,
        });
        const f = differingFraction(rested, off, 0);
        return f === 0 ? undefined : `${(f * 100).toFixed(3)} % of pixels differ with strength 0`;
      });

      await check('over the runway the picture shows asphalt and paint', async () => {
        // Fifty metres above the middle of the runway, looking straight down, at a size where the
        // paint is several pixels wide.
        const w = 480;
        const h = 270;
        renderer.resize(w, h);
        const mesh = patchAt(0);
        const ground = 450; // the test patch's height at its centre
        renderer.setTerrainPatch(mesh);
        const { state } = world(0, 'down', false, { eye: [0, 0, ground + 50] });
        renderer.render(
          scene.build(state, {
            aspect: w / h,
            zRange: renderer.zRange,
            timeS: 0,
            patchOriginBf: mesh.originBf,
            spacingM: mesh.spacingM,
            // The runway lies 1500 m north of its threshold, so move the threshold 1500 m south of
            // the middle of the patch.
            airfield: airfieldShifted(0, 4500),
          }),
        );
        const px = await renderer.debugReadPixels();
        renderer.resize(width, height);
        const neutral = (r: number, g: number, b: number): boolean =>
          Math.abs(r - g) <= 14 && Math.abs(g - b) <= 14 && Math.abs(r - b) <= 14;
        const asphalt = countWhere(px, (r, g, b) => neutral(r, g, b) && r >= 30 && r <= 110);
        const paint = countWhere(px, (r, g, b) => neutral(r, g, b) && r >= 170);
        return asphalt > 2000 && paint > 40
          ? undefined
          : `${String(asphalt)} asphalt pixels and ${String(paint)} paint pixels`;
      });

      await check('mouse-aim draws the aim and nose cues', async () => {
        const w = 640;
        const h = 360;
        renderer.resize(w, h);
        renderer.setTerrainPatch(patchAt(0));
        const without = await pixelsFor(0, 'ahead', false, { aspect: w / h });
        const withCue = await pixelsFor(0, 'ahead', false, {
          aspect: w / h,
          // The aim ten degrees to the right of the nose and a little up.
          ownship: {
            pos: [0, 0, 0],
            flags2: OWNSHIP_FLAG2.MOUSE_AIM,
            aim: [(10 * Math.PI) / 180, (3 * Math.PI) / 180],
          },
        });
        renderer.resize(width, height);
        const amber = countWhere(withCue, (r, g, b) => r > 235 && g > 160 && g < 210 && b < 60);
        const before = countWhere(without, (r, g, b) => r > 235 && g > 160 && g < 210 && b < 60);
        const white = countWhere(withCue, (r, g, b) => r > 235 && g > 235 && b > 235);
        return amber - before >= 6 && white >= 6
          ? undefined
          : `${String(amber - before)} amber and ${String(white)} white cue pixels`;
      });
    }

    await check('onLost accepts listeners', () => {
      renderer.onLost(() => undefined);
      return undefined;
    });
  } finally {
    renderer.dispose();
  }

  // Lifecycle, on a renderer of its own.
  await check('dispose is idempotent and drawing afterwards does nothing', async () => {
    const r = await make();
    r.resize(width, height);
    r.dispose();
    r.dispose();
    r.render(frameFor(0, 'ahead', true));
    return undefined;
  });

  return checks;
}
