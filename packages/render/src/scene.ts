/**
 * Turns an interpolated world into what the GPU needs for one frame: the uniform blocks and the
 * marker instance data. Shared by both backends, so the maths is written (and tested) once and the
 * backends only upload and draw.
 *
 * `Frame` block (ADR-40), 56 floats = 224 bytes, std140-compatible (two mat4s and six vec4s), one
 * per frame:
 *
 * | floats | field       | contents                                                            |
 * |--------|-------------|---------------------------------------------------------------------|
 * | 0-15   | `view_proj` | projection times the rotation-only view                             |
 * | 16-31  | `inv_view`  | the camera axes in render space (right, up, back): view ray to world |
 * | 32-35  | `sun_dir`   | unit vector towards the sun                                         |
 * | 36-39  | `params`    | time, tan(fov / 2), aspect, eye height above MSL                    |
 * | 40-43  | `earth`     | eye height above the globe, globe radius, sin(dip), h * (2 R + h)   |
 * | 44-47  | `eye_up`    | local up at the eye, render axes                                    |
 * | 48-51  | `fx`        | g stress, red-out, effect strength                                  |
 * | 52-55  | `debug`     | classify view, flat-earth fault, eye east and north of the anchor   |
 *
 * `TerrainLevel` block, 12 floats = 48 bytes, one per terrain level: the patch origin minus the
 * eye (render axes) and the level number, the post spacing and the patch span, and the runway
 * (threshold centre in patch-local east and south, length, width; only the finest level has one).
 */
import { BF_TO_RENDER, quatMul, quatRotate } from '@flying/sim-bridge/frame-math';
import { AIRFIELD, OWNSHIP_FLAG2, SNAPSHOT, TERRAIN } from '@flying/sim-bridge/layout';
import { TERRAIN_LEVEL_COUNT } from '@flying/sim-bridge/terrain-levels';
import {
  FOV_Y,
  mul4,
  NEAR_M,
  projection,
  relativeToEye,
  sunDirection,
  viewRotation,
  type ZRange,
} from './camera.ts';
import type { CameraPose } from './camera-rig.ts';
import { MARKER_INSTANCE_FLOATS, MARKER_SIZE_M } from './markers.ts';
import type { SceneState } from './interp.ts';
import { eyeGeometry, globeFloorM, newEyeGeometry } from './sky.ts';

export const FRAME_UNIFORM_FLOATS = 56;
export const FRAME_UNIFORM_BYTES = FRAME_UNIFORM_FLOATS * 4;

/** Where each field of the `Frame` block starts, in floats. */
export const FRAME_AT = {
  VIEW_PROJ: 0,
  INV_VIEW: 16,
  SUN_DIR: 32,
  PARAMS: 36,
  EARTH: 40,
  EYE_UP: 44,
  FX: 48,
  DEBUG: 52,
} as const;

export const LEVEL_UNIFORM_FLOATS = 12;
export const LEVEL_UNIFORM_BYTES = LEVEL_UNIFORM_FLOATS * 4;
/** The post count of a level and so its span in posts' worth of intervals. */
const PATCH_INTERVALS = TERRAIN.POSTS - 1;

/** The default anchor: the point where the sphere touches the BF plane. */
const ORIGIN: readonly [number, number] = [0, 0];

/** What the scene needs of a terrain level to place it. */
export interface LevelSource {
  originBf: ArrayLike<number>;
  spacingM: number;
}

/** Where the terrain levels come from: `TerrainSet` is one. */
export interface LevelSet {
  mesh(level: number): LevelSource | undefined;
}

/** `normal` is the picture; `classify` flat key colours by what drew each pixel (see `classify.ts`). */
export type DebugView = 'normal' | 'classify';

/** A seeded render fault, for the screenshot tests that must fail (ADR-39). Honoured in debug only. */
export type RenderFault = 'flat-earth';

/** Most copies of the terrain patch one frame may draw (the benchmark's heaviest load level). */
export const MAX_PATCH_INSTANCES = 64;

/**
 * A runway the terrain shader paints: where its threshold centre is in BF (east, north), and its
 * size. It lies along north, as the test airfield's does (heading 000).
 */
export interface Airfield {
  thresholdBf: readonly [number, number];
  lengthM: number;
  widthM: number;
}

/** The test airfield (ADR-35): the generated constants of the sim's layout. */
export const TEST_AIRFIELD: Airfield = {
  thresholdBf: [0, -AIRFIELD.THRESHOLD_SOUTH_M],
  lengthM: AIRFIELD.LENGTH_M,
  widthM: AIRFIELD.WIDTH_M,
};

/** The test airfield moved `east` and `north` metres, for tests that move the whole scene. */
export function airfieldShifted(east: number, north: number): Airfield {
  return {
    ...TEST_AIRFIELD,
    thresholdBf: [TEST_AIRFIELD.thresholdBf[0] + east, TEST_AIRFIELD.thresholdBf[1] + north],
  };
}

/** The two cue markers (nose and mouse-aim point) drawn when mouse-aim is in use. */
export const CUE_COUNT = 2;
/** Where the cues are drawn: this far along the nose and aim directions, metres. */
export const CUE_DISTANCE_M = 1500;
const CUE_NOSE_SIZE_M = 45;
const CUE_AIM_SIZE_M = 70;
const CUE_NOSE_TINT = [0.95, 0.95, 0.95] as const;
const CUE_AIM_TINT = [1.0, 0.72, 0.1] as const;

/** Most marker instances a frame can hold: every body, and the cues. */
export const MARKER_INSTANCE_CAPACITY = SNAPSHOT.BODY_CAPACITY + CUE_COUNT;

/** The pilot-effect strength below which, and the stress or red-out below which, nothing is drawn. */
const FX_GREY_FROM = 0.35;
const FX_RED_FROM = 0.45;

export interface FrameData {
  /** The `Frame` block. */
  uniform: Float32Array;
  /** The `TerrainLevel` blocks, one after the other. */
  levelUniform: Float32Array;
  /** Bit `n` is set if terrain level `n` is drawn. */
  levelMask: number;
  /** Draw the sky pass first (off only for the GPU benchmark, which measures the terrain alone). */
  sky: boolean;
  instances: Float32Array;
  instanceCount: number;
  /** Copies of the terrain patch to draw, laid out on a grid; 1 for normal frames. */
  patchInstances: number;
  /** The g and red-out effects have something to draw: the screen pass runs. */
  fx: boolean;
}

export class SceneBuilder {
  readonly #frame: FrameData = {
    uniform: new Float32Array(FRAME_UNIFORM_FLOATS),
    levelUniform: new Float32Array(TERRAIN_LEVEL_COUNT * LEVEL_UNIFORM_FLOATS),
    levelMask: 0,
    sky: true,
    instances: new Float32Array(MARKER_INSTANCE_CAPACITY * MARKER_INSTANCE_FLOATS),
    instanceCount: 0,
    patchInstances: 1,
    fx: false,
  };
  readonly #proj = new Float64Array(16);
  readonly #view = new Float64Array(16);
  readonly #vp = new Float64Array(16);
  readonly #rel = new Float64Array(3);
  readonly #sun = sunDirection();
  readonly #eye = newEyeGeometry();
  /** The eye (BF) and field of view of the frame being built. */
  #eyeBf: ArrayLike<number> = new Float64Array(3);
  #fovY = FOV_Y;

  /**
   * Fills (and returns, for reuse next frame) the frame data for `state`.
   *
   * `terrain` holds the terrain levels (none yet is fine: the sky and globe still draw).
   */
  build(
    state: SceneState,
    opts: {
      aspect: number;
      zRange: ZRange;
      timeS: number;
      terrain: LevelSet;
      /** Copies of the finest terrain level to draw (default 1; the GPU benchmark's load). */
      patchInstances?: number;
      /** The runway to paint (default the test airfield); `null` for none. */
      airfield?: Airfield | null;
      /** How strongly the g and red-out effects show, 0 to 100 per cent (default 100). */
      gfxStrength?: number;
      /** Where the sphere touches the BF plane, east and north (default the origin). */
      anchorBf?: readonly [number, number];
      /** The lowest ground above the sphere, which the globe sits on (default the arena's). */
      globeFloorM?: number;
      debugView?: DebugView;
      fault?: RenderFault | null;
      /** Draw the sky pass (default true). */
      sky?: boolean;
      /** Where the eye is and how it looks; without one, the aircraft's reference point and attitude. */
      camera?: CameraPose;
    },
  ): FrameData {
    const f = this.#frame;
    const cam = opts.camera;
    const q = cam?.quatBf ?? state.ownshipQuat;
    projection(this.#proj, cam?.fovY ?? FOV_Y, opts.aspect, cam?.near ?? NEAR_M, opts.zRange);
    viewRotation(this.#view, [q[0] ?? 1, q[1] ?? 0, q[2] ?? 0, q[3] ?? 0]);
    this.#eyeBf = cam?.eyeBf ?? state.ownshipPosBf;
    this.#fovY = cam?.fovY ?? FOV_Y;
    mul4(this.#vp, this.#proj, this.#view);
    f.sky = opts.sky ?? true;
    f.patchInstances = Math.min(
      MAX_PATCH_INSTANCES,
      Math.max(1, Math.floor(opts.patchInstances ?? 1)),
    );
    this.#frameBlock(f.uniform, state, opts);
    this.#levelBlocks(f, opts);
    f.fx = f.uniform[FRAME_AT.FX + 2] !== 0 && this.#fxActive(state);
    this.#bodies(f, state);
    return f;
  }

  /** Whether the g and red-out effects have anything to draw. */
  #fxActive(state: SceneState): boolean {
    return state.gStress > FX_GREY_FROM || state.redout > FX_RED_FROM;
  }

  /** Fills the `Frame` block. */
  #frameBlock(
    u: Float32Array,
    state: SceneState,
    opts: {
      aspect: number;
      timeS: number;
      gfxStrength?: number;
      anchorBf?: readonly [number, number];
      globeFloorM?: number;
      debugView?: DebugView;
      fault?: RenderFault | null;
    },
  ): void {
    u.set(this.#vp, FRAME_AT.VIEW_PROJ);
    // The camera axes are the rows of the view rotation: its transpose takes a view ray back.
    const v = this.#view;
    for (let col = 0; col < 3; col++) {
      for (let row = 0; row < 3; row++) {
        u[FRAME_AT.INV_VIEW + col * 4 + row] = v[row * 4 + col] ?? 0;
      }
      u[FRAME_AT.INV_VIEW + col * 4 + 3] = 0;
    }
    u[FRAME_AT.INV_VIEW + 12] = 0;
    u[FRAME_AT.INV_VIEW + 13] = 0;
    u[FRAME_AT.INV_VIEW + 14] = 0;
    u[FRAME_AT.INV_VIEW + 15] = 1;
    u[FRAME_AT.SUN_DIR] = this.#sun[0];
    u[FRAME_AT.SUN_DIR + 1] = this.#sun[1];
    u[FRAME_AT.SUN_DIR + 2] = this.#sun[2];
    u[FRAME_AT.SUN_DIR + 3] = 0;

    const flat = opts.fault === 'flat-earth';
    const anchor = opts.anchorBf ?? ORIGIN;
    const eye = eyeGeometry(
      this.#eye,
      this.#eyeBf,
      anchor,
      opts.globeFloorM ?? globeFloorM(state.arena),
      flat,
    );
    u[FRAME_AT.PARAMS] = opts.timeS;
    u[FRAME_AT.PARAMS + 1] = Math.tan(this.#fovY / 2);
    u[FRAME_AT.PARAMS + 2] = opts.aspect;
    u[FRAME_AT.PARAMS + 3] = eye.eyeMslM;
    u[FRAME_AT.EARTH] = eye.heightM;
    u[FRAME_AT.EARTH + 1] = eye.globeRadiusM;
    u[FRAME_AT.EARTH + 2] = eye.sinDip;
    u[FRAME_AT.EARTH + 3] = eye.chordTerm;
    u[FRAME_AT.EYE_UP] = eye.up[0];
    u[FRAME_AT.EYE_UP + 1] = eye.up[1];
    u[FRAME_AT.EYE_UP + 2] = eye.up[2];
    u[FRAME_AT.EYE_UP + 3] = 0;

    // What the pilot's body does to the picture.
    const strength = Math.min(Math.max((opts.gfxStrength ?? 100) / 100, 0), 1);
    u[FRAME_AT.FX] = state.gStress;
    u[FRAME_AT.FX + 1] = state.redout;
    u[FRAME_AT.FX + 2] = strength;
    u[FRAME_AT.FX + 3] = 0;

    u[FRAME_AT.DEBUG] = opts.debugView === 'classify' ? 1 : 0;
    u[FRAME_AT.DEBUG + 1] = flat ? 1 : 0;
    u[FRAME_AT.DEBUG + 2] = (this.#eyeBf[0] ?? 0) - anchor[0];
    u[FRAME_AT.DEBUG + 3] = (this.#eyeBf[1] ?? 0) - anchor[1];
  }

  /** Fills one `TerrainLevel` block per level that has a mesh, and the mask of those. */
  #levelBlocks(f: FrameData, opts: { terrain: LevelSet; airfield?: Airfield | null }): void {
    const airfield = opts.airfield === undefined ? TEST_AIRFIELD : opts.airfield;
    f.levelMask = 0;
    for (let level = 0; level < TERRAIN_LEVEL_COUNT; level++) {
      const at = level * LEVEL_UNIFORM_FLOATS;
      const u = f.levelUniform;
      u.fill(0, at, at + LEVEL_UNIFORM_FLOATS);
      const mesh = opts.terrain.mesh(level);
      if (mesh === undefined) continue;
      f.levelMask |= 1 << level;
      relativeToEye(this.#rel, 0, mesh.originBf, 0, this.#eyeBf);
      u[at] = this.#rel[0] ?? 0;
      u[at + 1] = this.#rel[1] ?? 0;
      u[at + 2] = this.#rel[2] ?? 0;
      u[at + 3] = level;
      u[at + 4] = mesh.spacingM;
      u[at + 5] = mesh.spacingM * PATCH_INTERVALS;
      // The runway, as the patch origin sees it (subtracted in f64, like everything else). Only
      // the finest level paints it.
      if (level === 0 && airfield) {
        u[at + 8] = airfield.thresholdBf[0] - (mesh.originBf[0] ?? 0);
        u[at + 9] = (mesh.originBf[1] ?? 0) - airfield.thresholdBf[1];
        u[at + 10] = airfield.lengthM;
        u[at + 11] = airfield.widthM;
      }
    }
  }

  /** Fills the marker instances: one per body, then the cues. */
  #bodies(f: FrameData, state: SceneState): void {
    const n = Math.min(state.bodyCount, SNAPSHOT.BODY_CAPACITY);
    for (let i = 0; i < n; i++) {
      const at = i * MARKER_INSTANCE_FLOATS;
      relativeToEye(f.instances, at, state.bodyPosBf, i * 3, this.#eyeBf);
      f.instances[at + 3] = MARKER_SIZE_M;
      // Body axes -> render axes is the BF rotation of the body, after the BF -> render change:
      // q = BF_TO_RENDER * bodyQuat, in scalars (this runs every frame and makes no arrays).
      const bw = state.bodyQuat[i * 4] ?? 1;
      const bx = state.bodyQuat[i * 4 + 1] ?? 0;
      const by = state.bodyQuat[i * 4 + 2] ?? 0;
      const bz = state.bodyQuat[i * 4 + 3] ?? 0;
      const s = Math.SQRT1_2;
      // The shader takes (x, y, z, w).
      f.instances[at + 4] = s * (bx - bw);
      f.instances[at + 5] = s * (by + bz);
      f.instances[at + 6] = s * (bz - by);
      f.instances[at + 7] = s * (bw + bx);
      // No tint of its own: the shader colours a body by its index.
      f.instances[at + 8] = f.instances[at + 9] = f.instances[at + 10] = f.instances[at + 11] = 0;
    }
    f.instanceCount = n;
    if ((state.ownshipFlags2 & OWNSHIP_FLAG2.MOUSE_AIM) !== 0) {
      this.#cues(f, state);
    }
  }

  /** Appends the nose and mouse-aim markers, 1.5 km out along their directions. */
  #cues(f: FrameData, state: SceneState): void {
    const q = quatMul(BF_TO_RENDER, [
      state.ownshipQuat[0] ?? 1,
      state.ownshipQuat[1] ?? 0,
      state.ownshipQuat[2] ?? 0,
      state.ownshipQuat[3] ?? 0,
    ]);
    const nose = quatRotate(q, [1, 0, 0]);
    // Azimuth is clockwise from north and elevation is up; render axes are east, up, south.
    const ce = Math.cos(state.aimEl);
    const aim: [number, number, number] = [
      ce * Math.sin(state.aimAz),
      Math.sin(state.aimEl),
      -ce * Math.cos(state.aimAz),
    ];
    this.#cue(f, nose, CUE_NOSE_SIZE_M, CUE_NOSE_TINT);
    this.#cue(f, aim, CUE_AIM_SIZE_M, CUE_AIM_TINT);
  }

  #cue(
    f: FrameData,
    dir: readonly [number, number, number],
    sizeM: number,
    tint: readonly [number, number, number],
  ): void {
    const at = f.instanceCount * MARKER_INSTANCE_FLOATS;
    f.instances[at] = dir[0] * CUE_DISTANCE_M;
    f.instances[at + 1] = dir[1] * CUE_DISTANCE_M;
    f.instances[at + 2] = dir[2] * CUE_DISTANCE_M;
    f.instances[at + 3] = sizeM;
    // The shortest turn from the dart's nose (body x) to the direction: axis = x cross d.
    const w = 1 + dir[0];
    if (w < 1e-6) {
      f.instances[at + 4] = 0;
      f.instances[at + 5] = 1;
      f.instances[at + 6] = 0;
      f.instances[at + 7] = 0;
    } else {
      const x = 0;
      const y = -dir[2];
      const z = dir[1];
      const inv = 1 / Math.hypot(x, y, z, w);
      f.instances[at + 4] = x * inv;
      f.instances[at + 5] = y * inv;
      f.instances[at + 6] = z * inv;
      f.instances[at + 7] = w * inv;
    }
    f.instances[at + 8] = tint[0];
    f.instances[at + 9] = tint[1];
    f.instances[at + 10] = tint[2];
    f.instances[at + 11] = 1;
    f.instanceCount += 1;
  }
}
