/** Builds snapshots for tests and renderer self-tests. Not production code. */
import { SNAPSHOT } from '@flying/sim-bridge/layout';

export interface FixtureBody {
  pos: [number, number, number];
  /** `[w, x, y, z]`. */
  quat?: [number, number, number, number];
}

export interface FixtureSnapshot {
  tick?: number;
  publishMs?: number;
  inputMs?: number;
  lastInputSeq?: number;
  coreState?: number;
  ownship?: FixtureBody & {
    speed?: number;
    throttle?: number;
    flags?: number;
    flags2?: number;
    gStress?: number;
    redout?: number;
    /** Mouse-aim direction: azimuth, elevation, radians. */
    aim?: [number, number];
  };
  bodies?: FixtureBody[];
}

/** Heading north, wings level: the attitude the ownship spawns with. */
export const NORTH: [number, number, number, number] = [0, Math.SQRT1_2, Math.SQRT1_2, 0];

/**
 * Nose straight up, right wing east: a rotation of 120 degrees about -(1,1,1), which sends body
 * x (forward) to BF up, body y (right) to BF east and body z (down) to BF north.
 */
export const LOOKING_UP: [number, number, number, number] = [0.5, -0.5, -0.5, -0.5];

/**
 * Nose straight down, right wing east: forward goes to BF down, right to east, and the body's down
 * axis to south.
 */
export const LOOKING_DOWN: [number, number, number, number] = quatFromColumns(
  [0, 0, -1],
  [1, 0, 0],
  [0, -1, 0],
);

/** The unit quaternion `[w, x, y, z]` of the rotation whose columns are the images of x, y, z. */
function quatFromColumns(
  cx: [number, number, number],
  cy: [number, number, number],
  cz: [number, number, number],
): [number, number, number, number] {
  const [m00, m10, m20] = cx;
  const [m01, m11, m21] = cy;
  const [m02, m12, m22] = cz;
  const trace = m00 + m11 + m22;
  let q: [number, number, number, number];
  if (trace > 0) {
    const s = Math.sqrt(trace + 1) * 2;
    q = [0.25 * s, (m21 - m12) / s, (m02 - m20) / s, (m10 - m01) / s];
  } else if (m00 > m11 && m00 > m22) {
    const s = Math.sqrt(1 + m00 - m11 - m22) * 2;
    q = [(m21 - m12) / s, 0.25 * s, (m01 + m10) / s, (m02 + m20) / s];
  } else if (m11 > m22) {
    const s = Math.sqrt(1 + m11 - m00 - m22) * 2;
    q = [(m02 - m20) / s, (m01 + m10) / s, 0.25 * s, (m12 + m21) / s];
  } else {
    const s = Math.sqrt(1 + m22 - m00 - m11) * 2;
    q = [(m10 - m01) / s, (m02 + m20) / s, (m12 + m21) / s, 0.25 * s];
  }
  return q;
}

export function makeSnapshot(s: FixtureSnapshot = {}): Uint8Array {
  const out = new Uint8Array(SNAPSHOT.SIZE);
  const dv = new DataView(out.buffer);
  dv.setUint32(SNAPSHOT.H_MAGIC, SNAPSHOT.MAGIC, true);
  dv.setUint32(SNAPSHOT.H_VERSION, SNAPSHOT.VERSION, true);
  dv.setBigUint64(SNAPSHOT.H_TICK, BigInt(s.tick ?? 1), true);
  dv.setFloat64(SNAPSHOT.H_PUBLISH_TIME_MS, s.publishMs ?? 0, true);
  dv.setUint32(SNAPSHOT.H_LAST_INPUT_SEQ, s.lastInputSeq ?? 0, true);
  dv.setUint32(SNAPSHOT.H_CORE_STATE, s.coreState ?? 2, true);
  dv.setFloat64(SNAPSHOT.H_INPUT_T_MS, s.inputMs ?? 0, true);
  const o = s.ownship ?? { pos: [0, -6000, 1000] };
  o.pos.forEach((v, k) => {
    dv.setFloat64(SNAPSHOT.O_POS_BF + 8 * k, v, true);
  });
  (o.quat ?? NORTH).forEach((v, k) => {
    dv.setFloat32(SNAPSHOT.O_QUAT + 4 * k, v, true);
  });
  dv.setFloat32(SNAPSHOT.O_TAS, o.speed ?? 180, true);
  dv.setFloat32(SNAPSHOT.O_THROTTLE, o.throttle ?? 0.5, true);
  dv.setUint8(SNAPSHOT.O_FLAGS, o.flags ?? 0);
  dv.setUint16(SNAPSHOT.O_FLAGS2, o.flags2 ?? 0, true);
  dv.setFloat32(SNAPSHOT.O_G_STRESS, o.gStress ?? 0, true);
  dv.setFloat32(SNAPSHOT.O_REDOUT, o.redout ?? 0, true);
  dv.setFloat32(SNAPSHOT.O_AIM_AZ, o.aim?.[0] ?? 0, true);
  dv.setFloat32(SNAPSHOT.O_AIM_EL, o.aim?.[1] ?? 0, true);
  const bodies = s.bodies ?? [];
  dv.setUint32(SNAPSHOT.H_BODY_COUNT, bodies.length, true);
  bodies.forEach((b, i) => {
    dv.setFloat64(SNAPSHOT.B_X + 8 * i, b.pos[0], true);
    dv.setFloat64(SNAPSHOT.B_Y + 8 * i, b.pos[1], true);
    dv.setFloat64(SNAPSHOT.B_Z + 8 * i, b.pos[2], true);
    (b.quat ?? NORTH).forEach((v, k) => {
      dv.setFloat32(SNAPSHOT.B_QUAT + 16 * i + 4 * k, v, true);
    });
  });
  return out;
}
