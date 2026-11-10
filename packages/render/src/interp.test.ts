import { describe, expect, it } from 'vitest';
import {
  BANDIT_LIFE,
  COMBAT_FLAG,
  EXPLOSION_KIND,
  MISSILE_STATE,
  SNAPSHOT,
} from '@flying/sim-bridge/layout';
import { SnapshotInterpolator } from './interp.ts';
import { makeSnapshot, NORTH, type FixtureCombat } from './snapshot-fixture.ts';

const DT = 1000 / 60;
const at = (tick: number, x: number, extra: Parameters<typeof makeSnapshot>[0] = {}): Uint8Array =>
  makeSnapshot({ tick, publishMs: 1000 + tick * DT, ownship: { pos: [x, 0, 100] }, ...extra });

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
    expect(s?.ownshipPosBf[0]).toBe(10);
    expect(i.status.alpha).toBe(1);
    expect(i.held).toBe(1);
  });

  it('blends between the two newest by publish time, drawing one interval in the past', () => {
    const i = new SnapshotInterpolator();
    i.push(at(1, 0));
    i.push(at(2, 60));
    const t2 = 1000 + 2 * DT;
    // Drawing one interval behind "now": at now = publish(2) the picture is exactly tick 1.
    expect(i.sample(t2)?.ownshipPosBf[0]).toBeCloseTo(0, 9);
    expect(i.sample(t2 + DT / 2)?.ownshipPosBf[0]).toBeCloseTo(30, 9);
    expect(i.sample(t2 + DT)?.ownshipPosBf[0]).toBeCloseTo(60, 9);
    // Never extrapolates past the newest, never before the older one.
    expect(i.sample(t2 + 10 * DT)?.ownshipPosBf[0]).toBe(60);
    expect(i.sample(t2 - 10 * DT)?.ownshipPosBf[0]).toBe(0);
    expect(i.status.alpha).toBe(0);
  });

  it('moves smoothly: positions advance monotonically as time does', () => {
    const i = new SnapshotInterpolator();
    i.push(at(1, 0));
    i.push(at(2, 3));
    let last = -1;
    for (let k = 0; k <= 20; k++) {
      const x = i.sample(1000 + 2 * DT + (k / 20) * DT)?.ownshipPosBf[0] ?? NaN;
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
    expect(i.sample(1000 + 5 * DT + DT / 2)?.ownshipPosBf[0]).toBeCloseTo(45, 9);
  });

  it('ignores duplicates, invalid bytes and short buffers, and counts them', () => {
    const i = new SnapshotInterpolator();
    expect(i.push(at(1, 0))).toBe(true);
    expect(i.push(at(1, 99))).toBe(false); // same tick
    expect(i.push(new Uint8Array(SNAPSHOT.SIZE))).toBe(false); // no magic
    expect(i.push(new Uint8Array(10))).toBe(false);
    expect([i.accepted, i.ignored]).toEqual([1, 3]);
    expect(i.sample(2000)?.ownshipPosBf[0]).toBe(0);
  });

  it('starts over when the sim restarts (an older tick arrives)', () => {
    const i = new SnapshotInterpolator();
    i.push(at(500, 0));
    i.push(at(501, 5));
    expect(i.push(at(3, 77, { publishMs: 9000 }))).toBe(true);
    expect(i.held).toBe(1);
    expect(i.sample(9000)?.ownshipPosBf[0]).toBe(77);
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

  it('does not sweep across the map when the ownship teleports (a respawn)', () => {
    const i = new SnapshotInterpolator();
    i.push(at(1, 0));
    i.push(at(2, 6000)); // the reset button
    const s = i.sample(1000 + 2 * DT + DT / 2);
    expect(s?.ownshipPosBf[0]).toBe(6000);
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
    i.push(at(1, 0, { ownship: { pos: [0, 0, 0], quat: [1, 0, 0, 0] } }));
    i.push(at(2, 0, { ownship: { pos: [0, 0, 0], quat: [half, half, 0, 0] } }));
    const q = i.sample(1000 + 2 * DT + DT / 2)?.ownshipQuat;
    const [w, x] = [q?.[0] ?? 0, q?.[1] ?? 0];
    expect(Math.hypot(w, x, q?.[2] ?? 0, q?.[3] ?? 0)).toBeCloseTo(1, 6);
    expect(Math.atan2(x, w) * 2).toBeCloseTo(Math.PI / 4, 3);
  });

  it('takes the short way round when the two quaternions are on opposite hemispheres', () => {
    const i = new SnapshotInterpolator();
    const [w, x, y, z] = NORTH;
    i.push(at(1, 0, { ownship: { pos: [0, 0, 0], quat: [w, x, y, z] } }));
    i.push(at(2, 0, { ownship: { pos: [0, 0, 0], quat: [-w, -x, -y, -z] } })); // same rotation
    const q = i.sample(1000 + 2 * DT + DT / 2)?.ownshipQuat;
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
        ownship: { pos: [0, 0, 0], speed: 222, throttle: 0.9 },
      }),
    );
    const s = i.sample(2000);
    expect([s?.tick, s?.inputTimeMs, s?.coreState]).toEqual([7, 1234.5, 3]);
    expect(s?.publishTimeMs).toBeCloseTo(1000 + 7 * DT, 9);
    expect([s?.ownshipSpeed, s?.ownshipThrottle]).toEqual([222, Math.fround(0.9)]);
  });

  it('treats equal publish stamps (an unstamped snapshot) as no interpolation', () => {
    const i = new SnapshotInterpolator();
    i.push(makeSnapshot({ tick: 1, publishMs: 0, ownship: { pos: [0, 0, 0] } }));
    i.push(makeSnapshot({ tick: 2, publishMs: 0, ownship: { pos: [5, 0, 0] } }));
    expect(i.sample(100)?.ownshipPosBf[0]).toBe(5);
  });

  it("passes on the pilot's state and the mouse-aim direction of the newest snapshot, unblended", () => {
    const i = new SnapshotInterpolator();
    i.push(
      at(1, 0, {
        ownship: { pos: [0, 0, 0], flags2: 0, gStress: 0.1, redout: 0, aim: [0, 0] },
      }),
    );
    i.push(
      at(2, 0, {
        ownship: {
          pos: [0, 0, 0],
          flags2: 16384,
          gStress: 0.8,
          redout: 0.6,
          aim: [1.25, -0.5],
        },
      }),
    );
    // Halfway between the two: what the pilot feels is not interpolated, it is the newest.
    const s = i.sample(1000 + 2 * DT + DT / 2);
    expect(s?.ownshipFlags2).toBe(16384);
    expect(s?.gStress).toBeCloseTo(0.8, 6);
    expect(s?.redout).toBeCloseTo(0.6, 6);
    expect([s?.aimAz, s?.aimEl].map((x) => Math.round((x ?? 0) * 1e6) / 1e6)).toEqual([1.25, -0.5]);
  });

  describe('the duel', () => {
    const duel = (tick: number, combat: FixtureCombat): Uint8Array =>
      makeSnapshot({ tick, publishMs: 1000 + tick * DT, ownship: { pos: [0, 0, 100] }, combat });
    /** Halfway between ticks 1 and 2. */
    const HALF = 1000 + 2 * DT + DT / 2;
    const got = (i: SnapshotInterpolator, t = HALF) => {
      const s = i.sample(t);
      if (!s) throw new Error('no state');
      return s;
    };
    const pair = (a: FixtureCombat, b: FixtureCombat): SnapshotInterpolator => {
      const i = new SnapshotInterpolator();
      i.push(duel(1, a));
      i.push(duel(2, b));
      return i;
    };
    const xyz = (v: ArrayLike<number>, at = 0): number[] =>
      [0, 1, 2].map((k) => Math.round((v[at + k] ?? NaN) * 1e6) / 1e6);
    const bandit = (x: number, extra: Partial<NonNullable<FixtureCombat['bandit']>> = {}) => ({
      pos: [x, 1000, 500] as [number, number, number],
      quat: NORTH,
      ...extra,
    });

    it('blends the pipper while both snapshots have one, and otherwise takes the newest', () => {
      const flags = COMBAT_FLAG.ACTIVE | COMBAT_FLAG.PIPPER;
      const both = pair({ flags, pipper: [0, 0, 0] }, { flags, pipper: [10, 20, 30] });
      expect(xyz(got(both).pipperBf)).toEqual([5, 10, 15]);
      const fresh = pair(
        { flags: COMBAT_FLAG.ACTIVE, pipper: [0, 0, 0] },
        { flags, pipper: [10, 20, 30] },
      );
      expect(xyz(got(fresh).pipperBf)).toEqual([10, 20, 30]);
    });

    it('passes on the weapon state of the newest snapshot, unblended', () => {
      const i = pair(
        { flags: COMBAT_FLAG.ACTIVE, rounds: 500, missiles: 2, tgtRange: 1000 },
        {
          flags: COMBAT_FLAG.ACTIVE | COMBAT_FLAG.ARMED,
          seeker: 2,
          inhibit: 3,
          inhibitAge: 0.25,
          rounds: 480,
          missiles: 1,
          tgtRange: 900,
          tgtClosure: 50,
        },
      );
      const s = got(i);
      expect(s.combatFlags).toBe(COMBAT_FLAG.ACTIVE | COMBAT_FLAG.ARMED);
      expect([s.seeker, s.inhibit, s.rounds, s.missilesLeft]).toEqual([2, 3, 480, 1]);
      expect(s.inhibitAge).toBeCloseTo(0.25, 6);
      expect(s.tgtRange).toBeCloseTo(900, 3);
      expect(s.tgtClosure).toBeCloseTo(50, 3);
    });

    it('blends the bandit between two snapshots of the same life', () => {
      const i = pair({ bandit: bandit(0) }, { bandit: bandit(100) });
      const s = got(i);
      expect(s.banditLife).toBe(BANDIT_LIFE.ALIVE);
      expect(xyz(s.banditPosBf)).toEqual([50, 1000, 500]);
      expect(Math.hypot(...s.banditQuat)).toBeCloseTo(1, 6);
      expect(i.status.snapped).toBe(false);
    });

    it('snaps the bandit when it respawned (a new spawn number), even a short way off', () => {
      const i = pair(
        { bandit: bandit(0, { spawnSeq: 1 }) },
        { bandit: bandit(100, { spawnSeq: 2 }) },
      );
      const s = got(i);
      expect(xyz(s.banditPosBf)).toEqual([100, 1000, 500]);
      expect(s.banditSpawnSeq).toBe(2);
    });

    it('snaps the bandit when it jumped further than the teleport distance, and says so', () => {
      const i = pair({ bandit: bandit(0) }, { bandit: bandit(5000) });
      expect(xyz(got(i).banditPosBf)).toEqual([5000, 1000, 500]);
      expect(i.status.snapped).toBe(true);
    });

    it('takes a bandit that has just appeared straight from the newest, and none when absent', () => {
      const appeared = pair({}, { bandit: bandit(300) });
      expect(xyz(got(appeared).banditPosBf)).toEqual([300, 1000, 500]);
      const gone = pair({ bandit: bandit(0) }, {});
      expect(got(gone).banditLife).toBe(BANDIT_LIFE.ABSENT);
    });

    it('packs the missiles in flight, blending a slot that was in flight in both', () => {
      const slot = (x: number, state: number) => ({
        pos: [x, 0, 200] as [number, number, number],
        vel: [600, 0, 0] as [number, number, number],
        state,
      });
      const i = pair(
        { missileSlots: [slot(0, MISSILE_STATE.FREE), slot(0, MISSILE_STATE.BOOST)] },
        {
          missileSlots: [
            slot(70, MISSILE_STATE.BOOST),
            slot(100, MISSILE_STATE.COAST),
            slot(0, MISSILE_STATE.FREE),
          ],
        },
      );
      const s = got(i);
      expect(s.missileCount).toBe(2);
      // Slot 0 has just launched: the newest. Slot 1 flew in both: halfway.
      expect(xyz(s.missilePosBf, 0)).toEqual([70, 0, 200]);
      expect(xyz(s.missilePosBf, 3)).toEqual([50, 0, 200]);
      expect([s.missileState[0], s.missileState[1]]).toEqual([
        MISSILE_STATE.BOOST,
        MISSILE_STATE.COAST,
      ]);
      expect(xyz(s.missileVelBf, 3)).toEqual([600, 0, 0]);
    });

    it('blends a tracer with the same round of the older snapshot, wherever it is in the list', () => {
      const round = (x: number, id: number) => ({
        pos: [x, 0, 300] as [number, number, number],
        vel: [1000, 0, 0] as [number, number, number],
        id,
      });
      const i = pair(
        { tracers: [round(0, 7), round(500, 9)] },
        // Round 7 was culled; 9 moved to the front; 11 is new.
        { tracers: [round(600, 9), round(40, 11)] },
      );
      const s = got(i);
      expect(s.tracerCount).toBe(2);
      expect(xyz(s.tracerPosBf, 0)).toEqual([550, 0, 300]);
      expect(xyz(s.tracerPosBf, 3)).toEqual([40, 0, 300]);
      // The id table is cleared after each sample. Round 7 comes back (the id wrapped) while the
      // older snapshot no longer has it: it is drawn where it is, not blended with a stale index.
      i.push(duel(3, { tracers: [round(700, 7)] }));
      expect(xyz(got(i, HALF + DT).tracerPosBf, 0)).toEqual([700, 0, 300]);
    });

    it('takes the explosions from the newest snapshot, skipping empty slots', () => {
      const i = pair(
        { explosions: [{ pos: [1, 2, 3], age: 0.1, kind: EXPLOSION_KIND.MISSILE }] },
        {
          explosions: [
            { pos: [0, 0, 0], age: 0, kind: 0 },
            { pos: [10, 20, 30], age: 0.4, kind: EXPLOSION_KIND.AIRCRAFT },
          ],
        },
      );
      const s = got(i);
      expect(s.explosionCount).toBe(1);
      expect(xyz(s.explosionPosBf)).toEqual([10, 20, 30]);
      expect(s.explosionAge[0]).toBeCloseTo(0.4, 6);
      expect(s.explosionKind[0]).toBe(EXPLOSION_KIND.AIRCRAFT);
    });
  });
});
