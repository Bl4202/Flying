/**
 * The mission recorder (GDD 65.2, 65.3, ADR-49): watches one mission from Fly to its end and writes
 * its `mission_result/1`. It reads what the page already reads every frame, the aircraft's status
 * and the duel's, on the simulation's clock (a held simulation's time does not count).
 *
 * - **Sorties** follow GDD 65.2: one starts when the wheels have been off the ground for 10 s (dated
 *   from when they left it) or at an air start, and ends on the wheels below 5 m/s for 5 s, with the
 *   aircraft's destruction, or with the mission. A touchdown and a roll-out below 15 m/s with no
 *   crash is a **successful landing**; lifting off again (a touch-and-go) takes it back.
 * - **Records** (highest altitude, fastest true airspeed and Mach) are kept while alive, per sortie.
 * - **The duel's numbers** are the rise of the snapshot's session counters since Fly, summed sample
 *   by sample, so a missed frame loses nothing and a counter that falls (a new session) counts from
 *   zero again. Events add only the moment of the first kill, which the counters give a frame later.
 * - **The task** (GDD 4.5): from Fly to the first stop below 2 m/s on the wheels, after a landing,
 *   following the first kill.
 *
 * `update` makes no garbage: everything is a number or a typed array, and a sortie becomes an object
 * only when it ends.
 */
import {
  COMBAT_ACTOR,
  COMBAT_COUNTER,
  COMBAT_COUNTER_COUNT,
  COMBAT_EVENT,
  OWNSHIP_FLAG,
  OWNSHIP_FLAG2,
  SCENARIO,
  type AircraftStatus,
  type CombatEventView,
  type CombatStatus,
  type SessionConfig,
} from '@flying/sim-bridge';
import {
  MISSION_RESULT_SCHEMA,
  type MissionOutcome,
  type MissionResult,
  type SortieRecord,
} from './types.ts';

/** What the recorder needs to know about the session flown. */
export type RecorderSession = Pick<SessionConfig, 'scenario' | 'skill' | 'assist' | 'arena'>;

/** Wheels off this long make a sortie, ms. */
export const SORTIE_START_MS = 10_000;
/** A sortie ends on the wheels below this speed (m/s) for this long (ms). */
export const STOP_SPEED_MPS = 5;
export const STOP_HOLD_MS = 5_000;
/** A touchdown is a successful landing once the roll-out is below this speed, m/s. */
export const LANDED_SPEED_MPS = 15;
/** The task's stop, m/s (the bot playtester's stop, ADR-45 BP1). */
export const TASK_STOP_MPS = 2;

/** Where the aircraft is in the sortie cycle. */
const PHASE = {
  /** On the ground with no sortie open (or no status yet). */
  GROUND: 0,
  /** Off the wheels, not yet for 10 s. */
  LIFTING: 1,
  /** In a sortie, in the air. */
  AIRBORNE: 2,
  /** In a sortie, on the wheels after a touchdown. */
  ROLLING: 3,
} as const;

/** Every mission id this page has made, so two missions begun in one second differ. */
let idCounter = 0;

/** `m-<yyyymmdd>-<hhmmss>-<4 hex>` from a wall-clock time (UTC). */
export function missionId(wallMs: number, salt: number): string {
  const d = new Date(wallMs);
  const p = (n: number, w = 2): string => String(n).padStart(w, '0');
  const date = `${p(d.getUTCFullYear(), 4)}${p(d.getUTCMonth() + 1)}${p(d.getUTCDate())}`;
  const time = `${p(d.getUTCHours())}${p(d.getUTCMinutes())}${p(d.getUTCSeconds())}`;
  return `m-${date}-${time}-${(salt & 0xffff).toString(16).padStart(4, '0')}`;
}

export interface BeginOptions {
  /** The wall clock at Fly, ms since 1970 (default: now). */
  wallMs?: number;
  /** The mission id (default: made from the wall clock and a random salt). */
  id?: string;
}

export class MissionRecorder {
  #active = false;
  #session: RecorderSession = { scenario: 0, skill: 0, assist: 0, arena: 0 };
  #build = '';
  #id = '';
  #dateIso = '';

  // The clock: sim milliseconds of the first and the newest sample.
  #haveTime = false;
  #startMs = 0;
  #nowMs = 0;

  // The counters: the last values seen and the sum of their rises.
  #haveCounters = false;
  readonly #last = new Float64Array(COMBAT_COUNTER_COUNT);
  readonly #sum = new Float64Array(COMBAT_COUNTER_COUNT);

  // The sortie cycle.
  #haveStatus = false;
  #spawnSeq = 0;
  #phase: number = PHASE.GROUND;
  #offSinceMs = 0;
  #slowSinceMs = -1;
  #dead = false;
  #ended = false;
  // The sortie being flown (or the lift-off that may become one).
  #airborneMs = 0;
  #landed = false;
  #maxAlt = 0;
  #maxTas = 0;
  #maxMach = 0;
  #sorties: SortieRecord[] = [];
  /** Whether the last sortie to end ended with a successful landing. */
  #lastLanded = false;

  // The task.
  #firstKillMs = -1;
  #taskMs = -1;

  /** A mission is being recorded. */
  get active(): boolean {
    return this.#active;
  }

  get missionId(): string {
    return this.#id;
  }

  /**
   * Whether a sortie has just ended with a stop on the wheels (5 s below 5 m/s): in a duel, the
   * debrief opens. Stays true until another sortie starts.
   */
  sortieEnded(): boolean {
    return this.#ended;
  }

  /** Starts recording a mission (at Fly), forgetting any before. */
  begin(session: RecorderSession, build: string, options: BeginOptions = {}): void {
    const wallMs = options.wallMs ?? Date.now();
    idCounter += 1;
    this.#session = { ...session };
    this.#build = build;
    this.#id = options.id ?? missionId(wallMs, Math.floor(Math.random() * 0x10000) ^ idCounter);
    this.#dateIso = new Date(wallMs).toISOString();
    this.#active = true;
    this.#restart();
  }

  /** Forgets everything measured (a new mission, or the clock running backwards: a fresh core). */
  #restart(): void {
    this.#haveTime = false;
    this.#startMs = 0;
    this.#nowMs = 0;
    this.#haveCounters = false;
    this.#last.fill(0);
    this.#sum.fill(0);
    this.#haveStatus = false;
    this.#spawnSeq = 0;
    this.#phase = PHASE.GROUND;
    this.#offSinceMs = 0;
    this.#slowSinceMs = -1;
    this.#dead = false;
    this.#ended = false;
    this.#resetFlight();
    this.#sorties = [];
    this.#lastLanded = false;
    this.#firstKillMs = -1;
    this.#taskMs = -1;
  }

  #resetFlight(): void {
    this.#airborneMs = 0;
    this.#landed = false;
    this.#maxAlt = 0;
    this.#maxTas = 0;
    this.#maxMach = 0;
  }

  /**
   * One sample: the aircraft's status and the duel's (either may be missing), at `simMs`, the
   * simulation's clock in milliseconds. Makes no garbage.
   */
  update(
    status: AircraftStatus | undefined,
    combat: CombatStatus | undefined,
    simMs: number,
  ): void {
    if (!this.#active) return;
    if (this.#haveTime && simMs < this.#nowMs - 1) this.#restart();
    const dt = this.#haveTime ? Math.max(simMs - this.#nowMs, 0) : 0;
    if (!this.#haveTime) this.#startMs = simMs;
    this.#haveTime = true;
    this.#nowMs = simMs;

    if (combat !== undefined) this.#count(combat.counters);
    if (this.#firstKillMs < 0 && (this.#sum[COMBAT_COUNTER.KILLS] as number) > 0) {
      this.#firstKillMs = simMs;
    }
    if (status !== undefined) this.#fly(status, dt, simMs);
  }

  /** An event from the snapshot's ring (shared memory only): the first kill's moment. */
  onEvent(e: CombatEventView): void {
    if (!this.#active) return;
    if (
      e.kind === COMBAT_EVENT.KILL &&
      e.actor === COMBAT_ACTOR.PLAYER &&
      this.#firstKillMs < 0 &&
      this.#haveTime
    ) {
      this.#firstKillMs = this.#nowMs;
    }
  }

  #count(counters: Uint32Array): void {
    const last = this.#last;
    const sum = this.#sum;
    for (let k = 0; k < COMBAT_COUNTER_COUNT; k++) {
      const now = counters[k] ?? 0;
      if (this.#haveCounters) {
        const rise = now - (last[k] as number);
        sum[k] = (sum[k] as number) + (rise >= 0 ? rise : now);
      }
      last[k] = now;
    }
    this.#haveCounters = true;
  }

  #fly(s: AircraftStatus, dt: number, now: number): void {
    const wow = (s.flags2 & OWNSHIP_FLAG2.WOW) !== 0;
    const crashed = (s.flags & (OWNSHIP_FLAG.CRASHED | OWNSHIP_FLAG.SPECTATOR)) !== 0;
    const first = !this.#haveStatus;
    const respawned = !first && s.spawnSeq !== this.#spawnSeq;
    this.#haveStatus = true;
    this.#spawnSeq = s.spawnSeq;

    if (respawned) {
      // A new aircraft: whatever the last one was doing is over.
      this.#close(false);
      this.#phase = PHASE.GROUND;
    }
    if (crashed) {
      // Destroyed (or watching): the sortie ends without a landing.
      this.#close(false);
      this.#phase = PHASE.GROUND;
      this.#dead = true;
      return;
    }
    this.#dead = false;
    if ((first || respawned) && !wow) {
      // An air start is a sortie from its first moment.
      this.#open();
      this.#phase = PHASE.AIRBORNE;
    }

    const gs = s.groundSpeed;
    switch (this.#phase) {
      case PHASE.GROUND:
        if (!wow) {
          this.#phase = PHASE.LIFTING;
          this.#offSinceMs = now;
          this.#resetFlight();
        }
        break;
      case PHASE.LIFTING:
        if (wow) {
          this.#phase = PHASE.GROUND;
        } else if (now - this.#offSinceMs >= SORTIE_START_MS) {
          // The records since lift-off are the sortie's.
          this.#ended = false;
          this.#phase = PHASE.AIRBORNE;
        }
        break;
      case PHASE.AIRBORNE:
        if (wow) {
          this.#phase = PHASE.ROLLING;
          this.#landed = false;
          this.#slowSinceMs = -1;
        }
        break;
      case PHASE.ROLLING:
        if (!wow) {
          // A touch-and-go: still the same sortie, and not a landing.
          this.#phase = PHASE.AIRBORNE;
          this.#landed = false;
          break;
        }
        if (gs < LANDED_SPEED_MPS) this.#landed = true;
        if (this.#landed && this.#taskMs < 0 && this.#firstKillMs >= 0 && gs < TASK_STOP_MPS) {
          this.#taskMs = now;
        }
        if (gs < STOP_SPEED_MPS) {
          if (this.#slowSinceMs < 0) this.#slowSinceMs = now;
          else if (now - this.#slowSinceMs >= STOP_HOLD_MS) {
            this.#close(this.#landed);
            this.#phase = PHASE.GROUND;
            this.#ended = true;
          }
        } else {
          this.#slowSinceMs = -1;
        }
        break;
      default:
        break;
    }

    if (this.#phase !== PHASE.GROUND) {
      if (!wow) this.#airborneMs += dt;
      if (s.altitude > this.#maxAlt) this.#maxAlt = s.altitude;
      if (s.tas > this.#maxTas) this.#maxTas = s.tas;
      if (s.mach > this.#maxMach) this.#maxMach = s.mach;
    }
  }

  /** A sortie starts now (an air start). */
  #open(): void {
    this.#resetFlight();
    this.#ended = false;
  }

  /** The open sortie (if any) ends; `landed` if it ended with a successful landing. */
  #close(landed: boolean): void {
    if (this.#phase !== PHASE.AIRBORNE && this.#phase !== PHASE.ROLLING) return;
    this.#sorties.push({
      airborne_s: this.#airborneMs / 1000,
      landed,
      max_alt_m: this.#maxAlt,
      max_tas_mps: this.#maxTas,
      max_mach: this.#maxMach,
    });
    this.#lastLanded = landed;
    this.#resetFlight();
  }

  /**
   * Ends the mission: a sortie still open ends with it, and the result is written. The recorder
   * stops until the next `begin`.
   */
  finish(): MissionResult {
    const onWheelsLanded = this.#phase === PHASE.ROLLING && this.#landed;
    if (this.#phase === PHASE.AIRBORNE || this.#phase === PHASE.ROLLING) {
      this.#close(onWheelsLanded);
    }
    const outcome: MissionOutcome = this.#dead
      ? 'died'
      : this.#lastLanded && this.#sorties.length > 0
        ? 'landed'
        : 'ended';
    const sum = this.#sum;
    const n = (k: number): number => sum[k] as number;
    const result: MissionResult = {
      schema: MISSION_RESULT_SCHEMA,
      mission_id: this.#id,
      build: this.#build,
      date_iso: this.#dateIso,
      mode: this.#session.scenario === SCENARIO.FREE ? 'free' : 'duel',
      assist: this.#session.assist,
      skill: this.#session.scenario === SCENARIO.FREE ? 0 : this.#session.skill,
      arena: this.#session.arena,
      duration_s: this.#haveTime ? (this.#nowMs - this.#startMs) / 1000 : 0,
      sorties: this.#sorties,
      combat: {
        kills: n(COMBAT_COUNTER.KILLS),
        gun_kills: n(COMBAT_COUNTER.GUN_KILLS),
        missile_kills: n(COMBAT_COUNTER.MISSILE_KILLS),
        deaths: n(COMBAT_COUNTER.DEATHS),
        missiles_fired: n(COMBAT_COUNTER.MISSILES_FIRED),
        missile_hits: n(COMBAT_COUNTER.MISSILE_HITS),
        gun_rounds: n(COMBAT_COUNTER.ROUNDS_FIRED),
        gun_hits: n(COMBAT_COUNTER.ROUND_HITS),
        hits_taken: n(COMBAT_COUNTER.HITS_TAKEN),
      },
      task_s: this.#taskMs < 0 ? null : (this.#taskMs - this.#startMs) / 1000,
      outcome,
    };
    this.#active = false;
    this.#sorties = [];
    return result;
  }
}
