/**
 * The mission recorder: ST1's twelve recorder cases (kills, no credit and deaths from the counters),
 * ST3's ten flights (sortie start and end, successful landing, the records) and its garbage (ADR-45).
 */
import { describe, expect, it } from 'vitest';
import {
  COMBAT_ACTOR,
  COMBAT_COUNTER,
  COMBAT_EVENT,
  newAircraftStatus,
  newCombatStatus,
  OWNSHIP_FLAG,
  OWNSHIP_FLAG2,
  SCENARIO,
  type AircraftStatus,
  type CombatEventView,
  type CombatStatus,
} from '@flying/sim-bridge';
import { bytesPerCall } from '../test-support.ts';
import { MissionRecorder, missionId, type RecorderSession } from './recorder.ts';

const DUEL: RecorderSession = { scenario: SCENARIO.DUEL, skill: 2, assist: 2, arena: 0 };

function recorder(session: RecorderSession = DUEL): MissionRecorder {
  const r = new MissionRecorder();
  r.begin(session, 'build-1', { wallMs: Date.UTC(2026, 10, 10, 9, 12, 3), id: 'm-test' });
  return r;
}

// ---- the duel's counters (ST1) -------------------------------------------------------------

type Counts = Partial<Record<keyof typeof COMBAT_COUNTER, number>>;
function combat(counts: Counts): CombatStatus {
  const c = newCombatStatus();
  for (const [k, v] of Object.entries(counts)) {
    c.counters[COMBAT_COUNTER[k as keyof typeof COMBAT_COUNTER]] = v;
  }
  return c;
}

/** Feeds `samples` of counters, 100 ms apart, and finishes. */
function countsOf(samples: (Counts | undefined)[]) {
  const r = recorder();
  samples.forEach((s, i) => {
    r.update(undefined, s === undefined ? undefined : combat(s), i * 100);
  });
  return r.finish().combat;
}

describe('ST1: kills, no credit and deaths from the counters', () => {
  it('1: a gun kill', () => {
    expect(countsOf([{}, { KILLS: 1, GUN_KILLS: 1 }])).toMatchObject({ kills: 1, gun_kills: 1 });
  });

  it('2: a missile kill', () => {
    const c = countsOf([
      {},
      { MISSILES_FIRED: 1 },
      { MISSILES_FIRED: 1, MISSILE_HITS: 1, KILLS: 1, MISSILE_KILLS: 1 },
    ]);
    expect(c).toMatchObject({ kills: 1, missile_kills: 1, missiles_fired: 1, missile_hits: 1 });
  });

  it('3: a crash after damage: a kill by neither weapon', () => {
    expect(countsOf([{}, { ROUND_HITS: 3 }, { ROUND_HITS: 3, KILLS: 1 }])).toMatchObject({
      kills: 1,
      gun_kills: 0,
      missile_kills: 0,
      gun_hits: 3,
    });
  });

  it('4: a bandit lost with no credit (it crashed undamaged): nothing', () => {
    expect(countsOf([{}, { BANDIT_SPAWNS: 1, EXPLOSIONS: 1 }])).toMatchObject({ kills: 0 });
  });

  it('5: the player shot down: a death', () => {
    expect(countsOf([{}, { HITS_TAKEN: 4, DEATHS: 1 }])).toMatchObject({
      deaths: 1,
      hits_taken: 4,
    });
  });

  it('6: a kill, then a death', () => {
    expect(
      countsOf([{}, { KILLS: 1, GUN_KILLS: 1 }, { KILLS: 1, GUN_KILLS: 1, DEATHS: 1 }]),
    ).toMatchObject({
      kills: 1,
      deaths: 1,
    });
  });

  it('7: two kills between two samples (a missed frame) are both counted', () => {
    expect(countsOf([{}, { KILLS: 2, GUN_KILLS: 1, MISSILE_KILLS: 1 }])).toMatchObject({
      kills: 2,
      gun_kills: 1,
      missile_kills: 1,
    });
  });

  it('8: samples with no duel status in between lose nothing', () => {
    expect(countsOf([{}, undefined, undefined, { ROUNDS_FIRED: 150 }])).toMatchObject({
      gun_rounds: 150,
    });
  });

  it('9: what the counters said before the first sample (a stale status) is not counted', () => {
    expect(
      countsOf([
        { KILLS: 5, ROUNDS_FIRED: 900 },
        { KILLS: 5, ROUNDS_FIRED: 950 },
      ]),
    ).toMatchObject({
      kills: 0,
      gun_rounds: 50,
    });
  });

  it('10: a counter that falls (a new session) counts again from zero', () => {
    expect(
      countsOf([{}, { KILLS: 3, GUN_KILLS: 3 }, {}, { KILLS: 1, GUN_KILLS: 1 }]),
    ).toMatchObject({
      kills: 4,
      gun_kills: 4,
    });
  });

  it('11: the gun and missile accuracy numbers', () => {
    const c = countsOf([
      {},
      { ROUNDS_FIRED: 40, ROUND_HITS: 6 },
      { ROUNDS_FIRED: 70, ROUND_HITS: 9, MISSILES_FIRED: 2, MISSILE_HITS: 1 },
    ]);
    expect(c).toMatchObject({ gun_rounds: 70, gun_hits: 9, missiles_fired: 2, missile_hits: 1 });
  });

  it('12: free flight records no bandit and the mode', () => {
    const r = recorder({ scenario: SCENARIO.FREE, skill: 3, assist: 1, arena: 1 });
    r.update(undefined, combat({}), 0);
    const result = r.finish();
    expect([result.mode, result.skill, result.assist, result.arena]).toEqual(['free', 0, 1, 1]);
    expect(Object.values(result.combat).every((v) => v === 0)).toBe(true);
  });
});

// ---- the flights (ST3) ---------------------------------------------------------------------

interface Sample {
  wow?: boolean;
  gs?: number;
  alt?: number;
  tas?: number;
  mach?: number;
  crashed?: boolean;
  spawn?: number;
}

function status(s: Sample): AircraftStatus {
  return {
    ...newAircraftStatus(),
    spawnSeq: s.spawn ?? 1,
    flags: s.crashed === true ? OWNSHIP_FLAG.CRASHED : 0,
    flags2: s.wow === false ? 0 : OWNSHIP_FLAG2.WOW,
    groundSpeed: s.gs ?? 0,
    altitude: s.alt ?? 40,
    tas: s.tas ?? s.gs ?? 0,
    mach: s.mach ?? 0,
  };
}

/** A flight script: holds each sample for `s` seconds, sampled every 100 ms, from `t0` ms. */
class Flight {
  t = 0;
  readonly r: MissionRecorder;
  constructor(r: MissionRecorder) {
    this.r = r;
  }
  hold(seconds: number, sample: Sample, kills = 0): this {
    const c = combat({ KILLS: kills, GUN_KILLS: kills });
    const end = this.t + seconds * 1000;
    for (; this.t < end; this.t += 100) this.r.update(status(sample), c, this.t);
    return this;
  }
}

const roll = (gs: number): Sample => ({ wow: true, gs });
const air = (alt = 1000, tas = 200, mach = 0.6): Sample => ({
  wow: false,
  gs: tas,
  alt,
  tas,
  mach,
});

describe('ST3: sorties, landings and records (GDD 65.2)', () => {
  it('1: a take-off is a sortie once the wheels have been off for 10 s, dated from lift-off', () => {
    const f = new Flight(recorder()).hold(20, roll(30)).hold(9.5, air());
    expect(f.r.finish().sorties).toHaveLength(0);
    const g = new Flight(recorder()).hold(20, roll(30)).hold(30, air());
    const s = g.r.finish().sorties;
    expect(s).toHaveLength(1);
    // The interval in which the wheels left counts: 30 s off the ground.
    expect(s[0]?.airborne_s).toBeCloseTo(30, 1);
  });

  it('2: a hop shorter than 10 s is no sortie', () => {
    const f = new Flight(recorder()).hold(10, roll(60)).hold(6, air(60)).hold(20, roll(3));
    const result = f.r.finish();
    expect(result.sorties).toHaveLength(0);
    expect(f.r.sortieEnded()).toBe(false);
  });

  it('3: a full-stop landing ends the sortie 5 s after it is below 5 m/s, landed', () => {
    const f = new Flight(recorder())
      .hold(10, roll(40))
      .hold(60, air())
      .hold(10, roll(40))
      .hold(4.8, roll(3));
    expect(f.r.sortieEnded()).toBe(false);
    f.hold(0.5, roll(3));
    expect(f.r.sortieEnded()).toBe(true);
    const result = f.r.finish();
    expect(result.sorties).toEqual([expect.objectContaining({ landed: true })]);
    expect(result.outcome).toBe('landed');
  });

  it('4: a touch-and-go is not a landing, and the sortie goes on to the final one', () => {
    const f = new Flight(recorder())
      .hold(10, roll(40))
      .hold(60, air())
      .hold(3, roll(60))
      .hold(60, air())
      .hold(10, roll(40))
      .hold(8, roll(0));
    const result = f.r.finish();
    expect(result.sorties).toHaveLength(1);
    expect(result.sorties[0]?.landed).toBe(true);
    expect(result.sorties[0]?.airborne_s).toBeCloseTo(120, 0);
  });

  it('5: a crash ends the sortie unlanded, and the mission ends in death', () => {
    const f = new Flight(recorder())
      .hold(10, roll(40))
      .hold(60, air())
      .hold(1, { ...air(), crashed: true });
    const result = f.r.finish();
    expect(result.sorties).toEqual([expect.objectContaining({ landed: false })]);
    expect(result.outcome).toBe('died');
  });

  it('6: an air start is a sortie from its first moment', () => {
    const f = new Flight(recorder()).hold(5, air(3000));
    const result = f.r.finish();
    expect(result.sorties).toHaveLength(1);
    expect(result.sorties[0]?.airborne_s).toBeCloseTo(4.9, 1);
    expect(result.outcome).toBe('ended');
  });

  it('7: two sorties, each with its own time in the air (the longest is a statistic)', () => {
    const f = new Flight(recorder())
      .hold(10, roll(40))
      .hold(30, air())
      .hold(10, roll(40))
      .hold(6, roll(0))
      .hold(10, roll(40))
      .hold(90, air())
      .hold(10, roll(40))
      .hold(6, roll(0));
    const s = f.r.finish().sorties;
    expect(s.map((x) => Math.round(x.airborne_s))).toEqual([30, 90]);
  });

  it('8: the highest altitude, the fastest true airspeed and Mach, while alive', () => {
    const f = new Flight(recorder())
      .hold(5, air(8000, 250, 0.82))
      .hold(5, air(10_400, 395, 1.32))
      .hold(5, air(2000, 180, 0.5))
      // Destroyed: a wreck falling fast is no record.
      .hold(1, { ...air(1000, 600, 1.9), crashed: true });
    expect(f.r.finish().sorties[0]).toMatchObject({
      max_alt_m: 10_400,
      max_tas_mps: 395,
      max_mach: 1.32,
    });
  });

  it('9: the mission ending in the air ends the sortie with it', () => {
    const f = new Flight(recorder()).hold(10, roll(40)).hold(30, air());
    const result = f.r.finish();
    expect(result.sorties).toEqual([expect.objectContaining({ landed: false })]);
    expect(result.outcome).toBe('ended');
    expect(result.duration_s).toBeCloseTo(39.9, 1);
  });

  it('10: a respawn in the air after a loss is a new sortie', () => {
    const f = new Flight(recorder())
      .hold(30, air())
      .hold(2, { ...air(), crashed: true })
      .hold(40, { ...air(), spawn: 2 });
    const result = f.r.finish();
    expect(result.sorties).toHaveLength(2);
    expect(result.sorties.map((s) => Math.round(s.airborne_s))).toEqual([30, 40]);
    expect(result.outcome).toBe('ended');
  });
});

describe('the task and the clock', () => {
  it('times the task from Fly to the first stop after a kill', () => {
    const f = new Flight(recorder())
      .hold(10, roll(40))
      .hold(100, air(), 0)
      .hold(50, air(), 1)
      .hold(10, roll(30), 1)
      .hold(3, roll(1), 1);
    const result = f.r.finish();
    expect(result.task_s).toBeCloseTo(170, 0);
    expect(result.combat.kills).toBe(1);
  });

  it('has no task without a kill, or with a stop before the kill', () => {
    const none = new Flight(recorder()).hold(10, roll(40)).hold(60, air()).hold(10, roll(1));
    expect(none.r.finish().task_s).toBeNull();
    const early = new Flight(recorder())
      .hold(10, roll(40))
      .hold(60, air())
      .hold(10, roll(1))
      .hold(5, roll(0), 1);
    expect(early.r.finish().task_s).toBeNull();
  });

  it("takes the first kill's moment from the event, before the counters show it", () => {
    const kill: CombatEventView = {
      seq: 1,
      tick: 60,
      kind: COMBAT_EVENT.KILL,
      actor: COMBAT_ACTOR.PLAYER,
      target: COMBAT_ACTOR.BANDIT,
      arg: 1,
      value: 400,
    };
    const run = (withEvent: boolean): number | null => {
      const r = recorder();
      r.update(status(air()), combat({}), 0);
      r.update(status(roll(30)), combat({}), 1000);
      if (withEvent) r.onEvent(kill);
      // Stopped before a sample with the kill's counters arrives.
      r.update(status(roll(1)), combat({}), 1100);
      return r.finish().task_s;
    };
    expect(run(true)).toBeCloseTo(1.1, 6);
    expect(run(false)).toBeNull();
    // A bandit's kill (of the player) is not the player's.
    const r = recorder();
    r.update(status(air()), combat({}), 0);
    r.update(status(roll(30)), combat({}), 1000);
    r.onEvent({ ...kill, actor: COMBAT_ACTOR.BANDIT, target: COMBAT_ACTOR.PLAYER });
    r.update(status(roll(1)), combat({}), 1100);
    expect(r.finish().task_s).toBeNull();
  });

  it('does not count time the simulation was held', () => {
    const r = recorder();
    r.update(status(air()), undefined, 1000);
    for (let i = 0; i < 50; i++) r.update(status(air()), undefined, 1000);
    r.update(status(air()), undefined, 3000);
    const result = r.finish();
    expect(result.duration_s).toBe(2);
    expect(result.sorties[0]?.airborne_s).toBe(2);
  });

  it('starts again when the clock runs backwards (a fresh core after a stale sample)', () => {
    const r = recorder();
    r.update(status({ ...air(9000, 500, 1.5) }), combat({ KILLS: 7 }), 500_000);
    r.update(status(roll(0)), combat({}), 0);
    r.update(status(roll(0)), combat({ ROUNDS_FIRED: 10 }), 1000);
    const result = r.finish();
    expect(result.sorties).toHaveLength(0);
    expect(result.combat).toMatchObject({ kills: 0, gun_rounds: 10 });
    expect(result.duration_s).toBe(1);
  });

  it('writes the header: schema, id, build, date, mode and skill', () => {
    const r = recorder();
    r.update(status(roll(0)), combat({}), 0);
    expect(r.active).toBe(true);
    const result = r.finish();
    expect(r.active).toBe(false);
    expect(result).toMatchObject({
      schema: 'mission_result/1',
      mission_id: 'm-test',
      build: 'build-1',
      date_iso: '2026-11-10T09:12:03.000Z',
      mode: 'duel',
      skill: 2,
    });
  });

  it('makes distinct ids of the date and time and a salt', () => {
    expect(missionId(Date.UTC(2026, 10, 10, 9, 12, 3), 0xab)).toBe('m-20261110-091203-00ab');
    const a = new MissionRecorder();
    const b = new MissionRecorder();
    a.begin(DUEL, 'x', { wallMs: 0 });
    b.begin(DUEL, 'x', { wallMs: 0 });
    expect(a.missionId).not.toBe(b.missionId);
  });
});

describe('the recorder: garbage', () => {
  it('updates without making garbage, flying and at rest', async () => {
    const r = recorder();
    const flying = status(air());
    const parked = status(roll(0));
    const c = combat({ ROUNDS_FIRED: 10 });
    let t = 0;
    const bytes = await bytesPerCall((i) => {
      t += 16;
      c.counters[COMBAT_COUNTER.ROUNDS_FIRED] = 10 + (i & 7);
      r.update(i % 2 === 0 ? flying : parked, c, t);
    });
    expect(bytes).toBeLessThan(1);
  }, 60_000);
});
