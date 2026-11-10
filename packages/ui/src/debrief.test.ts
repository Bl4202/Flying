// @vitest-environment happy-dom
/**
 * The debrief, the feel questionnaire (ST6) and the statistics screen: what they show, that a
 * keyboard can work them (focus, Tab wraps, native controls), and that their DOM is gone once they
 * close (ADR-45 ST4, ST6).
 */
import { describe, expect, it } from 'vitest';
import { memoryKv } from '@flying/platform-web';
import { clock, Debrief, debriefNumbers, debriefRows } from './debrief.ts';
import { FeelSurvey, feelAnswer, type FeelRatings } from './feel-survey.ts';
import { StatisticsScreen } from './statistics-screen.ts';
import { COMMENT_MAX, StatsStore } from './stats/store.ts';
import { zeroCombat, type MissionResult } from './stats/types.ts';

const RESULT: MissionResult = {
  schema: 'mission_result/1',
  mission_id: 'm-20261110-091203-00ab',
  build: 'b1',
  date_iso: '2026-11-10T09:12:03.000Z',
  mode: 'duel',
  assist: 2,
  skill: 2,
  arena: 0,
  duration_s: 655,
  sorties: [
    { airborne_s: 512.4, landed: true, max_alt_m: 3048, max_tas_mps: 257.2, max_mach: 0.812 },
  ],
  combat: {
    ...zeroCombat(),
    kills: 1,
    gun_kills: 1,
    gun_rounds: 140,
    gun_hits: 12,
    missiles_fired: 2,
    missile_hits: 1,
    hits_taken: 3,
  },
  task_s: 601.4,
  outcome: 'landed',
};

function root(): HTMLElement {
  const el = document.createElement('div');
  document.body.append(el);
  return el;
}
const q = (el: HTMLElement, id: string): HTMLElement => {
  const found = el.querySelector<HTMLElement>(`[data-testid="${id}"]`);
  if (!found) throw new Error(`no ${id}`);
  return found;
};
const tab = (el: HTMLElement, shift = false): void => {
  el.dispatchEvent(new KeyboardEvent('keydown', { key: 'Tab', shiftKey: shift, bubbles: true }));
};

describe('the debrief', () => {
  it("shows the mission's numbers", () => {
    const n = debriefNumbers(RESULT);
    expect(n).toMatchObject({
      kills: 1,
      gunHits: 12,
      landings: 1,
      taskS: 601.4,
      outcome: 'landed',
    });
    const rows = Object.fromEntries(debriefRows(n).map((r) => [r.id, r.value]));
    expect(rows).toEqual({
      airborne: '8:32',
      kills: '1 (gun 1, missile 0)',
      gun: '9% (12 of 140 rounds)',
      missiles: '1 hits of 2 fired',
      deaths: '0',
      landings: '1',
      altitude: '10000 ft',
      fastest: '500 kn, Mach 0.81',
    });
    const none = debriefRows(debriefNumbers({ ...RESULT, combat: zeroCombat() }));
    expect(none.find((r) => r.id === 'gun')?.value).toBe('–');
  });

  it('writes times as m:ss, and h:mm:ss from an hour', () => {
    expect([clock(0), clock(59.6), clock(61), clock(3725)]).toEqual([
      '0:00',
      '1:00',
      '1:01',
      '1:02:05',
    ]);
  });

  it('opens with Fly again focused, keeps Tab inside, and is gone when closed', () => {
    const el = root();
    const calls: string[] = [];
    const d = new Debrief(el, {
      onFlyAgain: () => calls.push('again'),
      onRate: () => calls.push('rate'),
      onMenu: () => calls.push('menu'),
    });
    d.open(RESULT);
    expect(d.isOpen).toBe(true);
    expect(document.activeElement).toBe(q(el, 'debrief-fly-again'));
    expect(q(el, 'debrief-task').textContent).toBe('Take-off, kill and landing: 10:01');
    expect(q(el, 'debrief-outcome').textContent).toBe('Landed and stopped.');
    expect(q(el, 'debrief').getAttribute('aria-modal')).toBe('true');
    // From the last button Tab wraps to the first; Shift+Tab from the first to the last.
    q(el, 'debrief-menu').focus();
    tab(q(el, 'debrief-menu'));
    expect(document.activeElement).toBe(q(el, 'debrief-fly-again'));
    tab(q(el, 'debrief-fly-again'), true);
    expect(document.activeElement).toBe(q(el, 'debrief-menu'));
    for (const id of ['debrief-fly-again', 'debrief-rate', 'debrief-menu']) q(el, id).click();
    expect(calls).toEqual(['again', 'rate', 'menu']);
    expect(d.numbers?.missionId).toBe(RESULT.mission_id);
    d.close();
    expect(el.childNodes).toHaveLength(0);
    expect(d.numbers).toBeNull();
  });

  it('says when the task was not done', () => {
    const el = root();
    const d = new Debrief(el, { onFlyAgain: () => 0, onRate: () => 0, onMenu: () => 0 });
    d.open({ ...RESULT, task_s: null, outcome: 'died' });
    expect(q(el, 'debrief-task').textContent).toBe(
      'Take-off, kill and landing: not done this time.',
    );
    expect(q(el, 'debrief-outcome').textContent).toBe('Aircraft lost.');
    d.destroy();
  });
});

describe('ST6: the feel questionnaire', () => {
  function survey() {
    const el = root();
    const saved: FeelRatings[] = [];
    let skipped = 0;
    const s = new FeelSurvey(el, {
      onSave: (r) => saved.push(r),
      onSkip: () => {
        skipped += 1;
      },
    });
    s.open();
    return { el, s, saved, skipped: () => skipped };
  }
  const choose = (el: HTMLElement, id: string, value: number): void => {
    const radio = q(el, `feel-${id}-${String(value)}`) as HTMLInputElement;
    radio.checked = true;
  };

  it('asks three 1-to-5 questions with native radio buttons and an optional comment', () => {
    const { el } = survey();
    for (const id of ['believable', 'satisfying', 'easy']) {
      const radios = q(el, `feel-${id}`).querySelectorAll('input[type="radio"]');
      expect(radios).toHaveLength(5);
    }
    expect(document.activeElement).toBe(q(el, 'feel-believable-1'));
    expect(q(el, 'feel-comment').getAttribute('maxlength')).toBe(String(COMMENT_MAX));
  });

  it('will not save until all three are answered, and says so', () => {
    const { el, saved } = survey();
    choose(el, 'believable', 4);
    q(el, 'feel-save').click();
    expect(saved).toEqual([]);
    expect(q(el, 'feel-missing').textContent).toBe('Please answer all three before saving.');
    expect(document.activeElement).toBe(q(el, 'feel-satisfying-1'));
  });

  it('saves the answers and the comment, kept with the assist, the bandit, the build and the date', async () => {
    const { el, saved } = survey();
    choose(el, 'believable', 4);
    choose(el, 'satisfying', 5);
    choose(el, 'easy', 2);
    (q(el, 'feel-comment') as HTMLTextAreaElement).value = 'The roll felt quick.';
    q(el, 'feel-save').click();
    expect(saved).toEqual([
      { believable: 4, satisfying: 5, easy: 2, comment: 'The roll felt quick.' },
    ]);
    const answer = feelAnswer(saved[0] as FeelRatings, {
      build: 'b1',
      assist: 3,
      skill: 2,
      missionId: RESULT.mission_id,
      wallMs: Date.UTC(2026, 10, 10, 10, 0, 0),
    });
    expect(answer).toEqual({
      date_iso: '2026-11-10T10:00:00.000Z',
      build: 'b1',
      assist: 3,
      skill: 2,
      believable: 4,
      satisfying: 5,
      easy: 2,
      comment: 'The roll felt quick.',
      mission_id: RESULT.mission_id,
    });
    // Stored, and in the statistics' export.
    const store = new StatsStore(memoryKv());
    await store.addFeel(answer);
    expect(store.export()).toContain('"comment": "The roll felt quick."');
    expect(
      feelAnswer(
        { ...answer, comment: 'z'.repeat(800) },
        { build: '', assist: 0, skill: 0, missionId: '' },
      ).comment,
    ).toHaveLength(COMMENT_MAX);
  });

  it('can be skipped, keeps Tab inside (radio groups are one stop), and is gone when closed', () => {
    const { el, s, skipped } = survey();
    choose(el, 'believable', 3);
    q(el, 'feel-believable-3').focus();
    tab(q(el, 'feel-believable-3'), true);
    expect(document.activeElement).toBe(q(el, 'feel-skip'));
    tab(q(el, 'feel-skip'));
    expect(document.activeElement).toBe(q(el, 'feel-believable-1'));
    q(el, 'feel-skip').click();
    expect(skipped()).toBe(1);
    s.close();
    expect(el.childNodes).toHaveLength(0);
  });
});

describe('the statistics screen', () => {
  async function screen() {
    const el = root();
    const store = new StatsStore(memoryKv());
    await store.aggregate(RESULT);
    await store.addFeel(
      feelAnswer(
        { believable: 4, satisfying: 5, easy: 3, comment: '' },
        { build: 'b1', assist: 2, skill: 2, missionId: '' },
      ),
    );
    const saved: [string, string][] = [];
    let back = 0;
    const s = new StatisticsScreen(el, {
      stats: () => store.stats,
      feel: () => store.feel,
      exportText: () => store.export(),
      save: (name, text) => saved.push([name, text]),
      onImport: (text) => store.import(text),
      onClear: () => store.clear(),
      onBack: () => {
        back += 1;
      },
    });
    s.open();
    return { el, s, store, saved, back: () => back };
  }

  it('shows the totals and bests, each with its definition, the log and the ratings', async () => {
    const { el } = await screen();
    expect(q(el, 'stats-total-kills').querySelector('dd')?.textContent).toBe('1');
    expect(q(el, 'stats-total-kills').getAttribute('title')).toMatch(/credited/);
    expect(q(el, 'stats-total-airborne_s').querySelector('dd')?.textContent).toBe('8:32');
    expect(q(el, 'stats-best-max_alt_m').querySelector('dd')?.textContent).toBe('10000 ft');
    expect(q(el, 'stats-best-best_task_s').querySelector('dd')?.textContent).toBe('10:01');
    expect(q(el, 'stats-best-best_task_s').getAttribute('title')).toMatch(/dogfight/);
    expect(q(el, 'stats-log-0').textContent).toBe('2026-11-10: Dogfight, 11 min, 1 kills, landed');
    expect(q(el, 'stats-feel-count').textContent).toBe('Sets of answers: 1');
    expect(q(el, 'stats-feel-median').textContent).toMatch(
      /believable 4, satisfying 5, easy to learn 3/,
    );
  });

  it('exports to a file, and clears only when pressed twice', async () => {
    const { el, store, saved } = await screen();
    q(el, 'stats-export').click();
    expect(saved[0]?.[0]).toBe('flying-stats.json');
    expect(saved[0]?.[1]).toBe(store.export());
    q(el, 'stats-clear').click();
    expect(store.stats.totals.missions).toBe(1);
    expect(q(el, 'stats-clear').textContent).toMatch(/Press again/);
    q(el, 'stats-clear').click();
    await new Promise((r) => setTimeout(r, 0));
    expect(store.stats.totals.missions).toBe(0);
    expect(q(el, 'stats-total-kills').querySelector('dd')?.textContent).toBe('0');
    expect(q(el, 'stats-log-empty').textContent).toBe('No flights yet.');
  });

  it('goes back, and is gone when closed', async () => {
    const { el, s, back } = await screen();
    expect(document.activeElement).toBe(q(el, 'stats-back'));
    q(el, 'stats-back').click();
    expect(back()).toBe(1);
    s.close();
    expect(el.childNodes).toHaveLength(0);
  });
});
