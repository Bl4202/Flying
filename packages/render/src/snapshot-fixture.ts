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
  ownship?: FixtureBody & { speed?: number; throttle?: number; flags?: number };
  bodies?: FixtureBody[];
}

/** Heading north, wings level: the attitude the ownship spawns with. */
export const NORTH: [number, number, number, number] = [0, Math.SQRT1_2, Math.SQRT1_2, 0];

/**
 * Nose straight up, right wing east: a rotation of 120 degrees about -(1,1,1), which sends body
 * x (forward) to BF up, body y (right) to BF east and body z (down) to BF north.
 */
export const LOOKING_UP: [number, number, number, number] = [0.5, -0.5, -0.5, -0.5];

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
