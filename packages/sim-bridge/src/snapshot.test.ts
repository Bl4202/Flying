import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { SNAPSHOT } from './layout.gen.ts';
import { SnapshotReader, stampPublishTime } from './snapshot.ts';

interface Decoded {
  magic: number;
  version: number;
  tick: number;
  publish_time_ms: number;
  last_input_seq: number;
  body_count: number;
  core_state: number;
  input_t_ms: number;
  ownship: {
    pos_bf: number[];
    quat: number[];
    vel_bf: number[];
    tas: number;
    cas: number;
    mach: number;
    alpha: number;
    beta: number;
    nz: number;
    throttle: number;
    thrust: number;
    fuel: number;
    mass: number;
    omega: number[];
    surfaces: number[];
    altitude: number;
    ab_stage: number;
    flags: number;
    assist: number;
    fcs_mode: number;
    flags2: number;
    spawn_seq: number;
    flap_detent: number;
    gear_pos: number;
    tef: number;
    airbrake: number;
    brake: number[];
    steer: number;
    ground_speed: number;
    vs: number;
    agl: number;
    g_lagged: number;
    g_stress: number;
    redout: number;
    n_cmd: number;
    touchdown_vs: number;
    aim: number[];
    crash_cause: number;
    gear_bits: number;
  };
  bodies: { pos_bf: number[]; quat: number[] }[];
}
const vector = JSON.parse(
  readFileSync(new URL('../../../tests/vectors/snapshot_v3.json', import.meta.url), 'utf8'),
) as { size: number; hex: string; decoded: Decoded };

const bytes = (): Uint8Array =>
  Uint8Array.from(vector.hex.match(/../g) ?? [], (h) => Number.parseInt(h, 16));

describe('SnapshotReader against the Rust reference snapshot', () => {
  const d = vector.decoded;

  it('has the size the layout says', () => {
    expect(vector.size).toBe(SNAPSHOT.SIZE);
    expect(bytes().byteLength).toBe(SNAPSHOT.SIZE);
  });

  it('reads the header', () => {
    const r = new SnapshotReader(bytes());
    expect(r.valid()).toBe(true);
    expect(d.magic).toBe(SNAPSHOT.MAGIC);
    expect(r.tick()).toBe(d.tick);
    expect(r.tickBig()).toBe(BigInt(d.tick));
    expect(r.publishTimeMs()).toBe(d.publish_time_ms);
    expect(r.lastInputSeq()).toBe(d.last_input_seq);
    expect(r.bodyCount()).toBe(d.body_count);
    expect(r.coreState()).toBe(d.core_state);
    expect(r.inputTimeMs()).toBe(d.input_t_ms);
  });

  it('reads the ownship (the aircraft rolling on the runway, in this vector)', () => {
    const r = new SnapshotReader(bytes());
    const o = d.ownship;
    const pos = new Float64Array(3);
    const quat = new Float32Array(4);
    const vel = new Float32Array(3);
    const omega = new Float32Array(3);
    const surfaces = new Float32Array(3);
    r.ownshipPos(pos);
    r.ownshipQuat(quat);
    r.ownshipVel(vel);
    r.ownshipOmega(omega);
    r.ownshipSurfaces(surfaces);
    expect([...pos]).toEqual(o.pos_bf);
    expect([...quat]).toEqual(o.quat);
    expect([...vel]).toEqual(o.vel_bf);
    expect([...omega]).toEqual(o.omega);
    expect([...surfaces]).toEqual(o.surfaces);
    expect([
      r.ownshipTas(),
      r.ownshipCas(),
      r.ownshipMach(),
      r.ownshipAlpha(),
      r.ownshipBeta(),
      r.ownshipNz(),
      r.ownshipThrottle(),
      r.ownshipThrust(),
      r.ownshipFuel(),
      r.ownshipMass(),
      r.ownshipAltitude(),
      r.ownshipAbStage(),
      r.ownshipFlags(),
    ]).toEqual([
      o.tas,
      o.cas,
      o.mach,
      o.alpha,
      o.beta,
      o.nz,
      o.throttle,
      o.thrust,
      o.fuel,
      o.mass,
      o.altitude,
      o.ab_stage,
      o.flags,
    ]);
    expect(o.tas, 'the aircraft is rolling').toBeGreaterThan(5);
  });

  it('reads the v3 block: flight control, gear, wheels, g and aim', () => {
    const r = new SnapshotReader(bytes());
    const o = d.ownship;
    const brakes = new Float32Array(2);
    const aim = new Float32Array(2);
    r.ownshipBrakes(brakes);
    r.ownshipAim(aim);
    expect([...brakes]).toEqual(o.brake);
    expect([...aim]).toEqual(o.aim);
    expect([
      r.ownshipAssist(),
      r.ownshipFcsMode(),
      r.ownshipFlags2(),
      r.ownshipSpawnSeq(),
      r.ownshipFlapDetent(),
      r.ownshipGearPos(),
      r.ownshipTef(),
      r.ownshipAirbrake(),
      r.ownshipSteer(),
      r.ownshipGroundSpeed(),
      r.ownshipVs(),
      r.ownshipAgl(),
      r.ownshipGLagged(),
      r.ownshipGStress(),
      r.ownshipRedout(),
      r.ownshipNCmd(),
      r.ownshipTouchdownVs(),
      r.ownshipCrashCause(),
      r.ownshipGearBits(),
    ]).toEqual([
      o.assist,
      o.fcs_mode,
      o.flags2,
      o.spawn_seq,
      o.flap_detent,
      o.gear_pos,
      o.tef,
      o.airbrake,
      o.steer,
      o.ground_speed,
      o.vs,
      o.agl,
      o.g_lagged,
      o.g_stress,
      o.redout,
      o.n_cmd,
      o.touchdown_vs,
      o.crash_cause,
      o.gear_bits,
    ]);
    // The vector is a roll on the runway under Standard assist with the pilot working the
    // pedals, the toe brakes, the flaps and the mouse, so that no field of the block is a zero a
    // misplaced reader would also read.
    expect(o.assist).toBe(2);
    expect(o.fcs_mode, 'the ground law').toBe(1);
    expect(o.flags2 & 1, 'weight on wheels').toBe(1);
    expect(o.flags2 & 0x4000, 'mouse-aim in use').not.toBe(0);
    expect(o.spawn_seq).toBe(1);
    expect(o.flap_detent).toBe(2);
    expect(o.gear_pos).toBe(1);
    expect(o.tef).toBeGreaterThan(0);
    expect(o.brake[0]).not.toBe(o.brake[1]);
    expect(Math.abs(o.steer)).toBeGreaterThan(0.1);
    expect(o.ground_speed).toBeGreaterThan(5);
    expect(o.g_lagged).toBeGreaterThan(0.5);
    expect(o.aim[0]).not.toBe(0);
    expect(o.aim[1]).not.toBe(0);
    expect(o.gear_bits & 0xc0, 'lever down, all down and locked').toBe(0xc0);
  });

  it('reads bodies (the vector lists the first three and the last)', () => {
    const r = new SnapshotReader(bytes());
    const indices = [0, 1, 2, d.body_count - 1];
    const pos = new Float64Array(3);
    const quat = new Float32Array(4);
    indices.forEach((body, k) => {
      r.bodyPos(body, pos);
      r.bodyQuat(body, quat);
      expect([...pos], `body ${String(body)} position`).toEqual(d.bodies[k]?.pos_bf);
      expect([...quat], `body ${String(body)} orientation`).toEqual(d.bodies[k]?.quat);
    });
  });

  it('writes into the caller-chosen offset', () => {
    const r = new SnapshotReader(bytes());
    const pos = new Float64Array(6);
    r.ownshipPos(pos, 3);
    expect([...pos.subarray(0, 3)]).toEqual([0, 0, 0]);
    expect([...pos.subarray(3)]).toEqual(d.ownship.pos_bf);
  });
});

describe('snapshot validity and stamping', () => {
  it('stamps the publish time in place without touching anything else', () => {
    const b = bytes();
    const before = b.slice();
    stampPublishTime(b, 1_700_000_123_456.5);
    expect(new SnapshotReader(b).publishTimeMs()).toBe(1_700_000_123_456.5);
    const changed = [...b].flatMap((x, i) => (x === before[i] ? [] : [i]));
    expect(
      changed.every((i) => i >= SNAPSHOT.H_PUBLISH_TIME_MS && i < SNAPSHOT.H_PUBLISH_TIME_MS + 8),
    ).toBe(true);
  });

  it('does not accept wrong magic or version', () => {
    const b = bytes();
    b[0] = (b[0] ?? 0) ^ 0xff;
    expect(new SnapshotReader(b).valid()).toBe(false);
    const c = bytes();
    c[SNAPSHOT.H_VERSION] = 1; // a v1 snapshot
    expect(new SnapshotReader(c).valid()).toBe(false);
    expect(new SnapshotReader(new Uint8Array(SNAPSHOT.SIZE)).valid()).toBe(false);
  });

  it('refuses a buffer too small to be a snapshot', () => {
    expect(() => new SnapshotReader(new Uint8Array(SNAPSHOT.SIZE - 1))).toThrow(RangeError);
  });

  it('reads a view that does not start at zero', () => {
    const backing = new Uint8Array(SNAPSHOT.SIZE + 24);
    backing.set(bytes(), 24);
    const r = new SnapshotReader(backing.subarray(24));
    expect(r.valid()).toBe(true);
    expect(r.tick()).toBe(vector.decoded.tick);
  });
});
