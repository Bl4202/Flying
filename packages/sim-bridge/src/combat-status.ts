/**
 * The duel as the page needs it (ADR-48): the player's weapons and cues, the target, the bandit
 * and the session's counters. Read from a snapshot into one reused object, so the main thread can
 * do it every animation frame and the Sim Worker four times a second without making garbage. All
 * zero without combat.
 */
import type { SnapshotReader } from './snapshot.ts';

/** Indices into `CombatStatus.counters` (the snapshot's `C_ROUNDS_FIRED` onward, in order). */
export const COMBAT_COUNTER = {
  ROUNDS_FIRED: 0,
  ROUND_HITS: 1,
  MISSILES_FIRED: 2,
  MISSILE_HITS: 3,
  KILLS: 4,
  GUN_KILLS: 5,
  MISSILE_KILLS: 6,
  DEATHS: 7,
  HITS_TAKEN: 8,
  BANDIT_ROUNDS: 9,
  EXPLOSIONS: 10,
  BANDIT_SPAWNS: 11,
} as const;

/** How many counters the snapshot carries. */
export const COMBAT_COUNTER_COUNT = 12;

export interface CombatStatus {
  /** `COMBAT_FLAG` bits (`ACTIVE` set in a duel). */
  flags: number;
  /** `SEEKER_MODE` of the next missile. */
  seeker: number;
  /** `LAUNCH_INHIBIT` of the last refused launch (0 none), and how long ago, seconds. */
  inhibit: number;
  inhibitAge: number;
  /** Gun rounds and missiles left. */
  rounds: number;
  missiles: number;
  /** The gunsight's aim point, BF metres (while `COMBAT_FLAG.PIPPER`). */
  pipperBf: Float64Array;
  /** The seeker's signal ratio (1 at the lock range; can exceed 1). */
  signal: number;
  /** The designated target's range (m), closure (m/s) and aspect (rad); 0 without one. */
  tgtRange: number;
  tgtClosure: number;
  tgtAspect: number;
  /** The player's health, 0 to 1. */
  health: number;
  /** The session's counters, `COMBAT_COUNTER` order. */
  counters: Uint32Array;
  /** The newest event's sequence number. */
  eventSeq: number;
  /** `BANDIT_LIFE`, `BANDIT_AI` and the skill rank (1 to 3). */
  banditLife: number;
  banditAi: number;
  banditSkill: number;
  /** `BANDIT_FLAG` bits. */
  banditFlags: number;
  /** The bandit's reference point (BF metres) and velocity (BF m/s). */
  banditPosBf: Float64Array;
  banditVelBf: Float32Array;
  /** The bandit's health, 0 to 1. */
  banditHealth: number;
}

/** An all-zero status. */
export function newCombatStatus(): CombatStatus {
  return {
    flags: 0,
    seeker: 0,
    inhibit: 0,
    inhibitAge: 0,
    rounds: 0,
    missiles: 0,
    pipperBf: new Float64Array(3),
    signal: 0,
    tgtRange: 0,
    tgtClosure: 0,
    tgtAspect: 0,
    health: 0,
    counters: new Uint32Array(COMBAT_COUNTER_COUNT),
    eventSeq: 0,
    banditLife: 0,
    banditAi: 0,
    banditSkill: 0,
    banditFlags: 0,
    banditPosBf: new Float64Array(3),
    banditVelBf: new Float32Array(3),
    banditHealth: 0,
  };
}

/** Fills `out` from a snapshot. Makes no garbage. */
export function readCombatStatus(reader: SnapshotReader, out: CombatStatus): void {
  out.flags = reader.combatFlags();
  out.seeker = reader.seekerMode();
  out.inhibit = reader.launchInhibit();
  out.inhibitAge = reader.launchInhibitAge();
  out.rounds = reader.gunRounds();
  out.missiles = reader.missilesLeft();
  reader.pipperPos(out.pipperBf);
  out.signal = reader.seekerSignal();
  out.tgtRange = reader.targetRange();
  out.tgtClosure = reader.targetClosure();
  out.tgtAspect = reader.targetAspect();
  out.health = reader.playerHealth();
  for (let k = 0; k < COMBAT_COUNTER_COUNT; k++) {
    out.counters[k] = reader.combatCounter(k);
  }
  out.eventSeq = reader.eventSeq();
  out.banditLife = reader.banditLife();
  out.banditAi = reader.banditAi();
  out.banditSkill = reader.banditSkill();
  out.banditFlags = reader.banditFlags();
  reader.banditPos(out.banditPosBf);
  reader.banditVel(out.banditVelBf);
  out.banditHealth = reader.banditHealth();
}
