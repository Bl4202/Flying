/**
 * What every renderer backend must do (`AC-61.5`, `AC-66.2`), as a function that returns its
 * results instead of asserting, so the same checks run under Vitest against the null renderer and
 * in the browser against WebGPU and WebGL2 (the `renderer` end-to-end spec, and `?selftest=renderer`).
 *
 * The drawing checks look at pixels: sky only when looking up (in the classify view, which draws
 * flat key colours); terrain, haze and markers when looking at the world; the same picture twice;
 * the horizon at the dip the height calls for, and not with the flat-earth fault; terrain levels
 * in their own colours; 4x MSAA that changes only edges; and, the one that matters for the exit
 * line, the same picture when the whole scene is moved 400 km (the globe with it), which holds
 * only if positions are made relative to the eye before they are rounded to f32. The pilot's body has its checks too: a
 * stressed pilot sees a dark picture, a pilot in red-out a red one, the effect strength switches
 * them off; and the runway paints its asphalt and its paint, and the mouse-aim cues show.
 */
import { CLASSIFY_KEYS, countKey, isKey } from './classify.ts';
import { SnapshotInterpolator } from './interp.ts';
import { type Renderer } from './renderer.ts';
import {
  airfieldShifted,
  SceneBuilder,
  type DebugView,
  type FrameData,
  type RenderFault,
} from './scene.ts';
import { FOV_Y } from './camera.ts';
import { CameraRig, DEFAULT_COCKPIT, type CameraPose } from './camera-rig.ts';
import { FIXTURE_COCKPIT, FIXTURE_FONT, FIXTURE_HUD, FIXTURE_MODEL } from './model-fixture.ts';
import { Hud } from './hud/hud.ts';
import { bodyToRender } from './camera.ts';
import { newAircraftStatus } from '@flying/sim-bridge';
import { buildCockpitMesh, buildModelMesh } from './model-mesh.ts';
import { ModelRig } from './model-rig.ts';
import { newViewState } from '@flying/sim-bridge/view-channel';
import { horizonDip } from './sky.ts';
import {
  LOOKING_DOWN,
  LOOKING_UP,
  makeSnapshot,
  NORTH,
  quatFromColumns,
  type FixtureBody,
  type FixtureSnapshot,
} from './snapshot-fixture.ts';
import { COMBAT_FLAG, OWNSHIP_FLAG2 } from '@flying/sim-bridge/layout';
import { TerrainSet } from './terrain-levels.ts';
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
  /** One terrain level 0 patch around `(shift, shift)`, the whole scene's offset. */
  const patchAt = (shift: number): TerrainSet => {
    const set = new TerrainSet();
    set.set(0, makeTestPatch({ centreEast: shift, centreNorth: shift }));
    return set;
  };
  const upload = (set: TerrainSet): void => {
    for (const mesh of set.installed()) renderer.setTerrainLevel(mesh.level, mesh);
  };
  type FrameOptions = Parameters<typeof world>[3] & {
    gfxStrength?: number;
    aspect?: number;
    debugView?: DebugView;
    fault?: RenderFault;
    terrain?: TerrainSet;
    camera?: CameraPose;
    model?: ModelRig;
  };
  const frameFor = (
    shift: number,
    looking: 'ahead' | 'up' | 'down',
    bodies: boolean,
    options: FrameOptions = {},
  ): FrameData => {
    const { state } = world(shift, looking, bodies, options);
    return scene.build(state, {
      aspect: options.aspect ?? width / height,
      zRange: renderer.zRange,
      timeS: 0,
      terrain: options.terrain ?? patchAt(shift),
      // The airfield and the globe are part of the scene: they move with it.
      airfield: airfieldShifted(shift, shift),
      anchorBf: [shift, shift],
      ...(options.gfxStrength === undefined ? {} : { gfxStrength: options.gfxStrength }),
      ...(options.debugView === undefined ? {} : { debugView: options.debugView }),
      ...(options.fault === undefined ? {} : { fault: options.fault }),
      ...(options.camera === undefined ? {} : { camera: options.camera }),
      ...(options.model === undefined ? {} : { model: options.model }),
    });
  };
  const pixelsFor = async (
    shift: number,
    looking: 'ahead' | 'up' | 'down',
    bodies: boolean,
    options: FrameOptions = {},
  ): Promise<Uint8Array> => {
    const terrain = options.terrain ?? patchAt(shift);
    upload(terrain);
    renderer.render(frameFor(shift, looking, bodies, { ...options, terrain }));
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
      await check('looking at the sky shows only the sky (classify view)', async () => {
        const px = await pixelsFor(0, 'up', false, { debugView: 'classify' });
        const sky = countKey(px, CLASSIFY_KEYS.sky);
        return sky === width * height
          ? undefined
          : `${String(sky)} of ${String(width * height)} pixels are the sky key`;
      });

      await check('the sky is lighter at the horizon than overhead', async () => {
        const w = 64;
        const h = 64;
        renderer.resize(w, h);
        const up = await pixelsFor(0, 'up', false);
        // Straight ahead from 1.5 km with no terrain: the horizon is a little below the middle.
        const empty = new TerrainSet();
        const ahead = await pixelsFor(0, 'ahead', false, { terrain: empty });
        renderer.resize(width, height);
        const luma = (px: Uint8Array, at: number): number =>
          0.2126 * (px[at] ?? 0) + 0.7152 * (px[at + 1] ?? 0) + 0.0722 * (px[at + 2] ?? 0);
        const overhead = luma(up, (h / 2) * w * 4 + (w / 2) * 4);
        // The row just above the horizon (the dip at 1.5 km is 1.2 degrees, 1.7 rows at 64 rows).
        const nearHorizon = luma(ahead, (h / 2 - 3) * w * 4 + (w / 2) * 4);
        return nearHorizon > overhead + 10
          ? undefined
          : `luma ${nearHorizon.toFixed(0)} near the horizon, ${overhead.toFixed(0)} overhead`;
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
        const px = await pixelsFor(0, 'ahead', false, { debugView: 'classify' });
        const at = (x: number, y: number): number => (y * width + x) * 4;
        const topIsSky = isKey(px, at(width >> 1, 2), CLASSIFY_KEYS.sky);
        const bottomIsTerrain = isKey(px, at(width >> 1, height - 3), CLASSIFY_KEYS.terrain[0]);
        return topIsSky && bottomIsTerrain
          ? undefined
          : `top ${[0, 1, 2].map((c) => String(px[at(width >> 1, 2) + c])).join(',')} bottom ${[0, 1, 2].map((c) => String(px[at(width >> 1, height - 3) + c])).join(',')}`;
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
      upload(patchAt(0));
      renderer.render(frameFor(0, 'ahead', true));
      await renderer.finish();
      return undefined;
    });

    if (opts.draws) {
      /** The first row, from the top, of the centre column that is the key `key`; -1 if none. */
      const firstRowOf = (px: Uint8Array, w: number, h: number, key: readonly number[]): number => {
        for (let y = 0; y < h; y++) {
          if (isKey(px, (y * w + (w >> 1)) * 4, key as readonly [number, number, number])) return y;
        }
        return -1;
      };
      /** Where the horizon should be: `dipRad` below the middle of a picture `h` rows tall. */
      const horizonRow = (h: number, dipRad: number): number =>
        ((1 + Math.tan(dipRad) / Math.tan(FOV_Y / 2)) / 2) * h;

      await check(
        'with no terrain the globe starts at the horizon dip (10 km: 3.2 degrees)',
        async () => {
          const [w, h] = [160, 180];
          renderer.resize(w, h);
          const px = await pixelsFor(0, 'ahead', false, {
            terrain: new TerrainSet(),
            eye: [0, 0, 10_000],
            debugView: 'classify',
            aspect: w / h,
          });
          renderer.resize(width, height);
          const row = firstRowOf(px, w, h, CLASSIFY_KEYS.globe);
          const want = horizonRow(h, horizonDip(10_000));
          // 3.2 degrees is 8 rows at this size, and a flat world would put it at row 90.
          return Math.abs(row - want) <= 1.5
            ? undefined
            : `the globe starts at row ${String(row)}, expected ${want.toFixed(1)}`;
        },
      );

      await check('the flat-earth fault puts the horizon on the eye line', async () => {
        const [w, h] = [160, 180];
        renderer.resize(w, h);
        const px = await pixelsFor(0, 'ahead', false, {
          terrain: new TerrainSet(),
          eye: [0, 0, 10_000],
          debugView: 'classify',
          fault: 'flat-earth',
          aspect: w / h,
        });
        renderer.resize(width, height);
        const row = firstRowOf(px, w, h, CLASSIFY_KEYS.globe);
        return Math.abs(row - h / 2) <= 1.5
          ? undefined
          : `the flat world starts at row ${String(row)}, expected ${String(h / 2)}`;
      });

      await check('terrain levels draw in their own colours', async () => {
        const set = new TerrainSet();
        set.set(1, makeTestPatch({ spacingM: 1000 }));
        set.set(0, makeTestPatch({ spacingM: 250 }));
        const px = await pixelsFor(0, 'ahead', false, {
          terrain: set,
          eye: [0, -8000, 1500],
          debugView: 'classify',
        });
        const [l0, l1] = [
          countKey(px, CLASSIFY_KEYS.terrain[0]),
          countKey(px, CLASSIFY_KEYS.terrain[1]),
        ];
        return l0 > 1000 && l1 > 100
          ? undefined
          : `${String(l0)} level 0 pixels and ${String(l1)} level 1 pixels`;
      });

      await check(
        '4x MSAA changes only edges, and the count asked for is the count in use',
        async () => {
          const terrain = patchAt(0);
          const before = await pixelsFor(0, 'ahead', false, { terrain, debugView: 'classify' });
          const got = renderer.setSampleCount(4);
          if (renderer.sampleCount !== got)
            return `sampleCount ${String(renderer.sampleCount)} after asking for 4 got ${String(got)}`;
          const after = await pixelsFor(0, 'ahead', false, { terrain, debugView: 'classify' });
          renderer.setSampleCount(1);
          if (got === 1) return undefined; // this backend has no MSAA
          const f = differingFraction(before, after, 8);
          return f < 0.1 ? undefined : `${(f * 100).toFixed(1)} % of pixels differ with 4x MSAA`;
        },
      );

      /** A snapshot of the aircraft with its gear at `gearPos`, as the scene sees it. */
      const aircraftWorld = (gearPos: number, elevatorRad = 0, abLevel = 0) => {
        const interp = new SnapshotInterpolator();
        interp.push(
          makeSnapshot({
            publishMs: 1000,
            ownship: { pos: [0, -6000, 1500], quat: NORTH, gearPos, elevatorRad, abLevel },
          }),
        );
        const state = interp.sample(2000);
        if (!state) throw new Error('no scene state');
        return state;
      };

      const drawAircraft = async (
        camera: 'cockpit' | 'chase',
        extra: {
          gearPos?: number;
          elevatorRad?: number;
          abLevel?: number;
          debugView?: DebugView;
        } = {},
      ): Promise<Uint8Array> => {
        const [w, h] = [480, 270];
        renderer.resize(w, h);
        renderer.setMesh('exterior', buildModelMesh(FIXTURE_MODEL));
        renderer.setMesh('cockpit', buildCockpitMesh(FIXTURE_COCKPIT));
        const state = aircraftWorld(extra.gearPos ?? 1, extra.elevatorRad ?? 0, extra.abLevel ?? 0);
        const rig = new CameraRig(
          { eyeBodyM: FIXTURE_COCKPIT.eyePointM, headLimits: DEFAULT_COCKPIT.headLimits },
          { heightAt: () => Number.NaN },
        );
        const view = { ...newViewState(), camera };
        for (let i = 0; i < 400; i++) rig.step(state, view, 1 / 60);
        const terrain = patchAt(0);
        upload(terrain);
        renderer.render(
          scene.build(state, {
            aspect: w / h,
            zRange: renderer.zRange,
            timeS: 0,
            terrain,
            camera: rig.pose,
            model: new ModelRig(FIXTURE_MODEL),
            ...(extra.debugView === undefined ? {} : { debugView: extra.debugView }),
          }),
        );
        const px = await renderer.debugReadPixels();
        renderer.setMesh('exterior', undefined);
        renderer.setMesh('cockpit', undefined);
        renderer.resize(width, height);
        return px;
      };

      await check('the chase view draws the aircraft, canopy and all', async () => {
        const px = await drawAircraft('chase', { debugView: 'classify' });
        // From 45 m behind and 9 m above, the aircraft is a few dozen pixels across at this size.
        const model = countKey(px, CLASSIFY_KEYS.model);
        return model > 80 ? undefined : `${String(model)} model pixels`;
      });

      await check('the bandit draws its model where it is (classify view)', async () => {
        const [w, h] = [480, 270];
        renderer.resize(w, h);
        renderer.setMesh('exterior', buildModelMesh(FIXTURE_MODEL));
        // 50 m ahead and 10 m to the right, flying east: broadside on, a few hundred pixels. Off
        // the axis, so a bandit drawn in the wrong place (or the ownship's place) is caught.
        const [ahead, right] = [50, 10];
        const east = quatFromColumns([1, 0, 0], [0, -1, 0], [0, 0, -1]);
        const interp = new SnapshotInterpolator();
        interp.push(
          makeSnapshot({
            publishMs: 1000,
            ownship: { pos: [0, -6000, 1500], quat: NORTH },
            combat: {
              flags: COMBAT_FLAG.ACTIVE,
              bandit: { pos: [right, -6000 + ahead, 1500], quat: east },
            },
          }),
        );
        const state = interp.sample(2000);
        if (!state) return 'no scene state';
        const terrain = patchAt(0);
        upload(terrain);
        renderer.render(
          scene.build(state, {
            aspect: w / h,
            zRange: renderer.zRange,
            timeS: 0,
            terrain,
            debugView: 'classify',
            banditModel: new ModelRig(FIXTURE_MODEL),
          }),
        );
        const px = await renderer.debugReadPixels();
        renderer.setMesh('exterior', undefined);
        renderer.resize(width, height);
        let n = 0;
        let sx = 0;
        let sy = 0;
        for (let i = 0; i < w * h; i++) {
          if (!isKey(px, i * 4, CLASSIFY_KEYS.model)) continue;
          n += 1;
          sx += i % w;
          sy += Math.floor(i / w);
        }
        // About 12 m by 2 m at 50 m: some 60 by 10 pixels, and its wing seen edge on.
        if (n < 200) return `${String(n)} model pixels`;
        const [cx, cy] = [sx / n, sy / n];
        const wantX = (1 + right / ahead / (Math.tan(FOV_Y / 2) * (w / h))) * (w / 2);
        return Math.abs(cx - wantX) < w * 0.03 && Math.abs(cy - h / 2) < h * 0.05
          ? undefined
          : `the model's pixels are centred at (${cx.toFixed(0)}, ${cy.toFixed(0)}), not ` +
              `(${wantX.toFixed(0)}, ${String(h / 2)})`;
      });

      await check(
        'the cockpit view draws the cockpit and the airframe ahead of the eye',
        async () => {
          const px = await drawAircraft('cockpit', { debugView: 'classify' });
          const [cockpit, model] = [
            countKey(px, CLASSIFY_KEYS.cockpit),
            countKey(px, CLASSIFY_KEYS.model),
          ];
          return cockpit > 100 && model > 100
            ? undefined
            : `${String(cockpit)} cockpit pixels and ${String(model)} airframe pixels`;
        },
      );

      await check(
        'the parts move with the sim: elevator and afterburner change the chase picture',
        async () => {
          // From behind and above, the tail surfaces and the glowing nozzle are what can be seen.
          const [rest, moved] = [
            await drawAircraft('chase'),
            await drawAircraft('chase', { elevatorRad: (25 * Math.PI) / 180, abLevel: 1 }),
          ];
          const f = differingFraction(rest, moved, 6);
          return f > 0.0001 ? undefined : `only ${(f * 100).toFixed(4)} % of pixels changed`;
        },
      );

      await check(
        'the aircraft is lit, not flat: the normal view differs from the classify view',
        async () => {
          const [lit, flat] = [
            await drawAircraft('chase'),
            await drawAircraft('chase', { debugView: 'classify' }),
          ];
          const f = differingFraction(lit, flat, 6);
          return f > 0.02 ? undefined : `only ${(f * 100).toFixed(2)} % of pixels differ`;
        },
      );

      /** A cockpit or chase picture with the HUD, with the eye moved `eyeShift` metres in body y (right). */
      const drawHud = async (
        camera: 'cockpit' | 'chase',
        extra: { eyeShift?: number; w?: number; h?: number } = {},
      ): Promise<Uint8Array> => {
        const [w, h] = [extra.w ?? 480, extra.h ?? 270];
        renderer.resize(w, h);
        const state = aircraftWorld(1);
        const rig = new CameraRig(
          { eyeBodyM: FIXTURE_COCKPIT.eyePointM, headLimits: DEFAULT_COCKPIT.headLimits },
          { heightAt: () => Number.NaN },
        );
        const view = { ...newViewState(), camera };
        for (let i = 0; i < 400; i++) rig.step(state, view, 1 / 60);
        const terrain = patchAt(0);
        upload(terrain);
        // Heading north, so body right is east: move the eye east by `eyeShift`.
        const pose = { ...rig.pose, eyeBf: Float64Array.from(rig.pose.eyeBf) };
        pose.eyeBf[0] = (pose.eyeBf[0] ?? 0) + (extra.eyeShift ?? 0);
        const frame = scene.build(state, {
          aspect: w / h,
          zRange: renderer.zRange,
          timeS: 0,
          terrain,
          camera: pose,
          model: new ModelRig(FIXTURE_MODEL),
          debugView: 'classify',
        });
        const hud = new Hud(FIXTURE_HUD, FIXTURE_COCKPIT, FIXTURE_FONT);
        const status = newAircraftStatus();
        status.assist = 2;
        status.cas = 130;
        status.altitude = 1500;
        status.agl = 1000;
        const rotation = new Float32Array(16);
        bodyToRender(rotation, state.ownshipQuat);
        hud.fillFrame(
          frame,
          {
            posBf: state.ownshipPosBf,
            quatBf: state.ownshipQuat,
            velBf: [0, 130, 0],
            status,
            warnings: 0,
            timeS: 0,
          },
          rotation,
          {
            declutter: 0,
            brightness: 1,
            palette: 'green',
            overlay: camera === 'chase',
            overlayScale: 1,
            widthPx: w,
            heightPx: h,
            fovY: pose.fovY,
            eyeBf: pose.eyeBf,
          },
        );
        renderer.setMesh('exterior', buildModelMesh(FIXTURE_MODEL));
        renderer.setMesh('cockpit', buildCockpitMesh(FIXTURE_COCKPIT));
        renderer.render(frame);
        const px = await renderer.debugReadPixels();
        renderer.setMesh('exterior', undefined);
        renderer.setMesh('cockpit', undefined);
        renderer.resize(width, height);
        return px;
      };

      await check(
        'the HUD draws in the cockpit view, inside the combiner and nowhere else',
        async () => {
          const [w, h] = [640, 360];
          const px = await drawHud('cockpit', { w, h });
          let n = 0;
          let outside = 0;
          // The combiner: 28 by 20 degrees from the eye, in a picture 75 degrees tall.
          const tan = Math.tan(FOV_Y / 2);
          const halfX = ((Math.tan((14 * Math.PI) / 180) / (tan * (w / h))) * w) / 2 + 3;
          const halfY = ((Math.tan((10 * Math.PI) / 180) / tan) * h) / 2 + 3;
          for (let y = 0; y < h; y++) {
            for (let x = 0; x < w; x++) {
              if (!isKey(px, (y * w + x) * 4, CLASSIFY_KEYS.hud)) continue;
              n += 1;
              if (Math.abs(x - w / 2) > halfX || Math.abs(y - h / 2) > halfY) outside += 1;
            }
          }
          return n > 150 && outside === 0
            ? undefined
            : `${String(n)} HUD pixels, ${String(outside)} outside the combiner`;
        },
      );

      await check(
        'the HUD is collimated: moving the eye inside the eye box moves no symbol',
        async () => {
          const [w, h] = [640, 360];
          const [centre, shifted] = [
            await drawHud('cockpit', { w, h }),
            await drawHud('cockpit', { w, h, eyeShift: 0.03 }),
          ];
          // The middle of the combiner (8 by 6 degrees either way), away from its edge, which does
          // move with the eye: what is drawn there must not.
          const tan = Math.tan(FOV_Y / 2);
          const halfX = ((Math.tan((8 * Math.PI) / 180) / (tan * (w / h))) * w) / 2;
          const halfY = ((Math.tan((6 * Math.PI) / 180) / tan) * h) / 2;
          let differing = 0;
          let total = 0;
          for (let y = Math.floor(h / 2 - halfY); y < h / 2 + halfY; y++) {
            for (let x = Math.floor(w / 2 - halfX); x < w / 2 + halfX; x++) {
              const at = (y * w + x) * 4;
              const a = isKey(centre, at, CLASSIFY_KEYS.hud);
              const b = isKey(shifted, at, CLASSIFY_KEYS.hud);
              if (a || b) total += 1;
              if (a !== b) differing += 1;
            }
          }
          return total > 40 && differing <= total * 0.02
            ? undefined
            : `${String(differing)} of ${String(total)} HUD pixels moved with a 30 mm eye shift`;
        },
      );

      await check('the HUD is an overlay in the chase view', async () => {
        const px = await drawHud('chase');
        const n = countKey(px, CLASSIFY_KEYS.hud);
        return n > 150 ? undefined : `${String(n)} HUD pixels`;
      });

      await check('many patch instances draw without breaking the frame', async () => {
        upload(patchAt(0));
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
        const terrain = patchAt(0);
        const ground = 450; // the test patch's height at its centre
        upload(terrain);
        const { state } = world(0, 'down', false, { eye: [0, 0, ground + 50] });
        renderer.render(
          scene.build(state, {
            aspect: w / h,
            zRange: renderer.zRange,
            timeS: 0,
            terrain,
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
