/**
 * The camera rig (ADR-41, GDD 54.11): where the eye is and which way it looks, for the cockpit and
 * chase cameras. It lives on the render side and never touches the sim: the player's view is not
 * part of the simulation and cannot change a hash.
 *
 * **Cockpit.** The eye is the cockpit's design eye (BODY FRD from the reference point), moved a
 * couple of centimetres by the load the pilot feels (at most 2 cm, none with reduced motion). The
 * head turns within limits (data) under look keys, a hat or the mouse, through a critically damped
 * spring whose step response reaches 63 per cent in 60 ms. Zoom narrows the field of view to 30
 * degrees; the field of view setting runs 50 to 120 (default 75). Near plane 0.3 m.
 *
 * **Chase.** The camera sits behind the aircraft along its velocity direction (the nose below
 * 20 m/s, where the velocity means nothing), 45 m back by default (40 to 150) and 20 per cent of
 * that above, following with a first-order lag of 0.8 s, looking at the aircraft with the horizon
 * level. The field of view widens by 8 degrees towards the aircraft's top speed. Free look orbits
 * it. Near plane 0.5 m (the cockpit's 0.3 m would not do 5 m from a runway; clarification of
 * GDD 44.8's 5 m).
 *
 * **Both** keep the eye at least 1.5 m above the terrain, and the chase camera's arm (the segment
 * from the aircraft to the camera) is marched against the terrain and pulled in rather than let
 * the ground cut through it (AC-54.6). Shake from buffet, ground roll and the afterburner is
 * scaled by the player's setting and held to 0.5 degrees with reduced motion (AC-57.3).
 *
 * The step makes no objects: all state is in typed arrays, so a frame costs no garbage.
 */
import type { ViewState } from '@flying/sim-bridge/view-channel';
import type { CockpitSpec } from './present.ts';
import { EARTH_RADIUS_M } from './sky.ts';

export type CameraMode = 'cockpit' | 'chase';

/** Near planes, metres: inside the cockpit and outside it. */
export const NEAR_COCKPIT_M = 0.3;
export const NEAR_EXTERNAL_M = 0.5;

// DATA: GDD 54.11
/** The camera is never lower than this above the terrain, metres. */
export const MIN_CLEARANCE_M = 1.5;
/** The head spring reaches 63 per cent of a step in this long, seconds. */
export const HEAD_T63_S = 0.06;
/** Zoom (held) narrows the vertical field of view to this, degrees. */
export const ZOOM_FOV_DEG = 30;
/** Chase: height above the aircraft as a fraction of the distance (a design choice). */
export const CHASE_HEIGHT_FRACTION = 0.2;
/** Chase: time constant of the follow lag, seconds (the range is 0.6 to 1.2). */
export const CHASE_LAG_S = 0.8;
/** Chase: widening of the field of view at top speed, degrees. */
export const CHASE_FOV_BOOST_DEG = 8;
/** Chase: below this speed the camera follows the nose, not the velocity, m/s. */
export const CHASE_NOSE_BELOW_MPS = 20;
/** Chase: the speed at which the field of view has widened fully, m/s (the Kestrel's top speed). */
export const CHASE_VMAX_MPS = 340; // DATA: the Kestrel's Vmax, about M 1.0 at sea level
/** The load the pilot feels moves the eye by this much per g, and by at most the cap, metres. */
export const G_OFFSET_PER_G_M = 0.004;
export const G_OFFSET_MAX_M = 0.02;
/** Shake: its largest amplitude at a full setting, and with reduced motion, radians. */
const DEG = Math.PI / 180;
export const SHAKE_MAX_RAD = 1.5 * DEG;
export const SHAKE_REDUCED_MAX_RAD = 0.5 * DEG;

/** The critically damped spring rate that reaches 63.2 per cent of a step in `t63` seconds. */
const SPRING_U63 = 2.146193220620582; // solves (1 + u) exp(-u) = exp(-1)
function springRate(t63: number): number {
  return SPRING_U63 / t63;
}
const HEAD_OMEGA = springRate(HEAD_T63_S);
const FOV_OMEGA = springRate(0.08);

/** The arm is marched at this spacing, metres, so no terrain feature it could clip is skipped. */
const ARM_STEP_M = 4;
/** The arm stays this far above the terrain, metres. */
const ARM_CLEARANCE_M = 2;
/** The camera is not pulled closer to the aircraft than this, metres. */
const ARM_MIN_M = 6;
/** A jump larger than this, metres, is a teleport (a respawn): the chase camera snaps. */
const TELEPORT_M = 2000;
/** Free-look orbit pitch limit, radians. */
const ORBIT_PITCH_MAX = 80 * DEG;

/** What the rig needs to know of the cockpit. */
export interface CockpitView {
  /** The design eye, BODY FRD from the reference point, metres. */
  eyeBodyM: readonly [number, number, number];
  /** Head limits, degrees: yaw both ways, pitch up and pitch down. */
  headLimits: { yawDeg: number; pitchUpDeg: number; pitchDownDeg: number };
}

/** The rig's view of a cockpit layout from the presentation bundle. */
export function cockpitView(spec: CockpitSpec): CockpitView {
  return {
    eyeBodyM: spec.eyePointM,
    headLimits: {
      yawDeg: spec.headLimitsDeg.yaw,
      pitchUpDeg: spec.headLimitsDeg.pitchUp,
      pitchDownDeg: spec.headLimitsDeg.pitchDown,
    },
  };
}

/** Until a cockpit is known: the eye at the reference point and the head limits of GDD 54.11. */
export const DEFAULT_COCKPIT: CockpitView = {
  eyeBodyM: [0, 0, 0],
  headLimits: { yawDeg: 135, pitchUpDeg: 60, pitchDownDeg: 60 },
};

/** The ground the camera must stay above. */
export interface RigTerrain {
  /** The terrain surface's BF height (up) at BF `(east, north)`, metres; NaN where unknown. */
  heightAt(eastM: number, northM: number): number;
}

/** What the rig takes of the world each frame: the aircraft. */
export interface RigState {
  /** The aircraft's reference point, BF metres (f64). */
  ownshipPosBf: ArrayLike<number>;
  /** Body-to-BF attitude, `[w, x, y, z]`. */
  ownshipQuat: ArrayLike<number>;
  /** Velocity in BF, m/s. */
  ownshipVelBf: ArrayLike<number>;
  ownshipSpeed: number;
  /** The load the pilot feels, g. */
  gLagged: number;
  /** Angle of attack and the angle where the warning starts, radians. */
  alpha: number;
  alphaWarn: number;
  groundSpeed: number;
  abLevel: number;
  /** Weight is on the wheels. */
  onGround: boolean;
}

/** Where the eye is and how it looks: what the scene is drawn from. */
export interface CameraPose {
  mode: CameraMode;
  /** The eye, BF metres (f64). */
  eyeBf: Float64Array;
  /** The camera's attitude, `[w, x, y, z]`: its x is forward, y right, z down, as for a body. */
  quatBf: Float64Array;
  /** Vertical field of view, radians. */
  fovY: number;
  near: number;
}

export function newCameraPose(): CameraPose {
  return {
    mode: 'cockpit',
    eyeBf: new Float64Array(3),
    quatBf: new Float64Array([1, 0, 0, 0]),
    fovY: 75 * DEG,
    near: NEAR_COCKPIT_M,
  };
}

/** How hard the shake sources are working, 0 to 1 (before the player's setting scales it). */
export function shakeDrive(
  alpha: number,
  alphaWarn: number,
  groundSpeed: number,
  abLevel: number,
  onGround: boolean,
): number {
  // Buffet starts 3 degrees below the warning angle and is full at the warning angle.
  const margin = 3 * DEG;
  const buffet = Math.min(Math.max((alpha - (alphaWarn - margin)) / margin, 0), 1);
  // Ground roll: a rumble that grows with speed on the wheels, full at 60 m/s.
  const ground = onGround ? Math.min(Math.max(groundSpeed / 60, 0), 1) * 0.4 : 0;
  const burner = Math.min(Math.max(abLevel, 0), 1) * 0.3;
  return Math.min(buffet + ground + burner, 1);
}

// Registers: the rig's state, in one typed array so no store allocates. A spring occupies four:
// position, rate, target and its rate constant; the step takes only the index of the first, so no
// number crosses a call.
const R_YAW = 0;
const R_PITCH = 4;
const R_FOV = 8;
const S_X = 0;
const S_V = 1;
const S_T = 2;
const S_OMEGA = 3;
const R_MOUSE_YAW = 12;
const R_MOUSE_PITCH = 13;
const R_RECENTRE = 14;
const R_CAM_X = 15;
const R_CAM_Y = 16;
const R_CAM_Z = 17;
const R_ORBIT_YAW = 18;
const R_ORBIT_PITCH = 19;
const R_TIME = 20;
const R_FLAGS = 21; // bit 0: seen the view channel; bit 1: chase camera placed
const R_DT = 22;
const REGISTERS = 23;

/** Exact step of the critically damped spring at `at` over `r[R_DT]` seconds. */
function springStep(r: Float64Array, at: number): void {
  const x = r[at + S_X] ?? 0;
  const v = r[at + S_V] ?? 0;
  const omega = r[at + S_OMEGA] ?? 1;
  const dt = r[R_DT] ?? 0;
  const e = x - (r[at + S_T] ?? 0);
  const decay = Math.exp(-omega * dt);
  const j = v + omega * e;
  r[at + S_X] = (r[at + S_T] ?? 0) + (e + j * dt) * decay;
  r[at + S_V] = (v - j * omega * dt) * decay;
}

export class CameraRig {
  readonly pose: CameraPose = newCameraPose();
  #cockpit: CockpitView;
  readonly #terrain: RigTerrain;
  readonly #r = new Float64Array(REGISTERS);
  readonly #head = new Float64Array(4); // scratch quaternion
  readonly #args = new Float64Array(8); // scratch arguments for the helpers

  constructor(cockpit: CockpitView, terrain: RigTerrain) {
    this.#cockpit = cockpit;
    this.#terrain = terrain;
    this.reset();
  }

  /** The cockpit the rig looks out of. */
  get cockpit(): CockpitView {
    return this.#cockpit;
  }

  /** Looks out of another cockpit (another aircraft); the head goes back to the centre. */
  setCockpit(cockpit: CockpitView): void {
    this.#cockpit = cockpit;
    const r = this.#r;
    for (const at of [R_YAW, R_PITCH]) {
      r[at + S_X] = r[at + S_V] = r[at + S_T] = 0;
    }
  }

  /** Forgets the head angles, the chase camera's place and the view channel's totals. */
  reset(): void {
    const r = this.#r;
    r.fill(0);
    r[R_YAW + S_OMEGA] = HEAD_OMEGA;
    r[R_PITCH + S_OMEGA] = HEAD_OMEGA;
    r[R_FOV + S_X] = 75 * DEG;
    r[R_FOV + S_T] = 75 * DEG;
    r[R_FOV + S_OMEGA] = FOV_OMEGA;
  }

  /** The head's current yaw and pitch, radians (for tests and the HUD's eye position). */
  get head(): { yaw: number; pitch: number } {
    return { yaw: this.#r[R_YAW + S_X] ?? 0, pitch: this.#r[R_PITCH + S_X] ?? 0 };
  }

  /** The head's targets, radians: where the spring is heading. */
  get headTarget(): { yaw: number; pitch: number } {
    return { yaw: this.#r[R_YAW + S_T] ?? 0, pitch: this.#r[R_PITCH + S_T] ?? 0 };
  }

  /**
   * Moves the rig by `dtS` seconds and returns the pose (the same object every call).
   *
   * @param state the aircraft
   * @param view what the player is doing with the view
   */
  step(state: RigState, view: ViewState, dtS: number): CameraPose {
    const r = this.#r;
    const dt = Math.min(Math.max(Number.isFinite(dtS) ? dtS : 0, 0), 0.25);
    r[R_DT] = dt;
    r[R_TIME] = (r[R_TIME] ?? 0) + dt;
    this.#look(view, dt);
    const chase = view.camera === 'chase';
    this.pose.mode = view.camera;
    if (chase) {
      this.#placeChase(state, view, dt);
    } else {
      this.#placeCockpit(state, view);
      r[R_FLAGS] = (r[R_FLAGS] ?? 0) & ~2; // the chase camera is placed afresh next time
    }
    this.#fov(state, view, chase);
    this.pose.near = chase ? NEAR_EXTERNAL_M : NEAR_COCKPIT_M;
    return this.pose;
  }

  /** Reads the player's look input into the targets, and moves the head or orbit towards them. */
  #look(view: ViewState, dt: number): void {
    const r = this.#r;
    const first = ((r[R_FLAGS] ?? 0) & 1) === 0;
    if (first) {
      // Take the channel's totals as the starting point: mouse look before now is history.
      r[R_MOUSE_YAW] = view.mouseYawTotal;
      r[R_MOUSE_PITCH] = view.mousePitchTotal;
      r[R_RECENTRE] = view.recentreCount;
      r[R_FLAGS] = (r[R_FLAGS] ?? 0) | 1;
    }
    const mouseYaw = view.mouseYawTotal - (r[R_MOUSE_YAW] ?? 0);
    const mousePitch = view.mousePitchTotal - (r[R_MOUSE_PITCH] ?? 0);
    r[R_MOUSE_YAW] = view.mouseYawTotal;
    r[R_MOUSE_PITCH] = view.mousePitchTotal;
    const recentre = view.recentreCount !== (r[R_RECENTRE] ?? 0);
    r[R_RECENTRE] = view.recentreCount;

    const yawDelta = view.lookYawRate * dt + mouseYaw;
    const pitchDelta = view.lookPitchRate * dt + mousePitch;
    if (view.camera === 'chase') {
      // The look input orbits the camera; with no look input the orbit stays where it was left
      // (and `recentre` brings it behind the aircraft again).
      r[R_ORBIT_YAW] = wrapPi((r[R_ORBIT_YAW] ?? 0) + yawDelta);
      r[R_ORBIT_PITCH] = Math.min(
        Math.max((r[R_ORBIT_PITCH] ?? 0) + pitchDelta, -ORBIT_PITCH_MAX),
        ORBIT_PITCH_MAX,
      );
      if (recentre) {
        r[R_ORBIT_YAW] = 0;
        r[R_ORBIT_PITCH] = 0;
      }
      return;
    }
    const lim = this.#cockpit.headLimits;
    const yawMax = lim.yawDeg * DEG;
    r[R_YAW + S_T] = Math.min(Math.max((r[R_YAW + S_T] ?? 0) + yawDelta, -yawMax), yawMax);
    r[R_PITCH + S_T] = Math.min(
      Math.max((r[R_PITCH + S_T] ?? 0) + pitchDelta, -lim.pitchDownDeg * DEG),
      lim.pitchUpDeg * DEG,
    );
    if (recentre) {
      r[R_YAW + S_T] = 0;
      r[R_PITCH + S_T] = 0;
    }
    springStep(r, R_YAW);
    springStep(r, R_PITCH);
    // The spring may overshoot a hair past a limit only if the limit moved; never past the limit.
    r[R_YAW + S_X] = Math.min(Math.max(r[R_YAW + S_X] ?? 0, -yawMax), yawMax);
    r[R_PITCH + S_X] = Math.min(
      Math.max(r[R_PITCH + S_X] ?? 0, -lim.pitchDownDeg * DEG),
      lim.pitchUpDeg * DEG,
    );
  }

  /** Shake angles (yaw, pitch) at the current time, radians, written to `out[0..2]`. */
  #shake(state: RigState, view: ViewState, out: Float64Array): void {
    const drive = shakeDrive(
      state.alpha,
      state.alphaWarn,
      state.groundSpeed,
      state.abLevel,
      state.onGround,
    );
    let amp = SHAKE_MAX_RAD * view.shake * drive;
    if (view.reducedMotion) amp = Math.min(amp, SHAKE_REDUCED_MAX_RAD);
    // The amplitude is the size of the whole deviation, over both axes: each gets its share.
    amp *= Math.SQRT1_2;
    const t = this.#r[R_TIME] ?? 0;
    // Two incommensurate sines per axis: deterministic, never repeating within a flight.
    out[0] = amp * 0.5 * (Math.sin(t * 61.7) + Math.sin(t * 97.3 + 1.3));
    out[1] = amp * 0.5 * (Math.sin(t * 53.1 + 0.7) + Math.sin(t * 89.9 + 2.9));
  }

  /** The cockpit eye and attitude. */
  #placeCockpit(state: RigState, view: ViewState): void {
    const r = this.#r;
    const p = state.ownshipPosBf;
    const q = state.ownshipQuat;
    const eye = this.#cockpit.eyeBodyM;
    // The load the pilot feels moves the eye down in the body (a sag), at most 2 cm.
    const g = view.reducedMotion
      ? 0
      : Math.min(Math.max((state.gLagged - 1) * G_OFFSET_PER_G_M, -G_OFFSET_MAX_M), G_OFFSET_MAX_M);
    const out = this.pose.eyeBf;
    const a = this.#args;
    a[0] = eye[0];
    a[1] = eye[1];
    a[2] = eye[2] + g;
    rotateInto(out, q, a);
    out[0] = (out[0] ?? 0) + (p[0] ?? 0);
    out[1] = (out[1] ?? 0) + (p[1] ?? 0);
    out[2] = (out[2] ?? 0) + (p[2] ?? 0);
    this.#clampAboveTerrain(out);

    const shake = this.#head; // [yaw, pitch] scratch, reused below as the head quaternion
    this.#shake(state, view, shake);
    a[0] = (r[R_YAW + S_X] ?? 0) + (shake[0] ?? 0);
    a[1] = (r[R_PITCH + S_X] ?? 0) + (shake[1] ?? 0);
    headQuat(this.#head, a);
    quatMulInto(this.pose.quatBf, q, this.#head);
  }

  /** Raises `eye` (BF) to the minimum clearance over the terrain below it. */
  #clampAboveTerrain(eye: Float64Array): void {
    const ground = this.#terrain.heightAt(eye[0] ?? 0, eye[1] ?? 0);
    if (Number.isFinite(ground) && (eye[2] ?? 0) < ground + MIN_CLEARANCE_M) {
      eye[2] = ground + MIN_CLEARANCE_M;
    }
  }

  /** The chase camera: behind the aircraft, lagging, over the arm test. */
  #placeChase(state: RigState, view: ViewState, dt: number): void {
    const r = this.#r;
    const p = state.ownshipPosBf;
    const px = p[0] ?? 0;
    const py = p[1] ?? 0;
    const pz = p[2] ?? 0;
    // Local up at the aircraft (the BF frame is the tangent plane of a sphere).
    const ux0 = px;
    const uy0 = py;
    const uz0 = pz + EARTH_RADIUS_M;
    const un = Math.hypot(ux0, uy0, uz0) || 1;
    const ux = ux0 / un;
    const uy = uy0 / un;
    const uz = uz0 / un;

    // The direction the aircraft is going: its velocity, blending to its nose below 20 m/s.
    const q = state.ownshipQuat;
    this.#args[0] = 1;
    this.#args[1] = 0;
    this.#args[2] = 0;
    rotateInto(this.#head, q, this.#args);
    let nx = this.#head[0] ?? 1;
    let ny = this.#head[1] ?? 0;
    let nz = this.#head[2] ?? 0;
    const vx = state.ownshipVelBf[0] ?? 0;
    const vy = state.ownshipVelBf[1] ?? 0;
    const vz = state.ownshipVelBf[2] ?? 0;
    const speed = Math.hypot(vx, vy, vz);
    const w = Math.min(Math.max(speed / CHASE_NOSE_BELOW_MPS, 0), 1);
    let dx = nx;
    let dy = ny;
    let dz = nz;
    if (speed > 1e-6) {
      dx = w * (vx / speed) + (1 - w) * nx;
      dy = w * (vy / speed) + (1 - w) * ny;
      dz = w * (vz / speed) + (1 - w) * nz;
    }
    const dn = Math.hypot(dx, dy, dz);
    if (dn > 1e-9) {
      dx /= dn;
      dy /= dn;
      dz /= dn;
    } else {
      dx = nx;
      dy = ny;
      dz = nz;
    }
    nx = dx;
    ny = dy;
    nz = dz;

    // The camera's wish: `distance` back and a fifth of that up, then turned round the aircraft by
    // the free-look orbit.
    const dist = view.chaseDistanceM;
    let bx = -nx * dist;
    let by = -ny * dist;
    let bz = -nz * dist;
    const orbitYaw = r[R_ORBIT_YAW] ?? 0;
    const orbitPitch = r[R_ORBIT_PITCH] ?? 0;
    if (orbitYaw !== 0) {
      // Rodrigues about local up.
      const c = Math.cos(orbitYaw);
      const s = Math.sin(orbitYaw);
      const dot = bx * ux + by * uy + bz * uz;
      const cx = uy * bz - uz * by;
      const cy = uz * bx - ux * bz;
      const cz = ux * by - uy * bx;
      bx = bx * c + cx * s + ux * dot * (1 - c);
      by = by * c + cy * s + uy * dot * (1 - c);
      bz = bz * c + cz * s + uz * dot * (1 - c);
    }
    // Split into the part along up and the part across it; the orbit pitch tilts the back vector.
    const along = bx * ux + by * uy + bz * uz;
    const hx = bx - along * ux;
    const hy = by - along * uy;
    const hz = bz - along * uz;
    const cosP = Math.cos(orbitPitch);
    const sinP = Math.sin(orbitPitch);
    const lift = CHASE_HEIGHT_FRACTION * dist + dist * sinP;
    const tx = px + hx * cosP + ux * lift;
    const ty = py + hy * cosP + uy * lift;
    const tz = pz + hz * cosP + uz * lift;

    // Follow with a lag; snap when there was no camera yet or the aircraft jumped (a respawn).
    const placed = ((r[R_FLAGS] ?? 0) & 2) !== 0;
    const cx0 = r[R_CAM_X] ?? 0;
    const cy0 = r[R_CAM_Y] ?? 0;
    const cz0 = r[R_CAM_Z] ?? 0;
    if (!placed || Math.hypot(tx - cx0, ty - cy0, tz - cz0) > TELEPORT_M) {
      r[R_CAM_X] = tx;
      r[R_CAM_Y] = ty;
      r[R_CAM_Z] = tz;
      r[R_FLAGS] = (r[R_FLAGS] ?? 0) | 2;
    } else {
      const k = 1 - Math.exp(-dt / CHASE_LAG_S);
      r[R_CAM_X] = cx0 + (tx - cx0) * k;
      r[R_CAM_Y] = cy0 + (ty - cy0) * k;
      r[R_CAM_Z] = cz0 + (tz - cz0) * k;
    }
    let ex = r[R_CAM_X];
    let ey = r[R_CAM_Y];
    let ez = r[R_CAM_Z];

    // The arm: from the aircraft towards the camera, marched against the terrain. If the ground
    // rises through it the camera comes in to just before that point (not closer than ARM_MIN_M).
    const ax = ex - px;
    const ay = ey - py;
    const az = ez - pz;
    const length = Math.hypot(ax, ay, az);
    if (length > ARM_MIN_M) {
      const steps = Math.ceil(length / ARM_STEP_M);
      let safe = 1;
      for (let i = 1; i <= steps; i++) {
        const f = i / steps;
        const ground = this.#terrain.heightAt(px + ax * f, py + ay * f);
        if (Number.isFinite(ground) && pz + az * f < ground + ARM_CLEARANCE_M) {
          safe = (i - 1) / steps;
          break;
        }
      }
      if (safe < 1) {
        const pulled = Math.max(safe, ARM_MIN_M / length);
        ex = px + ax * pulled;
        ey = py + ay * pulled;
        ez = pz + az * pulled;
      }
    }
    const eye = this.pose.eyeBf;
    eye[0] = ex;
    eye[1] = ey;
    eye[2] = ez;
    this.#clampAboveTerrain(eye);

    // Look at the aircraft, horizon level: forward towards it, down along local down.
    let fx = px - eye[0];
    let fy = py - eye[1];
    let fz = pz - eye[2];
    const fn = Math.hypot(fx, fy, fz);
    if (fn < 1e-6) {
      fx = nx;
      fy = ny;
      fz = nz;
    } else {
      fx /= fn;
      fy /= fn;
      fz /= fn;
    }
    // Shake nudges the look direction a hair (the chase camera shakes with the buffet too).
    const shake = this.#head;
    this.#shake(state, view, shake);
    const look = this.#args;
    look[0] = fx;
    look[1] = fy;
    look[2] = fz;
    look[3] = ux;
    look[4] = uy;
    look[5] = uz;
    look[6] = shake[0] ?? 0;
    look[7] = shake[1] ?? 0;
    quatFromLook(this.pose.quatBf, look);
  }

  /** The field of view: the setting, zoom or the speed boost, through a spring. */
  #fov(state: RigState, view: ViewState, chase: boolean): void {
    const r = this.#r;
    let target = view.fovDeg * DEG;
    if (!chase && view.zoom) target = ZOOM_FOV_DEG * DEG;
    if (chase && !view.reducedMotion) {
      const speedFraction = Math.min(Math.max(state.ownshipSpeed / CHASE_VMAX_MPS, 0), 1);
      target += CHASE_FOV_BOOST_DEG * DEG * speedFraction;
    }
    r[R_FOV + S_T] = target;
    springStep(r, R_FOV);
    this.pose.fovY = r[R_FOV + S_X] ?? target;
  }
}

/** Wraps an angle to `[-pi, pi]`. */
function wrapPi(a: number): number {
  const twoPi = 2 * Math.PI;
  return a - twoPi * Math.round(a / twoPi);
}

/** Rotates the vector `v[0..3]` by the quaternion `q` (`[w, x, y, z]`) into `out[0..3]`. */
function rotateInto(out: Float64Array, q: ArrayLike<number>, v: Float64Array): void {
  const w = q[0] ?? 1;
  const ux = q[1] ?? 0;
  const uy = q[2] ?? 0;
  const uz = q[3] ?? 0;
  const x = v[0] ?? 0;
  const y = v[1] ?? 0;
  const z = v[2] ?? 0;
  const tx = 2 * (uy * z - uz * y);
  const ty = 2 * (uz * x - ux * z);
  const tz = 2 * (ux * y - uy * x);
  out[0] = x + w * tx + (uy * tz - uz * ty);
  out[1] = y + w * ty + (uz * tx - ux * tz);
  out[2] = z + w * tz + (ux * ty - uy * tx);
}

/** `out = a * b` (Hamilton product), `[w, x, y, z]`. */
function quatMulInto(out: Float64Array, a: ArrayLike<number>, b: ArrayLike<number>): void {
  const aw = a[0] ?? 1;
  const ax = a[1] ?? 0;
  const ay = a[2] ?? 0;
  const az = a[3] ?? 0;
  const bw = b[0] ?? 1;
  const bx = b[1] ?? 0;
  const by = b[2] ?? 0;
  const bz = b[3] ?? 0;
  out[0] = aw * bw - ax * bx - ay * by - az * bz;
  out[1] = aw * bx + ax * bw + ay * bz - az * by;
  out[2] = aw * by - ax * bz + ay * bw + az * bx;
  out[3] = aw * bz + ax * by - ay * bx + az * bw;
}

/**
 * The head's rotation in the body frame: yaw to the right about the body's down axis, then pitch
 * up about the yawed right axis. (Body axes are forward, right, down: a positive turn about down
 * takes forward towards right, a positive turn about right takes forward up.)
 */
function headQuat(out: Float64Array, angles: Float64Array): void {
  const yaw = angles[0] ?? 0;
  const pitch = angles[1] ?? 0;
  const cy = Math.cos(yaw / 2);
  const sy = Math.sin(yaw / 2);
  const cp = Math.cos(pitch / 2);
  const sp = Math.sin(pitch / 2);
  out[0] = cy * cp;
  out[1] = -sy * sp;
  out[2] = cy * sp;
  out[3] = sy * cp;
}

/**
 * The attitude of a camera that looks along `look[0..3]` with local up `look[3..6]` (both unit, BF),
 * turned by `look[6]` (yaw) and `look[7]` (pitch) for shake: forward, right = down x forward, down.
 */
function quatFromLook(out: Float64Array, look: Float64Array): void {
  const fx = look[0] ?? 1;
  const fy = look[1] ?? 0;
  const fz = look[2] ?? 0;
  const ux = look[3] ?? 0;
  const uy = look[4] ?? 0;
  const uz = look[5] ?? 1;
  const yaw = look[6] ?? 0;
  const pitch = look[7] ?? 0;
  // down = -up; right = down x forward; then down again, exactly perpendicular.
  let rx = -uy * fz + uz * fy;
  let ry = -uz * fx + ux * fz;
  let rz = -ux * fy + uy * fx;
  const rn = Math.hypot(rx, ry, rz);
  if (rn < 1e-9) {
    // Looking straight up or down: pick any right.
    rx = 0;
    ry = 1;
    rz = 0;
  } else {
    rx /= rn;
    ry /= rn;
    rz /= rn;
  }
  const dx = fy * rz - fz * ry;
  const dy = fz * rx - fx * rz;
  const dz = fx * ry - fy * rx;
  // Rotation matrix with columns forward, right, down -> quaternion.
  const m00 = fx;
  const m10 = fy;
  const m20 = fz;
  const m01 = rx;
  const m11 = ry;
  const m21 = rz;
  const m02 = dx;
  const m12 = dy;
  const m22 = dz;
  const trace = m00 + m11 + m22;
  let w: number;
  let x: number;
  let y: number;
  let z: number;
  if (trace > 0) {
    const s = Math.sqrt(trace + 1) * 2;
    w = 0.25 * s;
    x = (m21 - m12) / s;
    y = (m02 - m20) / s;
    z = (m10 - m01) / s;
  } else if (m00 > m11 && m00 > m22) {
    const s = Math.sqrt(1 + m00 - m11 - m22) * 2;
    w = (m21 - m12) / s;
    x = 0.25 * s;
    y = (m01 + m10) / s;
    z = (m02 + m20) / s;
  } else if (m11 > m22) {
    const s = Math.sqrt(1 + m11 - m00 - m22) * 2;
    w = (m02 - m20) / s;
    x = (m01 + m10) / s;
    y = 0.25 * s;
    z = (m12 + m21) / s;
  } else {
    const s = Math.sqrt(1 + m22 - m00 - m11) * 2;
    w = (m10 - m01) / s;
    x = (m02 + m20) / s;
    y = (m12 + m21) / s;
    z = 0.25 * s;
  }
  out[0] = w;
  out[1] = x;
  out[2] = y;
  out[3] = z;
  if (yaw !== 0 || pitch !== 0) {
    // Shake: turn by (yaw, pitch) in the camera's own frame.
    const cy = Math.cos(yaw / 2);
    const sy = Math.sin(yaw / 2);
    const cp = Math.cos(pitch / 2);
    const sp = Math.sin(pitch / 2);
    const hw = cy * cp;
    const hx = -sy * sp;
    const hy = cy * sp;
    const hz = sy * cp;
    out[0] = w * hw - x * hx - y * hy - z * hz;
    out[1] = w * hx + x * hw + y * hz - z * hy;
    out[2] = w * hy - x * hz + y * hw + z * hx;
    out[3] = w * hz + x * hy - y * hx + z * hw;
  }
}
