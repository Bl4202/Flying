/**
 * What every renderer backend must do (`AC-61.5`, `AC-66.2`), as a function that returns its
 * results instead of asserting, so the same checks run under Vitest against the null renderer and
 * in the browser against WebGPU and WebGL2 (the `renderer` end-to-end spec, and `?selftest=renderer`).
 *
 * The drawing checks look at pixels: sky only when looking up; terrain, fog and markers when
 * looking at the world; the same picture twice; and, the one that matters for the exit line, the
 * same picture when the whole scene is moved 400 km, which holds only if positions are made
 * relative to the eye before they are rounded to f32.
 */
import { SnapshotInterpolator } from './interp.ts';
import { CLEAR_COLOUR, type Renderer } from './renderer.ts';
import { SceneBuilder, type FrameData } from './scene.ts';
import { LOOKING_UP, makeSnapshot, NORTH, type FixtureBody } from './snapshot-fixture.ts';
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

/** Eye position and the six bodies in front of it, all offset by `shift` metres east and north. */
function world(shift: number, looking: 'ahead' | 'up', bodies: boolean) {
  const eye: [number, number, number] = [shift, shift - 8000, 1500];
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
      ownship: { pos: eye, quat: looking === 'up' ? LOOKING_UP : NORTH },
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
  const frameFor = (shift: number, looking: 'ahead' | 'up', bodies: boolean): FrameData => {
    const mesh = patchAt(shift);
    const { state } = world(shift, looking, bodies);
    return scene.build(state, {
      aspect: width / height,
      zRange: renderer.zRange,
      timeS: 0,
      patchOriginBf: mesh.originBf,
      spacingM: mesh.spacingM,
    });
  };
  const pixelsFor = async (
    shift: number,
    looking: 'ahead' | 'up',
    bodies: boolean,
  ): Promise<Uint8Array> => {
    renderer.setTerrainPatch(patchAt(shift));
    renderer.render(frameFor(shift, looking, bodies));
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
