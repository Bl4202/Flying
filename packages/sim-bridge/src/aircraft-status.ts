/**
 * The ownship as the cockpit-side code needs it: assist level, flight control mode, wheels and
 * flaps, the g-physiology numbers and where the nose points. Read from a snapshot into one reused
 * object, so the main thread can do it every animation frame without making garbage.
 */
import type { SnapshotReader } from './snapshot.ts';

export interface AircraftStatus {
  /** Changes at every (re)spawn. */
  spawnSeq: number;
  /** `ASSIST_LEVEL` in force (0 for the spectator). */
  assist: number;
  /** `FCS_MODE`. */
  fcsMode: number;
  /** `OWNSHIP_FLAG` bits. */
  flags: number;
  /** `OWNSHIP_FLAG2` bits. */
  flags2: number;
  throttle: number;
  /** True and calibrated airspeed, m/s. */
  tas: number;
  cas: number;
  mach: number;
  /** Angle of attack, radians. */
  alpha: number;
  nz: number;
  gearPos: number;
  /** 0 up, 1 take-off, 2 landing. */
  flapDetent: number;
  airbrake: number;
  brakeL: number;
  brakeR: number;
  groundSpeed: number;
  /** Vertical speed, m/s. */
  vs: number;
  /** Lowest wheel above the ground, metres. */
  agl: number;
  /** The pilot's lagged g, and the stress `S` and red-out `R` it causes. */
  gLagged: number;
  gStress: number;
  redout: number;
  nCmd: number;
  gearBits: number;
  /** `CRASH_CAUSE`. */
  crashCause: number;
  /** Where the nose points: azimuth clockwise from north, elevation up, radians. */
  noseAz: number;
  noseEl: number;
}

/** An all-zero status. */
export function newAircraftStatus(): AircraftStatus {
  return {
    spawnSeq: 0,
    assist: 0,
    fcsMode: 0,
    flags: 0,
    flags2: 0,
    throttle: 0,
    tas: 0,
    cas: 0,
    mach: 0,
    alpha: 0,
    nz: 0,
    gearPos: 0,
    flapDetent: 0,
    airbrake: 0,
    brakeL: 0,
    brakeR: 0,
    groundSpeed: 0,
    vs: 0,
    agl: 0,
    gLagged: 0,
    gStress: 0,
    redout: 0,
    nCmd: 0,
    gearBits: 0,
    crashCause: 0,
    noseAz: 0,
    noseEl: 0,
  };
}

const quat = new Float32Array(4);
const brakes = new Float32Array(2);

/** Fills `out` from the newest snapshot. Makes no garbage beyond what the reader's accessors do. */
export function readAircraftStatus(reader: SnapshotReader, out: AircraftStatus): void {
  out.spawnSeq = reader.ownshipSpawnSeq();
  out.assist = reader.ownshipAssist();
  out.fcsMode = reader.ownshipFcsMode();
  out.flags = reader.ownshipFlags();
  out.flags2 = reader.ownshipFlags2();
  out.throttle = reader.ownshipThrottle();
  out.tas = reader.ownshipTas();
  out.cas = reader.ownshipCas();
  out.mach = reader.ownshipMach();
  out.alpha = reader.ownshipAlpha();
  out.nz = reader.ownshipNz();
  out.gearPos = reader.ownshipGearPos();
  out.flapDetent = reader.ownshipFlapDetent();
  out.airbrake = reader.ownshipAirbrake();
  reader.ownshipBrakes(brakes);
  out.brakeL = brakes[0] ?? 0;
  out.brakeR = brakes[1] ?? 0;
  out.groundSpeed = reader.ownshipGroundSpeed();
  out.vs = reader.ownshipVs();
  out.agl = reader.ownshipAgl();
  out.gLagged = reader.ownshipGLagged();
  out.gStress = reader.ownshipGStress();
  out.redout = reader.ownshipRedout();
  out.nCmd = reader.ownshipNCmd();
  out.gearBits = reader.ownshipGearBits();
  out.crashCause = reader.ownshipCrashCause();

  // The nose is the body x axis. For a body-to-BF quaternion [w, x, y, z], rotated, its east,
  // north and up components are:
  reader.ownshipQuat(quat);
  const w = quat[0] ?? 1;
  const x = quat[1] ?? 0;
  const y = quat[2] ?? 0;
  const z = quat[3] ?? 0;
  const east = 1 - 2 * (y * y + z * z);
  const north = 2 * (x * y + w * z);
  const up = 2 * (x * z - w * y);
  out.noseAz = Math.atan2(east, north);
  out.noseEl = Math.asin(Math.min(Math.max(up, -1), 1));
}
