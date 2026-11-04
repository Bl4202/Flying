/**
 * What the HUD needs to know, derived each frame from the snapshot (ADR-41): the aircraft's
 * attitude against the local horizon, its heading, where its velocity points in the HUD plane, and
 * the numbers the tapes and blocks show, in the units the symbology uses (knots, feet, feet per
 * minute). It lives in one typed array, so a frame makes no garbage.
 *
 * Conventions (each has a test):
 *
 * - The local frame at the aircraft: up is away from the Earth's centre, east is the polar axis
 *   crossed with up, north is up crossed with east. BF is a tangent plane at the anchor, so the
 *   Earth's centre is at `(0, 0, -R)` and the polar axis is `(0, cos lat, sin lat)` there.
 * - Pitch is the nose's elevation above the local horizontal; bank is positive with the right wing
 *   down; heading is true, clockwise from north, in [0, 360) degrees.
 * - The HUD plane is the body-fixed tangent plane `(u, v) = (tan az, tan el)`, right and up
 *   positive: the body direction `(1, u, -v)`.
 */
import { WORLD, OWNSHIP_FLAG2 } from '@flying/sim-bridge/layout';
import type { AircraftStatus } from '@flying/sim-bridge';

/** Indices into `HudState.v`. */
export const HS = {
  // Attitude, radians.
  PITCH: 0,
  BANK: 1,
  HEADING: 2,
  // Local up, east, north and the heading's horizontal unit vector, in body axes (3 each).
  UP_X: 3,
  UP_Y: 4,
  UP_Z: 5,
  EAST_X: 6,
  EAST_Y: 7,
  EAST_Z: 8,
  NORTH_X: 9,
  NORTH_Y: 10,
  NORTH_Z: 11,
  HEAD_X: 12,
  HEAD_Y: 13,
  HEAD_Z: 14,
  // The velocity in the HUD plane, mrad; 1 if it is ahead (drawn), 1 if caged to the boresight.
  FPM_U: 15,
  FPM_V: 16,
  FPM_AHEAD: 17,
  FPM_CAGED: 18,
  // Flight path angle and ground track, radians.
  GAMMA: 19,
  // Instruments.
  CAS_KN: 20,
  MACH: 21,
  ALT_FT: 22,
  AGL_FT: 23,
  VS_FPM: 24,
  NZ: 25,
  MAX_NZ: 26,
  N_MAX: 27,
  ALPHA_DEG: 28,
  THROTTLE_PCT: 29,
  AB_LEVEL: 30,
  FUEL_KG: 31,
  FUEL_FLOW_KGPM: 32,
  ENDURANCE_MIN: 33,
  GEAR_DOWN: 34,
  FLAPS: 35,
  BRAKES: 36,
  PARKING: 37,
  ASSIST: 38,
  WARNINGS: 39,
  TIME_S: 40,
  /** 1 if the aircraft is on the ground, 0 in the air. */
  ON_GROUND: 41,
} as const;

export const HUD_STATE_SIZE = 42;

const KN_PER_MPS = 1.943_844_492;
const FT_PER_M = 3.280_839_895;
const FPM_PER_MPS = 196.850_393_7;

export interface HudInput {
  /** The aircraft's reference point, BF metres (f64). */
  posBf: ArrayLike<number>;
  /** Body-to-BF attitude `[w, x, y, z]`. */
  quatBf: ArrayLike<number>;
  /** Velocity in BF, m/s. */
  velBf: ArrayLike<number>;
  status: AircraftStatus;
  /** The `WARNING` bits that are on. */
  warnings: number;
  /** Seconds, for flashing symbols. */
  timeS: number;
}

export class HudState {
  readonly v = new Float64Array(HUD_STATE_SIZE);
  /** The last horizontal direction of the nose, kept so the heading holds when the nose is vertical. */
  readonly #lastHeading = new Float64Array(1);
  /** The body-to-BF rotation (row by row) and a vector to turn into body axes: scratch, no garbage. */
  readonly #rot = new Float64Array(9);
  readonly #vec = new Float64Array(3);

  /** Register `i` (an `HS` index). */
  at(i: number): number {
    return this.v[i] as number;
  }

  /** Fills the state from the world. */
  update(i: HudInput): void {
    const v = this.v;
    const q = i.quatBf;
    const w = q[0] ?? 1;
    const x = q[1] ?? 0;
    const y = q[2] ?? 0;
    const z = q[3] ?? 0;
    // Body to BF.
    const r00 = 1 - 2 * (y * y + z * z);
    const r01 = 2 * (x * y - w * z);
    const r02 = 2 * (x * z + w * y);
    const r10 = 2 * (x * y + w * z);
    const r11 = 1 - 2 * (x * x + z * z);
    const r12 = 2 * (y * z - w * x);
    const r20 = 2 * (x * z - w * y);
    const r21 = 2 * (y * z + w * x);
    const r22 = 1 - 2 * (x * x + y * y);

    // The local frame at the aircraft, in BF.
    const px = i.posBf[0] ?? 0;
    const py = i.posBf[1] ?? 0;
    const pz = (i.posBf[2] ?? 0) + WORLD.EARTH_RADIUS_M;
    const pn = Math.hypot(px, py, pz) || 1;
    const ux = px / pn;
    const uy = py / pn;
    const uz = pz / pn;
    const cl = Math.cos(WORLD.ANCHOR_LAT_RAD);
    const sl = Math.sin(WORLD.ANCHOR_LAT_RAD);
    // east = axis x up, with axis = (0, cos lat, sin lat)
    let ex = cl * uz - sl * uy;
    let ey = sl * ux;
    let ez = -cl * ux;
    const en = Math.hypot(ex, ey, ez) || 1;
    ex /= en;
    ey /= en;
    ez /= en;
    // north = up x east
    const nx = uy * ez - uz * ey;
    const ny = uz * ex - ux * ez;
    const nz = ux * ey - uy * ex;

    // The same vectors in body axes: R transposed times the BF vector.
    const rot = this.#rot;
    rot[0] = r00;
    rot[1] = r01;
    rot[2] = r02;
    rot[3] = r10;
    rot[4] = r11;
    rot[5] = r12;
    rot[6] = r20;
    rot[7] = r21;
    rot[8] = r22;
    const vec = this.#vec;
    vec[0] = ux;
    vec[1] = uy;
    vec[2] = uz;
    this.#toBody(HS.UP_X);
    vec[0] = ex;
    vec[1] = ey;
    vec[2] = ez;
    this.#toBody(HS.EAST_X);
    vec[0] = nx;
    vec[1] = ny;
    vec[2] = nz;
    this.#toBody(HS.NORTH_X);

    const upX = v[HS.UP_X] as number;
    const upY = v[HS.UP_Y] as number;
    const upZ = v[HS.UP_Z] as number;
    v[HS.PITCH] = Math.asin(Math.min(Math.max(upX, -1), 1));
    v[HS.BANK] = Math.atan2(-upY, -upZ);
    // Heading: the nose's horizontal part against north and east.
    const fe = v[HS.EAST_X] as number;
    const fn = v[HS.NORTH_X] as number;
    let heading = this.#lastHeading[0] as number;
    if (Math.hypot(fe, fn) > 1e-3) heading = Math.atan2(fe, fn);
    this.#lastHeading[0] = heading;
    let wrapped = heading < 0 ? heading + 2 * Math.PI : heading;
    if (wrapped >= 2 * Math.PI - 1e-9) wrapped = 0; // a hair under a full turn is north
    v[HS.HEADING] = wrapped;
    // The horizontal direction the nose points (or last pointed), in body axes.
    const ch = Math.cos(heading);
    const sh = Math.sin(heading);
    v[HS.HEAD_X] = ch * (v[HS.NORTH_X] as number) + sh * (v[HS.EAST_X] as number);
    v[HS.HEAD_Y] = ch * (v[HS.NORTH_Y] as number) + sh * (v[HS.EAST_Y] as number);
    v[HS.HEAD_Z] = ch * (v[HS.NORTH_Z] as number) + sh * (v[HS.EAST_Z] as number);

    // The velocity in the HUD plane.
    const vx = i.velBf[0] ?? 0;
    const vy = i.velBf[1] ?? 0;
    const vz = i.velBf[2] ?? 0;
    const speed = Math.hypot(vx, vy, vz);
    const bx = r00 * vx + r10 * vy + r20 * vz;
    const by = r01 * vx + r11 * vy + r21 * vz;
    const bz = r02 * vx + r12 * vy + r22 * vz;
    const s = i.status;
    const cage = speed < CAGE_BELOW_MPS;
    v[HS.FPM_CAGED] = cage ? 1 : 0;
    if (!cage && bx > 1e-6) {
      v[HS.FPM_U] = (by / bx) * 1000;
      v[HS.FPM_V] = (-bz / bx) * 1000;
      v[HS.FPM_AHEAD] = 1;
    } else {
      v[HS.FPM_U] = 0;
      v[HS.FPM_V] = 0;
      v[HS.FPM_AHEAD] = cage ? 1 : 0;
    }
    v[HS.GAMMA] =
      speed > 1e-6
        ? Math.asin(Math.min(Math.max((vx * ux + vy * uy + vz * uz) / speed, -1), 1))
        : 0;

    v[HS.CAS_KN] = s.cas * KN_PER_MPS;
    v[HS.MACH] = s.mach;
    v[HS.ALT_FT] = s.altitude * FT_PER_M;
    v[HS.AGL_FT] = s.agl * FT_PER_M;
    v[HS.VS_FPM] = s.vs * FPM_PER_MPS;
    v[HS.NZ] = s.nz;
    v[HS.MAX_NZ] = s.maxNz;
    v[HS.N_MAX] = s.nMax;
    v[HS.ALPHA_DEG] = (s.alpha * 180) / Math.PI;
    v[HS.THROTTLE_PCT] = s.throttle * 100;
    v[HS.AB_LEVEL] = s.abLevel;
    v[HS.FUEL_KG] = s.fuel;
    v[HS.FUEL_FLOW_KGPM] = s.fuelFlow * 60;
    v[HS.ENDURANCE_MIN] = s.fuelFlow > 1e-6 ? s.fuel / s.fuelFlow / 60 : 0;
    v[HS.GEAR_DOWN] = s.gearPos > 0.5 ? 1 : 0;
    v[HS.FLAPS] = s.flapDetent;
    v[HS.BRAKES] = (s.brakeL + s.brakeR) / 2;
    v[HS.PARKING] = (s.flags2 & OWNSHIP_FLAG2.PARKING_BRAKE) !== 0 ? 1 : 0;
    v[HS.ON_GROUND] = (s.flags2 & OWNSHIP_FLAG2.WOW) !== 0 ? 1 : 0;
    v[HS.ASSIST] = s.assist;
    v[HS.WARNINGS] = i.warnings;
    v[HS.TIME_S] = i.timeS;
  }

  /** The scratch vector (BF) into body axes at `v[at..at+3]`: R transposed times it. */
  #toBody(at: number): void {
    const r = this.#rot;
    const a = this.#vec;
    const x = a[0] as number;
    const y = a[1] as number;
    const z = a[2] as number;
    this.v[at] = (r[0] as number) * x + (r[3] as number) * y + (r[6] as number) * z;
    this.v[at + 1] = (r[1] as number) * x + (r[4] as number) * y + (r[7] as number) * z;
    this.v[at + 2] = (r[2] as number) * x + (r[5] as number) * y + (r[8] as number) * z;
  }
}

/** The velocity vector is caged to the boresight below this speed, m/s (GDD 54.3). */
export const CAGE_BELOW_MPS = 20;
