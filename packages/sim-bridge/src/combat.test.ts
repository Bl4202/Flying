import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { EventCursor, type CombatEventView } from './combat-events.ts';
import {
  COMBAT_COUNTER,
  COMBAT_COUNTER_COUNT,
  newCombatStatus,
  readCombatStatus,
} from './combat-status.ts';
import { COMBAT_EVENT, SNAPSHOT } from './layout.gen.ts';
import { SnapshotReader } from './snapshot.ts';

type V3 = [number, number, number];
interface Duel {
  flags: number;
  seeker: number;
  inhibit: number;
  inhibit_age: number;
  rounds: number;
  missiles: number;
  bandits: number;
  event_seq: number;
  pipper_bf: V3;
  signal: number;
  tgt_range: number;
  tgt_closure: number;
  tgt_aspect: number;
  health: number;
  counters: number[];
  bandit: {
    pos_bf: V3;
    quat: number[];
    vel_bf: V3;
    surfaces: V3;
    tef: number;
    airbrake: number;
    gear_pos: number;
    ab_level: number;
    health: number;
    tas: number;
    life: number;
    ai: number;
    skill: number;
    flags: number;
    spawn_seq: number;
  };
  missile_slots: { pos_bf: V3; vel_bf: V3; state: number; owner: number }[];
  tracer_count: number;
  tracers: { pos_bf: V3; vel_bf: V3; owner: number; id: number }[];
  explosions: { pos_bf: V3; age: number; kind: number }[];
  events: Omit<CombatEventView, never>[];
}

const vector = JSON.parse(
  readFileSync(new URL('../../../tests/vectors/snapshot_v5.json', import.meta.url), 'utf8'),
) as { hex: string; duel: { hex: string; decoded: Duel } };

const decode = (hex: string): Uint8Array =>
  Uint8Array.from(hex.match(/../g) ?? [], (h) => Number.parseInt(h, 16));
const duel = (): SnapshotReader => new SnapshotReader(decode(vector.duel.hex));
const d = vector.duel.decoded;

describe('SnapshotReader: the combat block of the Rust duel snapshot', () => {
  it('reads the weapons, the cues and the counters', () => {
    const r = duel();
    expect(r.valid()).toBe(true);
    expect(r.combatFlags()).toBe(d.flags);
    expect(r.seekerMode()).toBe(d.seeker);
    expect(r.launchInhibit()).toBe(d.inhibit);
    expect(r.launchInhibitAge()).toBe(d.inhibit_age);
    expect(r.gunRounds()).toBe(d.rounds);
    expect(r.missilesLeft()).toBe(d.missiles);
    expect(r.banditCount()).toBe(d.bandits);
    expect(r.eventSeq()).toBe(d.event_seq);
    const pip = new Float64Array(3);
    r.pipperPos(pip);
    expect([...pip]).toEqual(d.pipper_bf);
    expect(r.seekerSignal()).toBe(d.signal);
    expect([r.targetRange(), r.targetClosure(), r.targetAspect()]).toEqual([
      d.tgt_range,
      d.tgt_closure,
      d.tgt_aspect,
    ]);
    expect(r.playerHealth()).toBe(d.health);
    expect(Array.from({ length: COMBAT_COUNTER_COUNT }, (_, k) => r.combatCounter(k))).toEqual(
      d.counters,
    );
  });

  it('reads the bandit', () => {
    const r = duel();
    const b = d.bandit;
    const pos = new Float64Array(3);
    const quat = new Float32Array(4);
    const vel = new Float32Array(3);
    const surfaces = new Float32Array(3);
    r.banditPos(pos);
    r.banditQuat(quat);
    r.banditVel(vel);
    r.banditSurfaces(surfaces);
    expect([...pos]).toEqual(b.pos_bf);
    expect([...quat]).toEqual(b.quat);
    expect([...vel]).toEqual(b.vel_bf);
    expect([...surfaces]).toEqual(b.surfaces);
    expect(r.banditTef()).toBe(b.tef);
    expect(r.banditAirbrake()).toBe(b.airbrake);
    expect(r.banditGearPos()).toBe(b.gear_pos);
    expect(r.banditAbLevel()).toBe(b.ab_level);
    expect(r.banditHealth()).toBe(b.health);
    expect(r.banditTas()).toBe(b.tas);
    expect([r.banditLife(), r.banditAi(), r.banditSkill(), r.banditFlags()]).toEqual([
      b.life,
      b.ai,
      b.skill,
      b.flags,
    ]);
    expect(r.banditSpawnSeq()).toBe(b.spawn_seq);
  });

  it('reads the missiles, the tracers and the explosions', () => {
    const r = duel();
    const pos = new Float64Array(3);
    const vel = new Float32Array(3);
    d.missile_slots.forEach((m, i) => {
      r.missilePos(i, pos);
      r.missileVel(i, vel);
      expect([...pos], `missile ${String(i)}`).toEqual(m.pos_bf);
      expect([...vel], `missile ${String(i)}`).toEqual(m.vel_bf);
      expect([r.missileState(i), r.missileOwner(i)]).toEqual([m.state, m.owner]);
    });
    expect(r.tracerCount()).toBe(d.tracer_count);
    d.tracers.forEach((t, i) => {
      r.tracerPos(i, pos);
      r.tracerVel(i, vel);
      expect([...pos], `tracer ${String(i)}`).toEqual(t.pos_bf);
      expect([...vel], `tracer ${String(i)}`).toEqual(t.vel_bf);
      expect([r.tracerOwner(i), r.tracerId(i)]).toEqual([t.owner, t.id]);
    });
    d.explosions.forEach((x, i) => {
      r.explosionPos(i, pos);
      expect([...pos]).toEqual(x.pos_bf);
      expect([r.explosionAge(i), r.explosionKind(i)]).toEqual([x.age, x.kind]);
    });
  });

  it('reads the events where the ring keeps them', () => {
    const r = duel();
    for (const e of d.events) {
      const slot = (e.seq - 1) % SNAPSHOT.EVENT_CAPACITY;
      expect({
        seq: r.eventSeqAt(slot),
        tick: r.eventTickAt(slot),
        kind: r.eventKindAt(slot),
        actor: r.eventActorAt(slot),
        target: r.eventTargetAt(slot),
        arg: r.eventArgAt(slot),
        value: r.eventValueAt(slot),
      }).toEqual(e);
    }
  });
});

describe('readCombatStatus', () => {
  it('fills the status from the duel snapshot', () => {
    const s = newCombatStatus();
    readCombatStatus(duel(), s);
    expect([s.flags, s.seeker, s.rounds, s.missiles, s.eventSeq]).toEqual([
      d.flags,
      d.seeker,
      d.rounds,
      d.missiles,
      d.event_seq,
    ]);
    expect(s.counters[COMBAT_COUNTER.ROUNDS_FIRED]).toBe(d.counters[0]);
    expect(s.counters[COMBAT_COUNTER.BANDIT_SPAWNS]).toBe(d.counters[11]);
    expect([...s.banditPosBf]).toEqual(d.bandit.pos_bf);
    expect([...s.banditVelBf]).toEqual(d.bandit.vel_bf);
    expect([s.banditLife, s.banditAi, s.banditSkill, s.banditHealth]).toEqual([
      d.bandit.life,
      d.bandit.ai,
      d.bandit.skill,
      d.bandit.health,
    ]);
    expect(s.tgtRange).toBe(d.tgt_range);
  });

  it('measures the bandit and the newest explosion from the ownship', () => {
    const bytes = new Uint8Array(SNAPSHOT.SIZE);
    const v = new DataView(bytes.buffer);
    const put3 = (at: number, xyz: [number, number, number]): void => {
      xyz.forEach((x, k) => {
        v.setFloat64(at + 8 * k, x, true);
      });
    };
    put3(SNAPSHOT.O_POS_BF, [100, 200, 1000]);
    v.setUint8(SNAPSHOT.BD_LIFE, 1);
    put3(SNAPSHOT.BD_POS_BF, [100, 1400, 1500]); // 1 200 m north, 500 m up: 1 300 m
    const boom = (i: number, xyz: [number, number, number], age: number): void => {
      const at = SNAPSHOT.EXPLOSIONS + SNAPSHOT.EXPLOSION_STRIDE * i;
      put3(at + SNAPSHOT.XP_POS_BF, xyz);
      v.setFloat32(at + SNAPSHOT.XP_AGE, age, true);
      v.setUint8(at + SNAPSHOT.XP_KIND, 1);
    };
    boom(2, [100, 200, 3000], 0.5);
    boom(5, [400, 600, 1000], 0.1); // the newest: 300 east, 400 north, 500 m away
    const s = newCombatStatus();
    readCombatStatus(new SnapshotReader(bytes), s);
    expect(s.banditRange).toBeCloseTo(1300, 9);
    expect(s.explosionRange).toBeCloseTo(500, 9);
    // Without a bandit or an explosion, both are 0.
    v.setUint8(SNAPSHOT.BD_LIFE, 0);
    for (const i of [2, 5]) {
      v.setUint8(SNAPSHOT.EXPLOSIONS + SNAPSHOT.EXPLOSION_STRIDE * i + SNAPSHOT.XP_KIND, 0);
    }
    readCombatStatus(new SnapshotReader(bytes), s);
    expect([s.banditRange, s.explosionRange]).toEqual([0, 0]);
  });

  it('reads all zero from a free-flight snapshot, and keeps its arrays', () => {
    const s = newCombatStatus();
    const { pipperBf, counters, banditPosBf } = s;
    readCombatStatus(duel(), s);
    readCombatStatus(new SnapshotReader(decode(vector.hex)), s);
    expect(s.flags).toBe(0);
    expect(s.rounds).toBe(0);
    expect([...s.counters].every((c) => c === 0)).toBe(true);
    expect([...s.banditPosBf]).toEqual([0, 0, 0]);
    expect(s.pipperBf).toBe(pipperBf);
    expect(s.counters).toBe(counters);
    expect(s.banditPosBf).toBe(banditPosBf);
  });
});

/** A blank snapshot whose ring holds events `from` to `to` (each in its slot, kind = seq % 9 + 1). */
function ring(from: number, to: number): SnapshotReader {
  const b = new Uint8Array(SNAPSHOT.SIZE);
  const v = new DataView(b.buffer);
  for (let seq = from; seq <= to; seq++) {
    const at = SNAPSHOT.EVENTS + SNAPSHOT.EVENT_STRIDE * ((seq - 1) % SNAPSHOT.EVENT_CAPACITY);
    v.setUint32(at + SNAPSHOT.EV_SEQ, seq, true);
    v.setUint32(at + SNAPSHOT.EV_TICK, seq * 10, true);
    v.setUint8(at + SNAPSHOT.EV_KIND, (seq % 9) + 1);
  }
  v.setUint32(SNAPSHOT.C_EVENT_SEQ, to, true);
  return new SnapshotReader(b);
}

describe('EventCursor', () => {
  const seqs = (c: EventCursor, r: SnapshotReader): number[] => {
    const out: number[] = [];
    c.drain(r, (e) => out.push(e.seq));
    return out;
  };

  it('hands out the duel snapshot’s events once, oldest first', () => {
    const c = new EventCursor();
    const got: CombatEventView[] = [];
    expect(c.drain(duel(), (e) => got.push({ ...e }))).toBe(2);
    expect(got).toEqual(d.events);
    expect(got.map((e) => e.kind)).toEqual([
      COMBAT_EVENT.BANDIT_SPAWN,
      COMBAT_EVENT.MISSILE_LAUNCH,
    ]);
    expect(c.drain(duel(), () => expect.unreachable())).toBe(0);
    expect([c.lastSeq, c.lost]).toEqual([2, 0]);
  });

  it('takes only what is new, in order, across snapshots', () => {
    const c = new EventCursor();
    expect(seqs(c, ring(1, 5))).toEqual([1, 2, 3, 4, 5]);
    expect(seqs(c, ring(1, 9))).toEqual([6, 7, 8, 9]);
    expect(seqs(c, ring(1, 9))).toEqual([]);
    expect(c.lost).toBe(0);
  });

  it('counts the events that fell out of the ring before it looked', () => {
    const c = new EventCursor();
    expect(seqs(c, ring(1, 3))).toEqual([1, 2, 3]);
    // 37 more events, of which the ring keeps the newest 32 (9 to 40): 4 to 8 are lost.
    expect(seqs(c, ring(9, 40))).toEqual(Array.from({ length: 32 }, (_, i) => 9 + i));
    expect(c.lost).toBe(5);
    expect(c.lastSeq).toBe(40);
  });

  it('starts again with a new session', () => {
    const c = new EventCursor();
    seqs(c, ring(1, 12));
    expect(seqs(c, ring(1, 2))).toEqual([1, 2]);
    expect(c.lastSeq).toBe(2);
    c.reset();
    expect([c.lastSeq, c.lost]).toEqual([0, 0]);
  });
});
