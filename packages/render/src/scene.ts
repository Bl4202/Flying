/**
 * Turns an interpolated world into what the GPU needs for one frame: the uniform block and the
 * marker instance data. Shared by both backends, so the maths is written (and tested) once and the
 * backends only upload and draw.
 *
 * Uniform block, 36 floats = 144 bytes, std140-compatible (it is a mat4 and five vec4s):
 * `view_proj` (16), `sun_dir` (4), `params` = time, post spacing, patch span (4), `patch_offset`
 * (4), `runway` = threshold centre in patch-local east and south, length, width (4), and `fx` = g
 * stress, red-out, effect strength (4).
 */
import { BF_TO_RENDER, quatMul, quatRotate } from '@flying/sim-bridge/frame-math';
import { AIRFIELD, OWNSHIP_FLAG2, SNAPSHOT, TERRAIN } from '@flying/sim-bridge/layout';
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
import { MARKER_INSTANCE_FLOATS, MARKER_SIZE_M } from './markers.ts';
import type { SceneState } from './interp.ts';

export const FRAME_UNIFORM_FLOATS = 36;
export const FRAME_UNIFORM_BYTES = FRAME_UNIFORM_FLOATS * 4;

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
  uniform: Float32Array;
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

  /**
   * Fills (and returns, for reuse next frame) the frame data for `state`. `patchOriginBf` is the
   * terrain patch origin, or null if there is no patch yet.
   */
  build(
    state: SceneState,
    opts: {
      aspect: number;
      zRange: ZRange;
      timeS: number;
      patchOriginBf: ArrayLike<number> | null;
      spacingM: number;
      /** Copies of the terrain patch to draw (default 1). */
      patchInstances?: number;
      /** The runway to paint (default the test airfield); `null` for none. */
      airfield?: Airfield | null;
      /** How strongly the g and red-out effects show, 0 to 100 per cent (default 100). */
      gfxStrength?: number;
    },
  ): FrameData {
    const f = this.#frame;
    projection(this.#proj, FOV_Y, opts.aspect, NEAR_M, opts.zRange);
    viewRotation(this.#view, [
      state.ownshipQuat[0] ?? 1,
      state.ownshipQuat[1] ?? 0,
      state.ownshipQuat[2] ?? 0,
      state.ownshipQuat[3] ?? 0,
    ]);
    mul4(this.#vp, this.#proj, this.#view);

    const u = f.uniform;
    u.set(this.#vp, 0);
    u[16] = this.#sun[0];
    u[17] = this.#sun[1];
    u[18] = this.#sun[2];
    u[19] = 0;
    u[20] = opts.timeS;
    u[21] = opts.spacingM;
    u[22] = opts.spacingM * (TERRAIN.POSTS - 1);
    u[23] = 0;
    f.patchInstances = Math.min(
      MAX_PATCH_INSTANCES,
      Math.max(1, Math.floor(opts.patchInstances ?? 1)),
    );
    if (opts.patchOriginBf) {
      relativeToEye(this.#rel, 0, opts.patchOriginBf, 0, state.ownshipPosBf);
      u[24] = this.#rel[0] ?? 0;
      u[25] = this.#rel[1] ?? 0;
      u[26] = this.#rel[2] ?? 0;
    } else {
      u[24] = u[25] = u[26] = 0;
    }
    u[27] = 0;

    // The runway, as the patch origin sees it (subtracted in f64, like everything else).
    const airfield = opts.airfield === undefined ? TEST_AIRFIELD : opts.airfield;
    if (opts.patchOriginBf && airfield) {
      u[28] = airfield.thresholdBf[0] - (opts.patchOriginBf[0] ?? 0);
      u[29] = (opts.patchOriginBf[1] ?? 0) - airfield.thresholdBf[1];
      u[30] = airfield.lengthM;
      u[31] = airfield.widthM;
    } else {
      u[28] = u[29] = u[30] = u[31] = 0;
    }

    // What the pilot's body does to the picture.
    const strength = Math.min(Math.max((opts.gfxStrength ?? 100) / 100, 0), 1);
    u[32] = state.gStress;
    u[33] = state.redout;
    u[34] = strength;
    u[35] = 0;
    f.fx = strength > 0 && (state.gStress > FX_GREY_FROM || state.redout > FX_RED_FROM);

    const n = Math.min(state.bodyCount, SNAPSHOT.BODY_CAPACITY);
    for (let i = 0; i < n; i++) {
      const at = i * MARKER_INSTANCE_FLOATS;
      relativeToEye(f.instances, at, state.bodyPosBf, i * 3, state.ownshipPosBf);
      f.instances[at + 3] = MARKER_SIZE_M;
      // Body axes -> render axes is the BF rotation of the body, after the BF -> render change.
      const q = quatMul(BF_TO_RENDER, [
        state.bodyQuat[i * 4] ?? 1,
        state.bodyQuat[i * 4 + 1] ?? 0,
        state.bodyQuat[i * 4 + 2] ?? 0,
        state.bodyQuat[i * 4 + 3] ?? 0,
      ]);
      // The shader takes (x, y, z, w).
      f.instances[at + 4] = q[1];
      f.instances[at + 5] = q[2];
      f.instances[at + 6] = q[3];
      f.instances[at + 7] = q[0];
      // No tint of its own: the shader colours a body by its index.
      f.instances[at + 8] = f.instances[at + 9] = f.instances[at + 10] = f.instances[at + 11] = 0;
    }
    f.instanceCount = n;
    if ((state.ownshipFlags2 & OWNSHIP_FLAG2.MOUSE_AIM) !== 0) {
      this.#cues(f, state);
    }
    return f;
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
