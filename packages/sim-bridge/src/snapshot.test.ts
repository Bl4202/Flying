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
  observer: {
    pos_bf: number[];
    quat: number[];
    vel_bf: number[];
    speed: number;
    throttle: number;
  };
  bodies: { pos_bf: number[]; quat: number[] }[];
}
const vector = JSON.parse(
  readFileSync(new URL('../../../tests/vectors/snapshot_v1.json', import.meta.url), 'utf8'),
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

  it('reads the observer', () => {
    const r = new SnapshotReader(bytes());
    const pos = new Float64Array(3);
    const quat = new Float32Array(4);
    const vel = new Float32Array(3);
    r.observerPos(pos);
    r.observerQuat(quat);
    r.observerVel(vel);
    expect([...pos]).toEqual(d.observer.pos_bf);
    expect([...quat]).toEqual(d.observer.quat);
    expect([...vel]).toEqual(d.observer.vel_bf);
    expect(r.observerSpeed()).toBe(d.observer.speed);
    expect(r.observerThrottle()).toBe(d.observer.throttle);
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
    r.observerPos(pos, 3);
    expect([...pos.subarray(0, 3)]).toEqual([0, 0, 0]);
    expect([...pos.subarray(3)]).toEqual(d.observer.pos_bf);
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
    c[SNAPSHOT.H_VERSION] = 2;
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
