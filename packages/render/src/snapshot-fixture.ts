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
  /** The test arena (`ARENA`): 0 rugged, 1 plain. */
  arena?: number;
  ownship?: FixtureBody & {
    speed?: number;
    /** Velocity in BF (east, north, up), m/s. */
    vel?: [number, number, number];
    /** The load factor the pilot feels, g. */
    gLagged?: number;
    /** Gear position, 0 up to 1 down. */
    gearPos?: number;
    /** Elevator deflection, radians. */
    elevatorRad?: number;
    /** Afterburner level, 0 to 1. */
    abLevel?: number;
    /** The assist level, 1 to 4 (default 2, Standard); 0 is a spectator, who has no HUD. */
    assist?: number;
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

/**
 * The attitude `[w, x, y, z]` for a heading (degrees clockwise from north), a pitch (nose up positive)
 * and a roll (right wing down positive): body forward, right and down in BF (east, north, up).
 */
export function attitudeQuat(
  headingDeg: number,
  pitchDeg: number,
  rollDeg: number,
): [number, number, number, number] {
  const d = Math.PI / 180;
  const h = headingDeg * d;
  const p = pitchDeg * d;
  const r = rollDeg * d;
  const f: [number, number, number] = [
    Math.sin(h) * Math.cos(p),
    Math.cos(h) * Math.cos(p),
    Math.sin(p),
  ];
  // Wings level: the right wing is horizontal, to the right of the heading; down is forward x right.
  const r0: [number, number, number] = [Math.cos(h), -Math.sin(h), 0];
  const d0: [number, number, number] = [
    f[1] * r0[2] - f[2] * r0[1],
    f[2] * r0[0] - f[0] * r0[2],
    f[0] * r0[1] - f[1] * r0[0],
  ];
  // Rolling right swings the right wing down and the belly out to the left.
  const right: [number, number, number] = [
    r0[0] * Math.cos(r) + d0[0] * Math.sin(r),
    r0[1] * Math.cos(r) + d0[1] * Math.sin(r),
    r0[2] * Math.cos(r) + d0[2] * Math.sin(r),
  ];
  const down: [number, number, number] = [
    d0[0] * Math.cos(r) - r0[0] * Math.sin(r),
    d0[1] * Math.cos(r) - r0[1] * Math.sin(r),
    d0[2] * Math.cos(r) - r0[2] * Math.sin(r),
  ];
  return quatFromColumns(f, right, down);
}

/** The unit quaternion `[w, x, y, z]` of the rotation whose columns are the images of x, y, z. */
export function quatFromColumns(
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
  dv.setUint8(SNAPSHOT.O_ARENA, s.arena ?? 0);
  const o = s.ownship ?? { pos: [0, -6000, 1000] };
  o.pos.forEach((v, k) => {
    dv.setFloat64(SNAPSHOT.O_POS_BF + 8 * k, v, true);
  });
  (o.quat ?? NORTH).forEach((v, k) => {
    dv.setFloat32(SNAPSHOT.O_QUAT + 4 * k, v, true);
  });
  dv.setFloat32(SNAPSHOT.O_TAS, o.speed ?? 180, true);
  (o.vel ?? [0, 0, 0]).forEach((v, k) => {
    dv.setFloat32(SNAPSHOT.O_VEL_BF + 4 * k, v, true);
  });
  dv.setFloat32(SNAPSHOT.O_G_LAGGED, o.gLagged ?? 1, true);
  dv.setFloat32(SNAPSHOT.O_GEAR_POS, o.gearPos ?? 1, true);
  dv.setFloat32(SNAPSHOT.O_AB_LEVEL, o.abLevel ?? 0, true);
  dv.setUint8(SNAPSHOT.O_ASSIST, o.assist ?? 2);
  dv.setFloat32(SNAPSHOT.O_SURFACES, o.elevatorRad ?? 0, true);
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
