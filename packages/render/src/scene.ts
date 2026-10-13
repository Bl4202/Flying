/**
 * Turns an interpolated world into what the GPU needs for one frame: the uniform block and the
 * marker instance data. Shared by both backends, so the maths is written (and tested) once and the
 * backends only upload and draw.
 *
 * Uniform block, 28 floats = 112 bytes, std140-compatible (it is a mat4 and three vec4s):
 * `view_proj` (16), `sun_dir` (4), `params` = time, post spacing (4), `patch_offset` (4).
 */
import { BF_TO_RENDER, quatMul } from '@flying/sim-bridge/frame-math';
import { SNAPSHOT } from '@flying/sim-bridge/layout';
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

export const FRAME_UNIFORM_FLOATS = 28;
export const FRAME_UNIFORM_BYTES = FRAME_UNIFORM_FLOATS * 4;

export interface FrameData {
  uniform: Float32Array;
  instances: Float32Array;
  instanceCount: number;
}

export class SceneBuilder {
  readonly #frame: FrameData = {
    uniform: new Float32Array(FRAME_UNIFORM_FLOATS),
    instances: new Float32Array(SNAPSHOT.BODY_CAPACITY * MARKER_INSTANCE_FLOATS),
    instanceCount: 0,
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
    },
  ): FrameData {
    const f = this.#frame;
    projection(this.#proj, FOV_Y, opts.aspect, NEAR_M, opts.zRange);
    viewRotation(this.#view, [
      state.observerQuat[0] ?? 1,
      state.observerQuat[1] ?? 0,
      state.observerQuat[2] ?? 0,
      state.observerQuat[3] ?? 0,
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
    u[22] = 0;
    u[23] = 0;
    if (opts.patchOriginBf) {
      relativeToEye(this.#rel, 0, opts.patchOriginBf, 0, state.observerPosBf);
      u[24] = this.#rel[0] ?? 0;
      u[25] = this.#rel[1] ?? 0;
      u[26] = this.#rel[2] ?? 0;
    } else {
      u[24] = u[25] = u[26] = 0;
    }
    u[27] = 0;

    const n = Math.min(state.bodyCount, SNAPSHOT.BODY_CAPACITY);
    for (let i = 0; i < n; i++) {
      const at = i * MARKER_INSTANCE_FLOATS;
      relativeToEye(f.instances, at, state.bodyPosBf, i * 3, state.observerPosBf);
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
    }
    f.instanceCount = n;
    return f;
  }
}
