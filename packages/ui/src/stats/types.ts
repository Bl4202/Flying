/**
 * What a mission leaves behind (GDD 65.4, ADR-49): `mission_result/1`, the subset this build can
 * fill. A result is written once, when the mission ends, and is never changed; the statistics are
 * aggregated from results by their `mission_id`, so counting one twice is impossible.
 */

export const MISSION_RESULT_SCHEMA = 'mission_result/1';

/** Free flight, or a duel against one bandit (the merge, a test start, counts as a duel). */
export type MissionMode = 'free' | 'duel';

/** How the mission ended: stopped on the runway after a sortie, left otherwise, or destroyed last. */
export type MissionOutcome = 'landed' | 'ended' | 'died';

/** One sortie (GDD 65.2): from leaving the ground (or an air start) to a stop, a loss or the end. */
export interface SortieRecord {
  /** Time off the wheels during the sortie, s (sim time). */
  airborne_s: number;
  /** It ended with a successful landing: a touchdown and a roll-out below 15 m/s, intact. */
  landed: boolean;
  /** The highest altitude above sea level, the fastest true airspeed and Mach, while alive. */
  max_alt_m: number;
  max_tas_mps: number;
  max_mach: number;
}

/** The duel's numbers over the whole mission (the snapshot's counters' change). */
export interface CombatNumbers {
  kills: number;
  gun_kills: number;
  missile_kills: number;
  deaths: number;
  missiles_fired: number;
  missile_hits: number;
  gun_rounds: number;
  gun_hits: number;
  hits_taken: number;
}

export interface MissionResult {
  schema: typeof MISSION_RESULT_SCHEMA;
  /** Unique per mission: `m-<yyyymmdd>-<hhmmss>-<4 hex>`. */
  mission_id: string;
  /** The build that flew it. */
  build: string;
  /** When it started, ISO 8601 (UTC). */
  date_iso: string;
  mode: MissionMode;
  /** `ASSIST_LEVEL` at Fly, the bandit's skill (0 in free flight) and the `ARENA`. */
  assist: number;
  skill: number;
  arena: number;
  /** Sim time from Fly to the end, s (a held simulation does not count). */
  duration_s: number;
  sorties: SortieRecord[];
  combat: CombatNumbers;
  /**
   * The duel's task (GDD 4.5): sim time from Fly to the first stop on the runway after the first
   * kill, s; null if it was not done.
   */
  task_s: number | null;
  outcome: MissionOutcome;
}

/** All zero. */
export function zeroCombat(): CombatNumbers {
  return {
    kills: 0,
    gun_kills: 0,
    missile_kills: 0,
    deaths: 0,
    missiles_fired: 0,
    missile_hits: 0,
    gun_rounds: 0,
    gun_hits: 0,
    hits_taken: 0,
  };
}

/** The combat number keys, in the order they are shown and stored. */
export const COMBAT_KEYS: readonly (keyof CombatNumbers)[] = [
  'kills',
  'gun_kills',
  'missile_kills',
  'deaths',
  'missiles_fired',
  'missile_hits',
  'gun_rounds',
  'gun_hits',
  'hits_taken',
];
