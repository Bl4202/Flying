/**
 * The player's statistics and feel answers, kept in the browser (GDD 65.3, 65.6, ADR-49).
 *
 * - `stats/1`: totals, personal bests (each with the mission it came from), the newest 50 results,
 *   and the id of every mission ever counted. A result is aggregated by its `mission_id`: once
 *   only, however often it is offered. The new value is written in **one** `set`, and the copy in
 *   memory changes only once that write has succeeded, so a failed write leaves both as they were
 *   and a retry counts the mission exactly once (AC-65.2).
 * - `feel/1`: the answers to the feel questionnaire, oldest first.
 *
 * Export writes both as one canonical JSON file (fixed key order, two-space indent, a final
 * newline). Import is strict: it names every value it cannot use by JSON pointer and changes
 * nothing; a good file is merged in by summing the totals and keeping the better bests (AC-65.6),
 * and a file whose missions are all counted here already adds nothing to the totals.
 */
import type { Kv } from '@flying/platform-web';
import {
  COMBAT_KEYS,
  MISSION_RESULT_SCHEMA,
  zeroCombat,
  type CombatNumbers,
  type MissionResult,
  type SortieRecord,
} from './types.ts';

export const STATS_KEY = 'stats/1';
export const FEEL_KEY = 'feel/1';
export const STATS_FILE_NAME = 'flying-stats.json';
export const EXPORT_SCHEMA = 'stats_export/1';
/** The newest results kept in the log. */
export const LOG_LIMIT = 50;
/** The longest comment the questionnaire keeps. */
export const COMMENT_MAX = 500;

export interface StatsTotals extends CombatNumbers {
  missions: number;
  sorties: number;
  landings: number;
  airborne_s: number;
}

/** A personal best and the mission it was set in. */
export interface Best {
  value: number;
  mission_id: string;
}

export interface StatsBests {
  longest_sortie_s: Best | null;
  max_alt_m: Best | null;
  max_tas_mps: Best | null;
  max_mach: Best | null;
  /** The shortest duel task (GDD 4.5). */
  best_task_s: Best | null;
}

export interface StatsData {
  schema: 'stats/1';
  totals: StatsTotals;
  bests: StatsBests;
  /** Newest first. */
  log: MissionResult[];
  /** Every mission counted. */
  seen: string[];
}

/** One set of answers to the feel questionnaire (each 1 strongly disagree to 5 strongly agree). */
export interface FeelAnswer {
  date_iso: string;
  build: string;
  assist: number;
  skill: number;
  believable: number;
  satisfying: number;
  easy: number;
  comment: string;
  /** The mission rated, or '' if none. */
  mission_id: string;
}

export interface FeelData {
  schema: 'feel/1';
  answers: FeelAnswer[];
}

const TOTAL_KEYS: readonly (keyof StatsTotals)[] = [
  'missions',
  'sorties',
  'landings',
  'airborne_s',
  ...COMBAT_KEYS,
];

/** Which way is better for each best. */
const BEST_KEYS: readonly [keyof StatsBests, 'max' | 'min'][] = [
  ['longest_sortie_s', 'max'],
  ['max_alt_m', 'max'],
  ['max_tas_mps', 'max'],
  ['max_mach', 'max'],
  ['best_task_s', 'min'],
];

export function emptyStats(): StatsData {
  return {
    schema: 'stats/1',
    totals: { missions: 0, sorties: 0, landings: 0, airborne_s: 0, ...zeroCombat() },
    bests: {
      longest_sortie_s: null,
      max_alt_m: null,
      max_tas_mps: null,
      max_mach: null,
      best_task_s: null,
    },
    log: [],
    seen: [],
  };
}

export function emptyFeel(): FeelData {
  return { schema: 'feel/1', answers: [] };
}

function better(a: Best | null, b: Best | null, way: 'max' | 'min'): Best | null {
  if (a === null) return b;
  if (b === null) return a;
  if (way === 'max' ? b.value > a.value : b.value < a.value) return b;
  return a;
}

/** Newest first: by date, then by id, so the order never depends on how the log was built. */
function byNewest(a: MissionResult, b: MissionResult): number {
  if (a.date_iso !== b.date_iso) return a.date_iso < b.date_iso ? 1 : -1;
  return a.mission_id < b.mission_id ? 1 : a.mission_id > b.mission_id ? -1 : 0;
}

/** `stats` with `result` counted in (a new object), or `stats` itself if it was counted already. */
export function applyResult(stats: StatsData, result: MissionResult): StatsData {
  if (stats.seen.includes(result.mission_id)) return stats;
  const totals = { ...stats.totals };
  totals.missions += 1;
  totals.sorties += result.sorties.length;
  totals.landings += result.sorties.filter((s) => s.landed).length;
  totals.airborne_s += result.sorties.reduce((sum, s) => sum + s.airborne_s, 0);
  for (const k of COMBAT_KEYS) totals[k] += result.combat[k];
  const id = result.mission_id;
  const top = (pick: (s: SortieRecord) => number): Best | null =>
    result.sorties.length === 0
      ? null
      : { value: Math.max(...result.sorties.map(pick)), mission_id: id };
  const mine: StatsBests = {
    longest_sortie_s: top((s) => s.airborne_s),
    max_alt_m: top((s) => s.max_alt_m),
    max_tas_mps: top((s) => s.max_tas_mps),
    max_mach: top((s) => s.max_mach),
    best_task_s: result.task_s === null ? null : { value: result.task_s, mission_id: id },
  };
  return {
    schema: 'stats/1',
    totals,
    bests: mergeBests(stats.bests, mine),
    log: [structuredClone(result), ...stats.log].sort(byNewest).slice(0, LOG_LIMIT),
    seen: [...stats.seen, id],
  };
}

function mergeBests(a: StatsBests, b: StatsBests): StatsBests {
  const out = { ...a };
  for (const [k, way] of BEST_KEYS) out[k] = better(a[k], b[k], way);
  return out;
}

/** `b` merged into `a` (an imported file into what is here). */
export function mergeStats(a: StatsData, b: StatsData): StatsData {
  const seen = new Set(a.seen);
  const news = b.seen.filter((id) => !seen.has(id));
  const totals = { ...a.totals };
  if (news.length > 0) {
    for (const k of TOTAL_KEYS) totals[k] += b.totals[k];
  }
  const ids = new Set(a.log.map((r) => r.mission_id));
  return {
    schema: 'stats/1',
    totals,
    bests: mergeBests(a.bests, b.bests),
    log: [...a.log, ...b.log.filter((r) => !ids.has(r.mission_id))]
      .sort(byNewest)
      .slice(0, LOG_LIMIT),
    seen: [...a.seen, ...news],
  };
}

function mergeFeel(a: FeelData, b: FeelData): FeelData {
  const key = (x: FeelAnswer): string => `${x.date_iso}|${x.mission_id}`;
  const have = new Set(a.answers.map(key));
  return {
    schema: 'feel/1',
    answers: [...a.answers, ...b.answers.filter((x) => !have.has(key(x)))],
  };
}

// ---------------------------------------------------------------------------------------------
// Reading what was stored or imported: every value checked, every object rebuilt in a fixed order.

export interface StatsProblem {
  /** JSON pointer to the value, `''` for the whole file. */
  pointer: string;
  message: string;
}

class Reader {
  readonly problems: StatsProblem[] = [];
  fail(pointer: string, message: string): void {
    this.problems.push({ pointer, message });
  }
  obj(v: unknown, at: string): Record<string, unknown> {
    if (typeof v === 'object' && v !== null && !Array.isArray(v)) {
      return v as Record<string, unknown>;
    }
    this.fail(at, 'expected an object');
    return {};
  }
  arr(v: unknown, at: string): unknown[] {
    if (Array.isArray(v)) return v;
    this.fail(at, 'expected a list');
    return [];
  }
  num(v: unknown, at: string, min = 0, max = Number.MAX_VALUE): number {
    if (typeof v === 'number' && Number.isFinite(v) && v >= min && v <= max) return v;
    this.fail(
      at,
      `expected a number from ${String(min)}${max < Number.MAX_VALUE ? ` to ${String(max)}` : ' up'}`,
    );
    return min;
  }
  int(v: unknown, at: string, min = 0, max = Number.MAX_SAFE_INTEGER): number {
    const n = this.num(v, at, min, max);
    if (!Number.isInteger(n)) this.fail(at, 'expected a whole number');
    return n;
  }
  str(v: unknown, at: string, maxLength = 200): string {
    if (typeof v === 'string' && v.length <= maxLength) return v;
    this.fail(at, `expected text of at most ${String(maxLength)} characters`);
    return '';
  }
  bool(v: unknown, at: string): boolean {
    if (typeof v === 'boolean') return v;
    this.fail(at, 'expected true or false');
    return false;
  }
  is(v: unknown, want: string, at: string): void {
    if (v !== want) this.fail(at, `expected "${want}"`);
  }
  oneOf<T extends string>(v: unknown, choices: readonly T[], at: string): T {
    const c = choices.find((x) => x === v);
    if (c !== undefined) return c;
    this.fail(at, `expected one of ${choices.join(', ')}`);
    return choices[0] as T;
  }

  combat(v: unknown, at: string): CombatNumbers {
    const o = this.obj(v, at);
    const out = zeroCombat();
    for (const k of COMBAT_KEYS) out[k] = this.int(o[k], `${at}/${k}`);
    return out;
  }

  sortie(v: unknown, at: string): SortieRecord {
    const o = this.obj(v, at);
    return {
      airborne_s: this.num(o['airborne_s'], `${at}/airborne_s`),
      landed: this.bool(o['landed'], `${at}/landed`),
      max_alt_m: this.num(o['max_alt_m'], `${at}/max_alt_m`, -1000),
      max_tas_mps: this.num(o['max_tas_mps'], `${at}/max_tas_mps`),
      max_mach: this.num(o['max_mach'], `${at}/max_mach`),
    };
  }

  result(v: unknown, at: string): MissionResult {
    const o = this.obj(v, at);
    this.is(o['schema'], MISSION_RESULT_SCHEMA, `${at}/schema`);
    const task = o['task_s'];
    return {
      schema: MISSION_RESULT_SCHEMA,
      mission_id: this.str(o['mission_id'], `${at}/mission_id`, 64),
      build: this.str(o['build'], `${at}/build`, 64),
      date_iso: this.str(o['date_iso'], `${at}/date_iso`, 40),
      mode: this.oneOf(o['mode'], ['free', 'duel'] as const, `${at}/mode`),
      assist: this.int(o['assist'], `${at}/assist`, 0, 4),
      skill: this.int(o['skill'], `${at}/skill`, 0, 3),
      arena: this.int(o['arena'], `${at}/arena`, 0, 255),
      duration_s: this.num(o['duration_s'], `${at}/duration_s`),
      sorties: this.arr(o['sorties'], `${at}/sorties`).map((s, i) =>
        this.sortie(s, `${at}/sorties/${String(i)}`),
      ),
      combat: this.combat(o['combat'], `${at}/combat`),
      task_s: task === null ? null : this.num(task, `${at}/task_s`),
      outcome: this.oneOf(o['outcome'], ['landed', 'ended', 'died'] as const, `${at}/outcome`),
    };
  }

  best(v: unknown, at: string): Best | null {
    if (v === null) return null;
    const o = this.obj(v, at);
    return {
      value: this.num(o['value'], `${at}/value`, -1000),
      mission_id: this.str(o['mission_id'], `${at}/mission_id`, 64),
    };
  }

  stats(v: unknown, at: string): StatsData {
    const o = this.obj(v, at);
    this.is(o['schema'], 'stats/1', `${at}/schema`);
    const t = this.obj(o['totals'], `${at}/totals`);
    const totals = emptyStats().totals;
    for (const k of TOTAL_KEYS) {
      totals[k] =
        k === 'airborne_s'
          ? this.num(t[k], `${at}/totals/${k}`)
          : this.int(t[k], `${at}/totals/${k}`);
    }
    const b = this.obj(o['bests'], `${at}/bests`);
    const bests = emptyStats().bests;
    for (const [k] of BEST_KEYS) bests[k] = this.best(b[k], `${at}/bests/${k}`);
    const log = this.arr(o['log'], `${at}/log`);
    if (log.length > LOG_LIMIT)
      this.fail(`${at}/log`, `expected at most ${String(LOG_LIMIT)} results`);
    return {
      schema: 'stats/1',
      totals,
      bests,
      log: log.map((r, i) => this.result(r, `${at}/log/${String(i)}`)).sort(byNewest),
      seen: this.arr(o['seen'], `${at}/seen`).map((s, i) =>
        this.str(s, `${at}/seen/${String(i)}`, 64),
      ),
    };
  }

  answer(v: unknown, at: string): FeelAnswer {
    const o = this.obj(v, at);
    return {
      date_iso: this.str(o['date_iso'], `${at}/date_iso`, 40),
      build: this.str(o['build'], `${at}/build`, 64),
      assist: this.int(o['assist'], `${at}/assist`, 0, 4),
      skill: this.int(o['skill'], `${at}/skill`, 0, 3),
      believable: this.int(o['believable'], `${at}/believable`, 1, 5),
      satisfying: this.int(o['satisfying'], `${at}/satisfying`, 1, 5),
      easy: this.int(o['easy'], `${at}/easy`, 1, 5),
      comment: this.str(o['comment'], `${at}/comment`, COMMENT_MAX),
      mission_id: this.str(o['mission_id'], `${at}/mission_id`, 64),
    };
  }

  feel(v: unknown, at: string): FeelData {
    const o = this.obj(v, at);
    this.is(o['schema'], 'feel/1', `${at}/schema`);
    return {
      schema: 'feel/1',
      answers: this.arr(o['answers'], `${at}/answers`).map((a, i) =>
        this.answer(a, `${at}/answers/${String(i)}`),
      ),
    };
  }
}

/** A stored `stats/1`, or the empty statistics if it is missing or unusable. */
export function readStats(raw: unknown): StatsData {
  if (raw === undefined) return emptyStats();
  const r = new Reader();
  const s = r.stats(raw, '');
  return r.problems.length === 0 ? s : emptyStats();
}

/** A stored `feel/1`, or none if it is missing or unusable. */
export function readFeel(raw: unknown): FeelData {
  if (raw === undefined) return emptyFeel();
  const r = new Reader();
  const f = r.feel(raw, '');
  return r.problems.length === 0 ? f : emptyFeel();
}

/** The statistics and the answers as a file's text. */
export function exportStats(stats: StatsData, feel: FeelData): string {
  // Through the reader, so the keys come out in its fixed order whatever built the objects.
  const s = readStats(stats);
  const f = readFeel(feel);
  return `${JSON.stringify({ schema: EXPORT_SCHEMA, stats: s, feel: f }, null, 2)}\n`;
}

export type StatsImportResult =
  { ok: true; stats: StatsData; feel: FeelData } | { ok: false; problems: StatsProblem[] };

/** Reads an exported file. Nothing is applied here. */
export function importStats(text: string): StatsImportResult {
  let raw: unknown;
  try {
    raw = JSON.parse(text);
  } catch {
    return { ok: false, problems: [{ pointer: '', message: 'this is not JSON' }] };
  }
  const r = new Reader();
  const o = r.obj(raw, '');
  if (r.problems.length > 0) return { ok: false, problems: r.problems };
  r.is(o['schema'], EXPORT_SCHEMA, '/schema');
  const stats = r.stats(o['stats'], '/stats');
  const feel = r.feel(o['feel'], '/feel');
  return r.problems.length > 0 ? { ok: false, problems: r.problems } : { ok: true, stats, feel };
}

/** The statistics and answers in memory, backed by the key-value store. */
export class StatsStore {
  readonly #kv: Kv;
  #stats: StatsData = emptyStats();
  #feel: FeelData = emptyFeel();

  constructor(kv: Kv) {
    this.#kv = kv;
  }

  get stats(): Readonly<StatsData> {
    return this.#stats;
  }

  get feel(): Readonly<FeelData> {
    return this.#feel;
  }

  /** Reads what was stored (an unreadable store, or value, gives the empty statistics). */
  async load(): Promise<void> {
    try {
      this.#stats = readStats(await this.#kv.get<unknown>(STATS_KEY));
      this.#feel = readFeel(await this.#kv.get<unknown>(FEEL_KEY));
    } catch {
      this.#stats = emptyStats();
      this.#feel = emptyFeel();
    }
  }

  /**
   * Counts a mission's result, once. Returns whether it was new. Throws if the store's write
   * fails, and then nothing has changed: offer the result again to count it.
   */
  async aggregate(result: MissionResult): Promise<boolean> {
    const next = applyResult(this.#stats, result);
    if (next === this.#stats) return false;
    await this.#kv.set(STATS_KEY, next);
    this.#stats = next;
    return true;
  }

  /** Keeps one set of questionnaire answers (the comment cut to its limit). */
  async addFeel(answer: FeelAnswer): Promise<void> {
    const next: FeelData = {
      schema: 'feel/1',
      answers: [
        ...this.#feel.answers,
        { ...answer, comment: answer.comment.slice(0, COMMENT_MAX) },
      ],
    };
    await this.#kv.set(FEEL_KEY, next);
    this.#feel = next;
  }

  /** Both, as a file's text. */
  export(): string {
    return exportStats(this.#stats, this.#feel);
  }

  /** Merges an exported file in; on a problem nothing changes and the problems are returned. */
  async import(text: string): Promise<StatsImportResult> {
    const r = importStats(text);
    if (!r.ok) return r;
    const stats = mergeStats(this.#stats, r.stats);
    const feel = mergeFeel(this.#feel, r.feel);
    await this.#kv.set(STATS_KEY, stats);
    await this.#kv.set(FEEL_KEY, feel);
    this.#stats = stats;
    this.#feel = feel;
    return { ok: true, stats, feel };
  }

  /** Forgets every statistic and answer. */
  async clear(): Promise<void> {
    await this.#kv.set(STATS_KEY, emptyStats());
    await this.#kv.set(FEEL_KEY, emptyFeel());
    this.#stats = emptyStats();
    this.#feel = emptyFeel();
  }
}

/** The median of some answers (each 1 to 5), or undefined for none. */
export function median(values: readonly number[]): number | undefined {
  if (values.length === 0) return undefined;
  const s = [...values].sort((a, b) => a - b);
  const mid = s.length >> 1;
  const hi = s[mid] ?? 0;
  return s.length % 2 === 1 ? hi : ((s[mid - 1] ?? 0) + hi) / 2;
}
