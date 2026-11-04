/**
 * The view channel (ADR-41): what the player is doing with the camera and the HUD, from the main
 * thread (where the keyboard, mouse and pads are read) to the Render Worker (where the camera is
 * driven). View actions never enter the input frame: the sim does not know where the pilot looks,
 * and determinism does not depend on it.
 *
 * It is one 64-byte record. With cross-origin isolation it lives in a `SharedArrayBuffer` that the
 * writer updates under a sequence lock (the sequence is odd while a write is in progress) and the
 * reader copies without ever blocking; without isolation the same `ViewState` travels in a message
 * when it changes. Either way the reader gets a `ViewState`.
 *
 * Mouse look travels as running totals, not deltas: the reader takes the difference from what it
 * saw last, so a dropped read loses nothing. The recentre request is a counter for the same reason.
 *
 * | bytes | field |
 * |-------|-------|
 * | 0-3   | sequence (u32; odd while being written) |
 * | 4-7   | flags (u32): 1 zoom, 2 free look, 4 reduced motion, 8 chase camera |
 * | 8     | HUD declutter level, 0 (full) to 2 (minimum) |
 * | 9     | HUD brightness, per cent |
 * | 12-15 | look yaw rate, rad/s (f32, right positive) |
 * | 16-19 | look pitch rate, rad/s (f32, up positive) |
 * | 24-31 | mouse yaw total, rad (f64) |
 * | 32-39 | mouse pitch total, rad (f64) |
 * | 40-43 | recentre count (u32) |
 * | 44-47 | field of view, degrees (f32) |
 * | 48-51 | shake setting, 0 to 1 (f32) |
 * | 52-55 | chase distance, metres (f32) |
 */

export const VIEW = {
  SIZE: 64,
  H_SEQ: 0,
  O_FLAGS: 4,
  O_HUD_DECLUTTER: 8,
  O_HUD_BRIGHTNESS: 9,
  O_LOOK_YAW_RATE: 12,
  O_LOOK_PITCH_RATE: 16,
  O_MOUSE_YAW: 24,
  O_MOUSE_PITCH: 32,
  O_RECENTRE: 40,
  O_FOV_DEG: 44,
  O_SHAKE: 48,
  O_CHASE_DISTANCE: 52,
} as const;

export const VIEW_FLAG = {
  ZOOM: 1,
  FREE_LOOK: 2,
  REDUCED_MOTION: 4,
  CHASE: 8,
} as const;

export type ViewCamera = 'cockpit' | 'chase';

/** Ranges the settings are kept within (GDD 54.11). */
export const FOV_DEG_RANGE = [50, 120] as const;
export const FOV_DEG_DEFAULT = 75;
export const CHASE_DISTANCE_RANGE_M = [40, 150] as const;
export const CHASE_DISTANCE_DEFAULT_M = 45;

export interface ViewState {
  camera: ViewCamera;
  /** Rates the look keys and a hat ask for, radians per second. */
  lookYawRate: number;
  lookPitchRate: number;
  /** Running totals of mouse look, radians. */
  mouseYawTotal: number;
  mousePitchTotal: number;
  /** Counts the recentre requests. */
  recentreCount: number;
  zoom: boolean;
  freeLook: boolean;
  reducedMotion: boolean;
  /** Vertical field of view, degrees. */
  fovDeg: number;
  /** The shake setting, 0 (none) to 1. */
  shake: number;
  chaseDistanceM: number;
  /** 0 full, 1 reduced, 2 minimum. */
  hudDeclutter: number;
  /** 0 to 100 per cent. */
  hudBrightness: number;
}

export function newViewState(): ViewState {
  return {
    camera: 'cockpit',
    lookYawRate: 0,
    lookPitchRate: 0,
    mouseYawTotal: 0,
    mousePitchTotal: 0,
    recentreCount: 0,
    zoom: false,
    freeLook: false,
    reducedMotion: false,
    fovDeg: FOV_DEG_DEFAULT,
    shake: 1,
    chaseDistanceM: CHASE_DISTANCE_DEFAULT_M,
    hudDeclutter: 0,
    hudBrightness: 100,
  };
}

/** Copies `from` into `into` (no allocation). */
export function copyViewState(into: ViewState, from: ViewState): void {
  into.camera = from.camera;
  into.lookYawRate = from.lookYawRate;
  into.lookPitchRate = from.lookPitchRate;
  into.mouseYawTotal = from.mouseYawTotal;
  into.mousePitchTotal = from.mousePitchTotal;
  into.recentreCount = from.recentreCount;
  into.zoom = from.zoom;
  into.freeLook = from.freeLook;
  into.reducedMotion = from.reducedMotion;
  into.fovDeg = from.fovDeg;
  into.shake = from.shake;
  into.chaseDistanceM = from.chaseDistanceM;
  into.hudDeclutter = from.hudDeclutter;
  into.hudBrightness = from.hudBrightness;
}

/** Keeps a received value inside what the channel allows, whatever the sender wrote. */
export function sanitizeViewState(s: ViewState): void {
  const finite = (v: number, fallback: number): number => (Number.isFinite(v) ? v : fallback);
  const clamp = (v: number, lo: number, hi: number, fallback: number): number =>
    Math.min(Math.max(finite(v, fallback), lo), hi);
  s.lookYawRate = clamp(s.lookYawRate, -20, 20, 0);
  s.lookPitchRate = clamp(s.lookPitchRate, -20, 20, 0);
  s.mouseYawTotal = finite(s.mouseYawTotal, 0);
  s.mousePitchTotal = finite(s.mousePitchTotal, 0);
  s.fovDeg = clamp(s.fovDeg, FOV_DEG_RANGE[0], FOV_DEG_RANGE[1], FOV_DEG_DEFAULT);
  s.shake = clamp(s.shake, 0, 1, 1);
  s.chaseDistanceM = clamp(
    s.chaseDistanceM,
    CHASE_DISTANCE_RANGE_M[0],
    CHASE_DISTANCE_RANGE_M[1],
    CHASE_DISTANCE_DEFAULT_M,
  );
  s.hudDeclutter = Math.round(clamp(s.hudDeclutter, 0, 2, 0));
  s.hudBrightness = Math.round(clamp(s.hudBrightness, 0, 100, 100));
}

/** Writes a `ViewState` into a 64-byte buffer under the sequence lock. */
export class ViewWriter {
  readonly #dv: DataView;
  readonly #seq: Int32Array;

  /** @throws RangeError if the buffer is not 64 bytes. */
  constructor(buffer: SharedArrayBuffer | ArrayBuffer) {
    if (buffer.byteLength !== VIEW.SIZE) {
      throw new RangeError(
        `SYS-040: the view channel is ${String(VIEW.SIZE)} bytes, got ${String(buffer.byteLength)}`,
      );
    }
    this.#dv = new DataView(buffer);
    this.#seq = new Int32Array(buffer, 0, 1);
  }

  write(s: ViewState): void {
    const dv = this.#dv;
    // Odd while the record is half-written.
    Atomics.add(this.#seq, 0, 1);
    dv.setUint32(
      VIEW.O_FLAGS,
      (s.zoom ? VIEW_FLAG.ZOOM : 0) |
        (s.freeLook ? VIEW_FLAG.FREE_LOOK : 0) |
        (s.reducedMotion ? VIEW_FLAG.REDUCED_MOTION : 0) |
        (s.camera === 'chase' ? VIEW_FLAG.CHASE : 0),
      true,
    );
    dv.setUint8(VIEW.O_HUD_DECLUTTER, s.hudDeclutter);
    dv.setUint8(VIEW.O_HUD_BRIGHTNESS, s.hudBrightness);
    dv.setFloat32(VIEW.O_LOOK_YAW_RATE, s.lookYawRate, true);
    dv.setFloat32(VIEW.O_LOOK_PITCH_RATE, s.lookPitchRate, true);
    dv.setFloat64(VIEW.O_MOUSE_YAW, s.mouseYawTotal, true);
    dv.setFloat64(VIEW.O_MOUSE_PITCH, s.mousePitchTotal, true);
    dv.setUint32(VIEW.O_RECENTRE, s.recentreCount >>> 0, true);
    dv.setFloat32(VIEW.O_FOV_DEG, s.fovDeg, true);
    dv.setFloat32(VIEW.O_SHAKE, s.shake, true);
    dv.setFloat32(VIEW.O_CHASE_DISTANCE, s.chaseDistanceM, true);
    Atomics.add(this.#seq, 0, 1);
  }
}

/** Reads a `ViewState` from a 64-byte buffer without blocking. */
export class ViewReader {
  readonly #dv: DataView;
  readonly #seq: Int32Array;
  /** Reads that gave up because the writer kept changing the record. */
  contended = 0;

  /** @throws RangeError if the buffer is not 64 bytes. */
  constructor(buffer: SharedArrayBuffer | ArrayBuffer) {
    if (buffer.byteLength !== VIEW.SIZE) {
      throw new RangeError(
        `SYS-040: the view channel is ${String(VIEW.SIZE)} bytes, got ${String(buffer.byteLength)}`,
      );
    }
    this.#dv = new DataView(buffer);
    this.#seq = new Int32Array(buffer, 0, 1);
  }

  /**
   * Copies the record into `out` and returns true, or returns false (leaving `out` as it was) if
   * the writer was mid-update on every try or nothing has been written yet.
   */
  read(out: ViewState): boolean {
    const dv = this.#dv;
    for (let attempt = 0; attempt < 4; attempt++) {
      const before = Atomics.load(this.#seq, 0);
      if (before === 0) return false; // never written
      if ((before & 1) !== 0) continue;
      const flags = dv.getUint32(VIEW.O_FLAGS, true);
      const declutter = dv.getUint8(VIEW.O_HUD_DECLUTTER);
      const brightness = dv.getUint8(VIEW.O_HUD_BRIGHTNESS);
      const yawRate = dv.getFloat32(VIEW.O_LOOK_YAW_RATE, true);
      const pitchRate = dv.getFloat32(VIEW.O_LOOK_PITCH_RATE, true);
      const mouseYaw = dv.getFloat64(VIEW.O_MOUSE_YAW, true);
      const mousePitch = dv.getFloat64(VIEW.O_MOUSE_PITCH, true);
      const recentre = dv.getUint32(VIEW.O_RECENTRE, true);
      const fov = dv.getFloat32(VIEW.O_FOV_DEG, true);
      const shake = dv.getFloat32(VIEW.O_SHAKE, true);
      const chase = dv.getFloat32(VIEW.O_CHASE_DISTANCE, true);
      if (Atomics.load(this.#seq, 0) !== before) continue;
      out.camera = (flags & VIEW_FLAG.CHASE) !== 0 ? 'chase' : 'cockpit';
      out.zoom = (flags & VIEW_FLAG.ZOOM) !== 0;
      out.freeLook = (flags & VIEW_FLAG.FREE_LOOK) !== 0;
      out.reducedMotion = (flags & VIEW_FLAG.REDUCED_MOTION) !== 0;
      out.hudDeclutter = declutter;
      out.hudBrightness = brightness;
      out.lookYawRate = yawRate;
      out.lookPitchRate = pitchRate;
      out.mouseYawTotal = mouseYaw;
      out.mousePitchTotal = mousePitch;
      out.recentreCount = recentre;
      out.fovDeg = fov;
      out.shake = shake;
      out.chaseDistanceM = chase;
      sanitizeViewState(out);
      return true;
    }
    this.contended += 1;
    return false;
  }
}
