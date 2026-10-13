import { describe, expect, it } from 'vitest';
import { SNAPSHOT } from '@flying/sim-bridge/layout';
import { SnapshotInterpolator } from './interp.ts';
import { makeSnapshot, NORTH } from './snapshot-fixture.ts';

const DT = 1000 / 60;
const at = (tick: number, x: number, extra: Parameters<typeof makeSnapshot>[0] = {}): Uint8Array =>
  makeSnapshot({ tick, publishMs: 1000 + tick * DT, observer: { pos: [x, 0, 100] }, ...extra });

describe('SnapshotInterpolator', () => {
  it('has nothing to draw before the first snapshot', () => {
    const i = new SnapshotInterpolator();
    expect(i.sample(0)).toBeNull();
    expect(i.held).toBe(0);
  });

  it('draws the only snapshot as it is', () => {
    const i = new SnapshotInterpolator();
    expect(i.push(at(1, 10))).toBe(true);
    const s = i.sample(1000 + DT);
    expect(s?.observerPosBf[0]).toBe(10);
    expect(i.status.alpha).toBe(1);
    expect(i.held).toBe(1);
  });

  it('blends between the two newest by publish time, drawing one interval in the past', () => {
    const i = new SnapshotInterpolator();
    i.push(at(1, 0));
    i.push(at(2, 60));
    const t2 = 1000 + 2 * DT;
    // Drawing one interval behind "now": at now = publish(2) the picture is exactly tick 1.
    expect(i.sample(t2)?.observerPosBf[0]).toBeCloseTo(0, 9);
    expect(i.sample(t2 + DT / 2)?.observerPosBf[0]).toBeCloseTo(30, 9);
    expect(i.sample(t2 + DT)?.observerPosBf[0]).toBeCloseTo(60, 9);
    // Never extrapolates past the newest, never before the older one.
    expect(i.sample(t2 + 10 * DT)?.observerPosBf[0]).toBe(60);
    expect(i.sample(t2 - 10 * DT)?.observerPosBf[0]).toBe(0);
    expect(i.status.alpha).toBe(0);
  });

  it('moves smoothly: positions advance monotonically as time does', () => {
    const i = new SnapshotInterpolator();
    i.push(at(1, 0));
    i.push(at(2, 3));
    let last = -1;
    for (let k = 0; k <= 20; k++) {
      const x = i.sample(1000 + 2 * DT + (k / 20) * DT)?.observerPosBf[0] ?? NaN;
      expect(x).toBeGreaterThanOrEqual(last);
      last = x;
    }
    expect(last).toBeCloseTo(3, 9);
  });

  it('keeps only the two newest', () => {
    const i = new SnapshotInterpolator();
    for (let t = 1; t <= 5; t++) i.push(at(t, t * 10));
    expect(i.held).toBe(2);
    // Between ticks 4 and 5.
    expect(i.sample(1000 + 5 * DT + DT / 2)?.observerPosBf[0]).toBeCloseTo(45, 9);
  });

  it('ignores duplicates, invalid bytes and short buffers, and counts them', () => {
    const i = new SnapshotInterpolator();
    expect(i.push(at(1, 0))).toBe(true);
    expect(i.push(at(1, 99))).toBe(false); // same tick
    expect(i.push(new Uint8Array(SNAPSHOT.SIZE))).toBe(false); // no magic
    expect(i.push(new Uint8Array(10))).toBe(false);
    expect([i.accepted, i.ignored]).toEqual([1, 3]);
    expect(i.sample(2000)?.observerPosBf[0]).toBe(0);
  });

  it('starts over when the sim restarts (an older tick arrives)', () => {
    const i = new SnapshotInterpolator();
    i.push(at(500, 0));
    i.push(at(501, 5));
    expect(i.push(at(3, 77, { publishMs: 9000 }))).toBe(true);
    expect(i.held).toBe(1);
    expect(i.sample(9000)?.observerPosBf[0]).toBe(77);
  });

  it('reports a stale newest snapshot, and a gap of several ticks', () => {
    const i = new SnapshotInterpolator({ staleMs: 100 });
    i.push(at(1, 0));
    i.push(at(5, 40));
    i.sample(1000 + 5 * DT + 50);
    expect([i.status.stale, i.status.gapped]).toEqual([false, true]);
    i.sample(1000 + 5 * DT + 500);
    expect(i.status.stale).toBe(true);
  });

  it('does not sweep across the map when the observer teleports', () => {
    const i = new SnapshotInterpolator();
    i.push(at(1, 0));
    i.push(at(2, 6000)); // the reset button
    const s = i.sample(1000 + 2 * DT + DT / 2);
    expect(s?.observerPosBf[0]).toBe(6000);
    expect(i.status.snapped).toBe(true);
  });

  it('blends bodies, and snaps only the one that jumped', () => {
    const i = new SnapshotInterpolator();
    i.push(
      at(1, 0, { bodies: [{ pos: [0, 0, 0] }, { pos: [0, 0, 0] }, { pos: [100, 100, 100] }] }),
    );
    i.push(
      at(2, 0, { bodies: [{ pos: [10, 0, 0] }, { pos: [9000, 0, 0] }, { pos: [100, 100, 100] }] }),
    );
    const s = i.sample(1000 + 2 * DT + DT / 2);
    expect(s?.bodyCount).toBe(3);
    expect(s?.bodyPosBf[0]).toBeCloseTo(5, 9); // blended
    expect(s?.bodyPosBf[3]).toBe(9000); // jumped: snapped
    expect(s?.bodyPosBf[6]).toBe(100);
    expect(i.status.snapped).toBe(true);
  });

  it('takes bodies the older snapshot did not have straight from the newer', () => {
    const i = new SnapshotInterpolator();
    i.push(at(1, 0, { bodies: [{ pos: [0, 0, 0] }] }));
    i.push(at(2, 0, { bodies: [{ pos: [10, 0, 0] }, { pos: [50, 0, 0] }] }));
    const s = i.sample(1000 + 2 * DT + DT / 2);
    expect(s?.bodyCount).toBe(2);
    expect(s?.bodyPosBf[3]).toBe(50);
  });

  it('interpolates orientation along the short arc and keeps it unit length', () => {
    const half = Math.SQRT1_2;
    const i = new SnapshotInterpolator();
    // Identity, then a quarter turn about +x (w = x = sqrt(1/2)): halfway is an eighth turn.
    i.push(at(1, 0, { observer: { pos: [0, 0, 0], quat: [1, 0, 0, 0] } }));
    i.push(at(2, 0, { observer: { pos: [0, 0, 0], quat: [half, half, 0, 0] } }));
    const q = i.sample(1000 + 2 * DT + DT / 2)?.observerQuat;
    const [w, x] = [q?.[0] ?? 0, q?.[1] ?? 0];
    expect(Math.hypot(w, x, q?.[2] ?? 0, q?.[3] ?? 0)).toBeCloseTo(1, 6);
    expect(Math.atan2(x, w) * 2).toBeCloseTo(Math.PI / 4, 3);
  });

  it('takes the short way round when the two quaternions are on opposite hemispheres', () => {
    const i = new SnapshotInterpolator();
    const [w, x, y, z] = NORTH;
    i.push(at(1, 0, { observer: { pos: [0, 0, 0], quat: [w, x, y, z] } }));
    i.push(at(2, 0, { observer: { pos: [0, 0, 0], quat: [-w, -x, -y, -z] } })); // same rotation
    const q = i.sample(1000 + 2 * DT + DT / 2)?.observerQuat;
    // Same rotation, so halfway is that rotation again (up to sign), not a collapse to zero.
    expect(Math.abs(Math.hypot(...(q ?? [0]))) - 1).toBeCloseTo(0, 6);
    expect(Math.abs(q?.[1] ?? 0)).toBeCloseTo(Math.SQRT1_2, 5);
  });

  it('passes on the header facts of the newest snapshot', () => {
    const i = new SnapshotInterpolator();
    i.push(
      at(7, 0, {
        inputMs: 1234.5,
        coreState: 3,
        observer: { pos: [0, 0, 0], speed: 222, throttle: 0.9 },
      }),
    );
    const s = i.sample(2000);
    expect([s?.tick, s?.inputTimeMs, s?.coreState]).toEqual([7, 1234.5, 3]);
    expect(s?.publishTimeMs).toBeCloseTo(1000 + 7 * DT, 9);
    expect([s?.observerSpeed, s?.observerThrottle]).toEqual([222, Math.fround(0.9)]);
  });

  it('treats equal publish stamps (an unstamped snapshot) as no interpolation', () => {
    const i = new SnapshotInterpolator();
    i.push(makeSnapshot({ tick: 1, publishMs: 0, observer: { pos: [0, 0, 0] } }));
    i.push(makeSnapshot({ tick: 2, publishMs: 0, observer: { pos: [5, 0, 0] } }));
    expect(i.sample(100)?.observerPosBf[0]).toBe(5);
  });
});
