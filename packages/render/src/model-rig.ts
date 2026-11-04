/**
 * Moves the parts of the exterior model with the sim (ADR-41): control surfaces, flaps, the
 * airbrake, the gear and the afterburner glow follow the snapshot through the channels the model
 * data binds them to, with no animation data. Each frame `update` writes one matrix per part into
 * the model uniform block: the part's rotation about its hinge, then the airframe's attitude, then
 * the airframe's position relative to the eye, in render axes. It makes no objects.
 *
 * The channel units and signs are the sim's (`crates/flight/src/actuators.rs`):
 *
 * | channel | unit | positive means |
 * |---|---|---|
 * | `elevator_deg`, `aileron_deg`, `rudder_deg`, `flap_deg` | degrees | trailing edge down (elevator, flap), right wing down (aileron), trailing edge left (rudder) |
 * | `airbrake` | 0 stowed to 1 open | |
 * | `gear_retract` | 0 down and locked to 1 up | |
 * | `afterburner` | 0 to 1 | |
 */
import { MODEL_MATRICES, MODEL_UNIFORM_FLOATS } from './model-mesh.ts';
import type { ModelChannel, ModelSpec } from './present.ts';

const CHANNELS: readonly ModelChannel[] = [
  'elevator_deg',
  'aileron_deg',
  'rudder_deg',
  'flap_deg',
  'airbrake',
  'gear_retract',
  'afterburner',
];
const DEG = Math.PI / 180;

/** The aircraft as the model needs it, each frame. */
export interface ModelState {
  /** The aircraft's reference point, BF metres (f64). */
  ownshipPosBf: ArrayLike<number>;
  /** Body-to-BF attitude `[w, x, y, z]`. */
  ownshipQuat: ArrayLike<number>;
  /** Elevator, aileron and rudder, radians; the trailing-edge flap, radians. */
  elevatorRad: number;
  aileronRad: number;
  rudderRad: number;
  flapRad: number;
  /** Airbrake, 0 stowed to 1 open. */
  airbrake: number;
  /** Gear position, 0 up to 1 down (the snapshot's). */
  gearPos: number;
  /** Afterburner level, 0 to 1. */
  abLevel: number;
}

/** One part, ready to animate. */
interface Part {
  glow: boolean;
  channel: number;
  pivot: [number, number, number];
  axis: [number, number, number];
  degPerUnit: number;
}

export class ModelRig {
  readonly #parts: Part[];
  readonly #channels = new Float64Array(CHANNELS.length);
  /** The matrix being built: the 3x3 row by row, then the translation (no number crosses a call). */
  readonly #m = new Float64Array(12);

  constructor(spec: ModelSpec) {
    this.#parts = spec.parts.map((p): Part => {
      const channel = CHANNELS.indexOf(p.channel);
      if (p.motion === 'glow') {
        return { glow: true, channel, pivot: [0, 0, 0], axis: [1, 0, 0], degPerUnit: 0 };
      }
      const len = Math.hypot(p.axis[0], p.axis[1], p.axis[2]) || 1;
      return {
        glow: false,
        channel,
        pivot: [p.pivotM[0], p.pivotM[1], p.pivotM[2]],
        axis: [p.axis[0] / len, p.axis[1] / len, p.axis[2] / len],
        degPerUnit: p.rotateDegPerUnit,
      };
    });
  }

  /** How many parts the model has. */
  get partCount(): number {
    return this.#parts.length;
  }

  /** The value of each channel for a state: degrees, 0-to-1 values, in `CHANNELS` order. */
  #setChannels(s: ModelState): void {
    const c = this.#channels;
    c[0] = s.elevatorRad / DEG;
    c[1] = s.aileronRad / DEG;
    c[2] = s.rudderRad / DEG;
    c[3] = s.flapRad / DEG;
    c[4] = s.airbrake;
    c[5] = 1 - s.gearPos;
    c[6] = s.abLevel;
  }

  /**
   * Writes the model uniform block: a column-major matrix for the fixed airframe and one per part
   * (taking a point of the model in body axes to eye-relative render axes), then the glow level.
   */
  update(out: Float32Array, s: ModelState, eyeBf: ArrayLike<number>): void {
    this.#setChannels(s);
    const q = s.ownshipQuat;
    const w = q[0] ?? 1;
    const x = q[1] ?? 0;
    const y = q[2] ?? 0;
    const z = q[3] ?? 0;
    // Body to BF (east, north, up), then to render axes (east, up, south): rows 0, 2 and -1.
    const r00 = 1 - 2 * (y * y + z * z);
    const r01 = 2 * (x * y - w * z);
    const r02 = 2 * (x * z + w * y);
    const r10 = 2 * (x * y + w * z);
    const r11 = 1 - 2 * (x * x + z * z);
    const r12 = 2 * (y * z - w * x);
    const r20 = 2 * (x * z - w * y);
    const r21 = 2 * (y * z + w * x);
    const r22 = 1 - 2 * (x * x + y * y);
    const b00 = r00;
    const b01 = r01;
    const b02 = r02;
    const b10 = r20;
    const b11 = r21;
    const b12 = r22;
    const b20 = -r10;
    const b21 = -r11;
    const b22 = -r12;
    // The airframe's position relative to the eye, in render axes (f64 subtraction first).
    const ox = (s.ownshipPosBf[0] ?? 0) - (eyeBf[0] ?? 0);
    const oy = (s.ownshipPosBf[2] ?? 0) - (eyeBf[2] ?? 0);
    const oz = -((s.ownshipPosBf[1] ?? 0) - (eyeBf[1] ?? 0));

    const m = this.#m;
    m[0] = b00;
    m[1] = b01;
    m[2] = b02;
    m[3] = b10;
    m[4] = b11;
    m[5] = b12;
    m[6] = b20;
    m[7] = b21;
    m[8] = b22;
    m[9] = ox;
    m[10] = oy;
    m[11] = oz;
    this.#store(out, 0);
    const parts = this.#parts;
    for (let i = 0; i < parts.length; i++) {
      const p = parts[i] as Part;
      if (p.glow) {
        // The glow part does not move; its rear face is lit by the glow level.
        m[0] = b00;
        m[1] = b01;
        m[2] = b02;
        m[3] = b10;
        m[4] = b11;
        m[5] = b12;
        m[6] = b20;
        m[7] = b21;
        m[8] = b22;
        m[9] = ox;
        m[10] = oy;
        m[11] = oz;
        this.#store(out, i + 1);
        continue;
      }
      const angle = (this.#channels[p.channel] ?? 0) * p.degPerUnit * DEG;
      const c = Math.cos(angle);
      const sn = Math.sin(angle);
      const k = 1 - c;
      const ax = p.axis[0];
      const ay = p.axis[1];
      const az = p.axis[2];
      // Rodrigues: rotation about the unit axis by the angle.
      const m00 = c + ax * ax * k;
      const m01 = ax * ay * k - az * sn;
      const m02 = ax * az * k + ay * sn;
      const m10 = ay * ax * k + az * sn;
      const m11 = c + ay * ay * k;
      const m12 = ay * az * k - ax * sn;
      const m20 = az * ax * k - ay * sn;
      const m21 = az * ay * k + ax * sn;
      const m22 = c + az * az * k;
      // M = Rb * Rot, and the translation (P - Rot P) carried through Rb.
      const n00 = b00 * m00 + b01 * m10 + b02 * m20;
      const n01 = b00 * m01 + b01 * m11 + b02 * m21;
      const n02 = b00 * m02 + b01 * m12 + b02 * m22;
      const n10 = b10 * m00 + b11 * m10 + b12 * m20;
      const n11 = b10 * m01 + b11 * m11 + b12 * m21;
      const n12 = b10 * m02 + b11 * m12 + b12 * m22;
      const n20 = b20 * m00 + b21 * m10 + b22 * m20;
      const n21 = b20 * m01 + b21 * m11 + b22 * m21;
      const n22 = b20 * m02 + b21 * m12 + b22 * m22;
      const px = p.pivot[0];
      const py = p.pivot[1];
      const pz = p.pivot[2];
      const dx = px - (m00 * px + m01 * py + m02 * pz);
      const dy = py - (m10 * px + m11 * py + m12 * pz);
      const dz = pz - (m20 * px + m21 * py + m22 * pz);
      m[0] = n00;
      m[1] = n01;
      m[2] = n02;
      m[3] = n10;
      m[4] = n11;
      m[5] = n12;
      m[6] = n20;
      m[7] = n21;
      m[8] = n22;
      m[9] = ox + b00 * dx + b01 * dy + b02 * dz;
      m[10] = oy + b10 * dx + b11 * dy + b12 * dz;
      m[11] = oz + b20 * dx + b21 * dy + b22 * dz;
      this.#store(out, i + 1);
    }
    // Matrices past the last part are never read; keep them identity-sized and finite.
    m.fill(0);
    m[0] = m[4] = m[8] = 1;
    for (let i = parts.length + 1; i < MODEL_MATRICES; i++) this.#store(out, i);
    out[MODEL_MATRICES * 16] = s.abLevel;
    out[MODEL_MATRICES * 16 + 1] = 0;
    out[MODEL_MATRICES * 16 + 2] = 0;
    out[MODEL_MATRICES * 16 + 3] = 0;
  }

  /** Writes the scratch matrix, column-major, as matrix `index` of the block. */
  #store(out: Float32Array, index: number): void {
    const m = this.#m;
    const at = index * 16;
    out[at] = m[0] as number;
    out[at + 1] = m[3] as number;
    out[at + 2] = m[6] as number;
    out[at + 3] = 0;
    out[at + 4] = m[1] as number;
    out[at + 5] = m[4] as number;
    out[at + 6] = m[7] as number;
    out[at + 7] = 0;
    out[at + 8] = m[2] as number;
    out[at + 9] = m[5] as number;
    out[at + 10] = m[8] as number;
    out[at + 11] = 0;
    out[at + 12] = m[9] as number;
    out[at + 13] = m[10] as number;
    out[at + 14] = m[11] as number;
    out[at + 15] = 1;
  }
}

/** A fresh model uniform block. */
export function newModelUniform(): Float32Array {
  return new Float32Array(MODEL_UNIFORM_FLOATS);
}
