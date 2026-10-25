import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { newAircraftStatus, readAircraftStatus } from './aircraft-status.ts';
import { SNAPSHOT } from './layout.gen.ts';
import { SnapshotReader } from './snapshot.ts';

const vector = JSON.parse(
  readFileSync(new URL('../../../tests/vectors/snapshot_v3.json', import.meta.url), 'utf8'),
) as {
  hex: string;
  decoded: {
    ownship: Record<string, number | number[]> & { quat: number[] };
  };
};
const bytes = (): Uint8Array =>
  Uint8Array.from(vector.hex.match(/../g) ?? [], (h) => Number.parseInt(h, 16));

describe('readAircraftStatus', () => {
  const o = vector.decoded.ownship;
  const f = Math.fround;

  it('copies the ownship fields the cockpit side needs, as the reference snapshot has them', () => {
    const s = newAircraftStatus();
    readAircraftStatus(new SnapshotReader(bytes()), s);
    expect(s.spawnSeq).toBe(o['spawn_seq']);
    expect(s.assist).toBe(o['assist']);
    expect(s.fcsMode).toBe(o['fcs_mode']);
    expect(s.flags).toBe(o['flags']);
    expect(s.flags2).toBe(o['flags2']);
    expect(s.throttle).toBe(f(o['throttle'] as number));
    expect(s.tas).toBe(f(o['tas'] as number));
    expect(s.cas).toBe(f(o['cas'] as number));
    expect(s.mach).toBe(f(o['mach'] as number));
    expect(s.alpha).toBe(f(o['alpha'] as number));
    expect(s.nz).toBe(f(o['nz'] as number));
    expect(s.gearPos).toBe(f(o['gear_pos'] as number));
    expect(s.flapDetent).toBe(o['flap_detent']);
    expect(s.airbrake).toBe(f(o['airbrake'] as number));
    expect([s.brakeL, s.brakeR]).toEqual((o['brake'] as number[]).map(f));
    expect(s.groundSpeed).toBe(f(o['ground_speed'] as number));
    expect(s.vs).toBe(f(o['vs'] as number));
    expect(s.agl).toBe(f(o['agl'] as number));
    expect(s.gLagged).toBe(f(o['g_lagged'] as number));
    expect(s.gStress).toBe(f(o['g_stress'] as number));
    expect(s.redout).toBe(f(o['redout'] as number));
    expect(s.nCmd).toBe(f(o['n_cmd'] as number));
    expect(s.gearBits).toBe(o['gear_bits']);
    expect(s.crashCause).toBe(o['crash_cause']);
  });

  it('works out where the nose points from the attitude', () => {
    const s = newAircraftStatus();
    readAircraftStatus(new SnapshotReader(bytes()), s);
    const [w = 1, x = 0, y = 0, z = 0] = o.quat.map(f);
    const east = 1 - 2 * (y * y + z * z);
    const north = 2 * (x * y + w * z);
    const up = 2 * (x * z - w * y);
    expect(s.noseAz).toBeCloseTo(Math.atan2(east, north), 12);
    expect(s.noseEl).toBeCloseTo(Math.asin(up), 12);
  });

  it('points north and level for the identity attitude, east for a quarter turn clockwise', () => {
    const base = bytes();
    const reader = new SnapshotReader(base);
    const dv = new DataView(base.buffer, base.byteOffset, base.byteLength);
    // The identity attitude carries the body x axis to east; a quarter turn about up takes it to
    // north.
    const qOff = SNAPSHOT.O_QUAT;
    const set = (q: number[]): void => {
      q.forEach((v, i) => {
        dv.setFloat32(qOff + 4 * i, v, true);
      });
    };
    const s = newAircraftStatus();
    const h = Math.SQRT1_2;
    set([h, 0, 0, h]); // body x -> north (the overlay's heading 0)
    readAircraftStatus(reader, s);
    expect(s.noseAz).toBeCloseTo(0, 6);
    expect(s.noseEl).toBeCloseTo(0, 6);
    set([1, 0, 0, 0]); // identity: body x -> east
    readAircraftStatus(reader, s);
    expect(s.noseAz).toBeCloseTo(Math.PI / 2, 6);
  });

  it('reuses the object it is given', () => {
    const s = newAircraftStatus();
    const reader = new SnapshotReader(bytes());
    readAircraftStatus(reader, s);
    const again = { ...s };
    readAircraftStatus(reader, s);
    expect(s).toEqual(again);
  });
});
