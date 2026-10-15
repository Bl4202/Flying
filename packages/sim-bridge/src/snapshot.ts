/**
 * Snapshot v2: what the sim publishes each tick and the renderer draws from. Layout in
 * `layout.gen.ts` (from Rust); this file reads and stamps it.
 *
 * Positions are battlefield-frame `f64`, orientations `f32` quaternions `[w, x, y, z]`, bodies
 * are structure-of-arrays so a whole axis is contiguous. The ownship block is the player's
 * aircraft (its reference point, attitude, air data, engine and fuel) or, with no aircraft
 * content, the spectator (`OWNSHIP_FLAG.SPECTATOR`; air data zero). Angles are radians.
 */
import { SNAPSHOT } from './layout.gen.ts';

/** Reads a snapshot in place. Accessors copy out; nothing here keeps a reference to a result. */
export class SnapshotReader {
  readonly bytes: Uint8Array;
  readonly #dv: DataView;

  constructor(bytes: Uint8Array) {
    if (bytes.byteLength < SNAPSHOT.SIZE) {
      throw new RangeError(
        `a snapshot is ${String(SNAPSHOT.SIZE)} bytes, got ${String(bytes.byteLength)}`,
      );
    }
    this.bytes = bytes;
    this.#dv = new DataView(bytes.buffer, bytes.byteOffset, SNAPSHOT.SIZE);
  }

  /** True if the magic and version are the ones this build reads. */
  valid(): boolean {
    return (
      this.#dv.getUint32(SNAPSHOT.H_MAGIC, true) === SNAPSHOT.MAGIC &&
      this.#dv.getUint32(SNAPSHOT.H_VERSION, true) === SNAPSHOT.VERSION
    );
  }

  tickBig(): bigint {
    return this.#dv.getBigUint64(SNAPSHOT.H_TICK, true);
  }

  /** The tick as a number (exact below 2^53 ticks, which is 4.7 million years). */
  tick(): number {
    return Number(this.tickBig());
  }

  /** When the host published this copy, milliseconds on the shared clock (0 if not stamped). */
  publishTimeMs(): number {
    return this.#dv.getFloat64(SNAPSHOT.H_PUBLISH_TIME_MS, true);
  }

  lastInputSeq(): number {
    return this.#dv.getUint32(SNAPSHOT.H_LAST_INPUT_SEQ, true);
  }

  bodyCount(): number {
    return this.#dv.getUint32(SNAPSHOT.H_BODY_COUNT, true);
  }

  /** `CORE_STATE` of the core when the snapshot was written. */
  coreState(): number {
    return this.#dv.getUint32(SNAPSHOT.H_CORE_STATE, true);
  }

  /** When the host sampled the newest input the core had applied, milliseconds. */
  inputTimeMs(): number {
    return this.#dv.getFloat64(SNAPSHOT.H_INPUT_T_MS, true);
  }

  /** Ownship position (the aircraft's reference point), BF metres. */
  ownshipPos(out: Float64Array, at = 0): void {
    for (let i = 0; i < 3; i++) {
      out[at + i] = this.#dv.getFloat64(SNAPSHOT.O_POS_BF + 8 * i, true);
    }
  }

  /** Ownship attitude, body to BF, `[w, x, y, z]`. */
  ownshipQuat(out: Float32Array, at = 0): void {
    for (let i = 0; i < 4; i++) {
      out[at + i] = this.#dv.getFloat32(SNAPSHOT.O_QUAT + 4 * i, true);
    }
  }

  /** Ownship velocity, BF m/s. */
  ownshipVel(out: Float32Array, at = 0): void {
    for (let i = 0; i < 3; i++) {
      out[at + i] = this.#dv.getFloat32(SNAPSHOT.O_VEL_BF + 4 * i, true);
    }
  }

  /** True airspeed, m/s. */
  ownshipTas(): number {
    return this.#dv.getFloat32(SNAPSHOT.O_TAS, true);
  }

  /** Calibrated airspeed, m/s. */
  ownshipCas(): number {
    return this.#dv.getFloat32(SNAPSHOT.O_CAS, true);
  }

  ownshipMach(): number {
    return this.#dv.getFloat32(SNAPSHOT.O_MACH, true);
  }

  /** Angle of attack, radians. */
  ownshipAlpha(): number {
    return this.#dv.getFloat32(SNAPSHOT.O_ALPHA, true);
  }

  /** Sideslip, radians. */
  ownshipBeta(): number {
    return this.#dv.getFloat32(SNAPSHOT.O_BETA, true);
  }

  /** Body-axis load factor, g (1 in level flight). */
  ownshipNz(): number {
    return this.#dv.getFloat32(SNAPSHOT.O_NZ, true);
  }

  /** Throttle lever, 0 to 1. */
  ownshipThrottle(): number {
    return this.#dv.getFloat32(SNAPSHOT.O_THROTTLE, true);
  }

  /** Net thrust, newtons. */
  ownshipThrust(): number {
    return this.#dv.getFloat32(SNAPSHOT.O_THRUST, true);
  }

  /** Fuel on board, kg. */
  ownshipFuel(): number {
    return this.#dv.getFloat32(SNAPSHOT.O_FUEL, true);
  }

  /** Mass, kg. */
  ownshipMass(): number {
    return this.#dv.getFloat32(SNAPSHOT.O_MASS, true);
  }

  /** Body rates (roll, pitch, yaw), rad/s. */
  ownshipOmega(out: Float32Array, at = 0): void {
    for (let i = 0; i < 3; i++) {
      out[at + i] = this.#dv.getFloat32(SNAPSHOT.O_OMEGA + 4 * i, true);
    }
  }

  /** Elevator, aileron and rudder, radians. */
  ownshipSurfaces(out: Float32Array, at = 0): void {
    for (let i = 0; i < 3; i++) {
      out[at + i] = this.#dv.getFloat32(SNAPSHOT.O_SURFACES + 4 * i, true);
    }
  }

  /** Height above MSL, metres. */
  ownshipAltitude(): number {
    return this.#dv.getFloat32(SNAPSHOT.O_ALTITUDE, true);
  }

  /** Afterburner stages burning. */
  ownshipAbStage(): number {
    return this.#dv.getUint8(SNAPSHOT.O_AB_STAGE);
  }

  /** `OWNSHIP_FLAG` bits. */
  ownshipFlags(): number {
    return this.#dv.getUint8(SNAPSHOT.O_FLAGS);
  }

  /** Body `i` position, BF metres. */
  bodyPos(i: number, out: Float64Array, at = 0): void {
    out[at] = this.#dv.getFloat64(SNAPSHOT.B_X + 8 * i, true);
    out[at + 1] = this.#dv.getFloat64(SNAPSHOT.B_Y + 8 * i, true);
    out[at + 2] = this.#dv.getFloat64(SNAPSHOT.B_Z + 8 * i, true);
  }

  /** Body `i` orientation `[w, x, y, z]`. */
  bodyQuat(i: number, out: Float32Array, at = 0): void {
    for (let k = 0; k < 4; k++) {
      out[at + k] = this.#dv.getFloat32(SNAPSHOT.B_QUAT + 16 * i + 4 * k, true);
    }
  }
}

/** Writes the host's publish time into a snapshot copy. Used by the Sim Worker only. */
export function stampPublishTime(bytes: Uint8Array, ms: number): void {
  new DataView(bytes.buffer, bytes.byteOffset, SNAPSHOT.SIZE).setFloat64(
    SNAPSHOT.H_PUBLISH_TIME_MS,
    ms,
    true,
  );
}
