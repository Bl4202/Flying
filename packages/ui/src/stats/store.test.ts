/**
 * The statistics store: ST2 (a result counts once, however often it is offered and whatever fails
 * in between), ST5's unit part (export and import round-trip byte for byte; clear empties), the
 * merge of an imported file (AC-65.6) and the strict import.
 */
import { describe, expect, it } from 'vitest';
import { memoryKv, type Kv } from '@flying/platform-web';
import {
  applyResult,
  COMMENT_MAX,
  emptyStats,
  exportStats,
  FEEL_KEY,
  importStats,
  LOG_LIMIT,
  median,
  mergeStats,
  readStats,
  STATS_KEY,
  StatsStore,
  type FeelAnswer,
} from './store.ts';
import { zeroCombat, type MissionResult } from './types.ts';

function result(id: string, over: Partial<MissionResult> = {}): MissionResult {
  return {
    schema: 'mission_result/1',
    mission_id: id,
    build: 'b1',
    date_iso: `2026-11-10T09:${id.slice(-2).padStart(2, '0')}:00.000Z`,
    mode: 'duel',
    assist: 2,
    skill: 1,
    arena: 0,
    duration_s: 600,
    sorties: [{ airborne_s: 420, landed: true, max_alt_m: 3200, max_tas_mps: 250, max_mach: 0.8 }],
    combat: { ...zeroCombat(), kills: 1, gun_kills: 1, gun_rounds: 120, gun_hits: 9 },
    task_s: 480,
    outcome: 'landed',
    ...over,
  };
}

const answer = (over: Partial<FeelAnswer> = {}): FeelAnswer => ({
  date_iso: '2026-11-10T10:00:00.000Z',
  build: 'b1',
  assist: 2,
  skill: 1,
  believable: 4,
  satisfying: 5,
  easy: 3,
  comment: 'Rolls well.',
  mission_id: 'm-01',
  ...over,
});

/** A store whose `set` throws the first `failures` times it is called. */
function flaky(failures: number): Kv & { sets: number } {
  const kv = memoryKv();
  let left = failures;
  const out = {
    backend: kv.backend,
    sets: 0,
    get: <T>(key: string) => kv.get<T>(key),
    delete: (key: string) => kv.delete(key),
    set: (key: string, value: unknown) => {
      out.sets += 1;
      if (left > 0) {
        left -= 1;
        return Promise.reject(new Error('quota exceeded'));
      }
      return kv.set(key, value);
    },
  };
  return out;
}

describe('ST2: a result is counted once (AC-65.2)', () => {
  it('aggregating one result five times counts it once', async () => {
    const store = new StatsStore(memoryKv());
    const r = result('m-01');
    const counted: boolean[] = [];
    for (let i = 0; i < 5; i++) counted.push(await store.aggregate(r));
    expect(counted).toEqual([true, false, false, false, false]);
    expect(store.stats.totals).toMatchObject({ missions: 1, kills: 1, sorties: 1, landings: 1 });
    expect(store.stats.seen).toEqual(['m-01']);
  });

  it('a failed write changes nothing, and the retry counts the mission exactly once', async () => {
    const kv = flaky(1);
    const store = new StatsStore(kv);
    const r = result('m-01');
    await expect(store.aggregate(r)).rejects.toThrow('quota exceeded');
    expect(store.stats.totals.missions).toBe(0);
    expect(await kv.get(STATS_KEY)).toBeUndefined();
    expect(await store.aggregate(r)).toBe(true);
    expect(await store.aggregate(r)).toBe(false);
    expect(store.stats.totals.missions).toBe(1);
    // One write per aggregation: the failed one and the one that counted.
    expect(kv.sets).toBe(2);
    // And what was stored reads back the same.
    const again = new StatsStore(kv);
    await again.load();
    expect(again.stats).toEqual(store.stats);
  });

  it('is the same without the seen list: the fault that ignores it is caught', () => {
    const once = applyResult(emptyStats(), result('m-01'));
    const twice = applyResult(once, result('m-01'));
    expect(twice).toBe(once);
    const ignoring = applyResult({ ...once, seen: [] }, result('m-01'));
    expect(ignoring.totals.missions).toBe(2);
  });
});

describe('aggregation', () => {
  it('sums the totals and keeps the bests with their missions', () => {
    let s = emptyStats();
    s = applyResult(s, result('m-01'));
    s = applyResult(
      s,
      result('m-02', {
        sorties: [
          { airborne_s: 100, landed: false, max_alt_m: 9000, max_tas_mps: 400, max_mach: 1.3 },
          { airborne_s: 700, landed: true, max_alt_m: 1000, max_tas_mps: 200, max_mach: 0.6 },
        ],
        task_s: 520,
        combat: { ...zeroCombat(), deaths: 1, missiles_fired: 2, missile_hits: 1 },
      }),
    );
    expect(s.totals).toMatchObject({
      missions: 2,
      sorties: 3,
      landings: 2,
      airborne_s: 1220,
      kills: 1,
      deaths: 1,
      missiles_fired: 2,
      gun_rounds: 120,
    });
    expect(s.bests).toEqual({
      longest_sortie_s: { value: 700, mission_id: 'm-02' },
      max_alt_m: { value: 9000, mission_id: 'm-02' },
      max_tas_mps: { value: 400, mission_id: 'm-02' },
      max_mach: { value: 1.3, mission_id: 'm-02' },
      // The shorter task is the better.
      best_task_s: { value: 480, mission_id: 'm-01' },
    });
    expect(s.log.map((r) => r.mission_id)).toEqual(['m-02', 'm-01']);
  });

  it(`keeps the newest ${String(LOG_LIMIT)} results in the log, and every id`, () => {
    let s = emptyStats();
    for (let i = 0; i < LOG_LIMIT + 5; i++) {
      s = applyResult(
        s,
        result(`m-${String(i).padStart(3, '0')}`, {
          date_iso: `2026-11-${String(10 + (i % 10)).padStart(2, '0')}T00:00:${String(i % 60).padStart(2, '0')}.000Z`,
        }),
      );
    }
    expect(s.log).toHaveLength(LOG_LIMIT);
    expect(s.seen).toHaveLength(LOG_LIMIT + 5);
    expect(s.totals.missions).toBe(LOG_LIMIT + 5);
  });
});

describe('export and import (ST5, AC-65.6)', () => {
  it('round-trips byte for byte through a cleared store', async () => {
    const store = new StatsStore(memoryKv());
    await store.aggregate(result('m-01'));
    await store.aggregate(result('m-02', { mode: 'free', skill: 0, task_s: null }));
    await store.addFeel(answer());
    const text = store.export();
    expect(text.startsWith('{\n  "schema": "stats_export/1",\n  "stats": {')).toBe(true);
    expect(text.endsWith('}\n')).toBe(true);
    await store.clear();
    expect(store.stats).toEqual(emptyStats());
    expect(store.feel.answers).toEqual([]);
    const r = await store.import(text);
    expect(r.ok).toBe(true);
    expect(store.export()).toBe(text);
  });

  it('merges by summing the totals and keeping the better bests', () => {
    const here = applyResult(emptyStats(), result('m-01'));
    const there = applyResult(
      emptyStats(),
      result('m-09', {
        task_s: 300,
        sorties: [{ airborne_s: 60, landed: true, max_alt_m: 100, max_tas_mps: 90, max_mach: 0.3 }],
      }),
    );
    const m = mergeStats(here, there);
    expect(m.totals).toMatchObject({ missions: 2, kills: 2, airborne_s: 480 });
    expect(m.bests.best_task_s).toEqual({ value: 300, mission_id: 'm-09' });
    expect(m.bests.max_alt_m).toEqual({ value: 3200, mission_id: 'm-01' });
    expect(m.seen).toEqual(['m-01', 'm-09']);
  });

  it('adds nothing to the totals from a file whose missions are all counted here already', () => {
    const here = applyResult(emptyStats(), result('m-01'));
    expect(mergeStats(here, here).totals).toEqual(here.totals);
  });

  it('names every value it cannot use, by pointer, and changes nothing', async () => {
    const store = new StatsStore(memoryKv());
    await store.aggregate(result('m-01'));
    const good = JSON.parse(store.export()) as {
      stats: { totals: Record<string, unknown>; log: Record<string, unknown>[] };
      feel: { answers: Record<string, unknown>[] };
    };
    good.stats.totals['kills'] = -1;
    (good.stats.log[0] as Record<string, unknown>)['mode'] = 'campaign';
    good.feel.answers.push({ ...answer(), easy: 6, comment: 'x'.repeat(COMMENT_MAX + 1) });
    const before = store.export();
    const r = await store.import(JSON.stringify(good));
    expect(r.ok).toBe(false);
    if (!r.ok) {
      expect(r.problems.map((p) => p.pointer)).toEqual([
        '/stats/totals/kills',
        '/stats/log/0/mode',
        '/feel/answers/0/easy',
        '/feel/answers/0/comment',
      ]);
    }
    expect(store.export()).toBe(before);
    for (const text of ['', '[]', 'null', '{"schema":"other"}']) {
      expect(importStats(text).ok).toBe(false);
    }
  });

  it('writes the same text however the objects were built (a fixed key order)', () => {
    const s = applyResult(emptyStats(), result('m-01'));
    const shuffled = JSON.parse(JSON.stringify(s), (_k, v: unknown) =>
      typeof v === 'object' && v !== null && !Array.isArray(v)
        ? Object.fromEntries(Object.entries(v).reverse())
        : v,
    ) as typeof s;
    const feel = { schema: 'feel/1' as const, answers: [] };
    expect(exportStats(shuffled, feel)).toBe(exportStats(s, feel));
  });
});

describe('the store', () => {
  it('reads an unusable stored value as empty, and survives a store that fails to read', async () => {
    const kv = memoryKv();
    await kv.set(STATS_KEY, { schema: 'stats/1', totals: 'lots' });
    await kv.set(FEEL_KEY, 42);
    const store = new StatsStore(kv);
    await store.load();
    expect(store.stats).toEqual(emptyStats());
    expect(store.feel.answers).toEqual([]);
    const broken: Kv = { ...kv, get: () => Promise.reject(new Error('blocked')) };
    const b = new StatsStore(broken);
    await b.load();
    expect(b.stats).toEqual(emptyStats());
    expect(readStats(undefined)).toEqual(emptyStats());
  });

  it('keeps the questionnaire answers in order, the comment cut to its limit', async () => {
    const kv = memoryKv();
    const store = new StatsStore(kv);
    await store.addFeel(answer());
    await store.addFeel(answer({ believable: 2, comment: 'y'.repeat(900), mission_id: '' }));
    expect(store.feel.answers.map((a) => a.believable)).toEqual([4, 2]);
    expect(store.feel.answers[1]?.comment).toHaveLength(COMMENT_MAX);
    const again = new StatsStore(kv);
    await again.load();
    expect(again.feel).toEqual(store.feel);
  });

  it('takes the median of answers', () => {
    expect(median([])).toBeUndefined();
    expect(median([4])).toBe(4);
    expect(median([5, 1, 3])).toBe(3);
    expect(median([2, 5, 4, 1])).toBe(3);
  });
});
