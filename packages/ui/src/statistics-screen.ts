/**
 * The statistics screen (GDD 65.6, 65.7, ADR-49), opened from the main menu: the totals and the
 * personal bests (each with its definition as a tooltip, GDD 65.2), the last ten missions, the feel
 * answers (how many, and their medians), and Export, Import, Clear and Back.
 *
 * Built when it opens and removed when it closes; it rebuilds its figures after an import or a
 * clear. Clear asks to be pressed a second time before it forgets anything.
 */
import { clock } from './debrief.ts';
import { h } from './dom.ts';
import { trapTab } from './form.ts';
import {
  median,
  STATS_FILE_NAME,
  type FeelData,
  type StatsBests,
  type StatsData,
  type StatsImportResult,
  type StatsTotals,
} from './stats/store.ts';
import type { MissionResult } from './stats/types.ts';
import { t, type StringKey } from './strings.ts';

export interface StatisticsDeps {
  stats(): Readonly<StatsData>;
  feel(): Readonly<FeelData>;
  /** The export file's text. */
  exportText(): string;
  /** Gives the player a file to keep. */
  save(name: string, text: string): void;
  onImport(text: string): Promise<StatsImportResult>;
  onClear(): Promise<void>;
  onBack(): void;
  readFile?(file: File): Promise<string>;
}

const FT_PER_M = 3.280_84;
const KN_PER_MPS = 1.943_844;

/** Each total: its label and its definition (GDD 65.2), and how it is written. */
const TOTALS: readonly [keyof StatsTotals, StringKey, StringKey, (v: number) => string][] = [
  ['missions', 'stats.missions', 'stats.missions.def', String],
  ['sorties', 'stats.sorties', 'stats.sorties.def', String],
  ['landings', 'stats.landings', 'stats.landings.def', String],
  ['airborne_s', 'stats.airborne', 'stats.airborne.def', clock],
  ['kills', 'stats.kills', 'stats.kills.def', String],
  ['gun_kills', 'stats.gun-kills', 'stats.gun-kills.def', String],
  ['missile_kills', 'stats.missile-kills', 'stats.missile-kills.def', String],
  ['deaths', 'stats.deaths', 'stats.deaths.def', String],
  ['missiles_fired', 'stats.missiles-fired', 'stats.missiles-fired.def', String],
  ['missile_hits', 'stats.missile-hits', 'stats.missile-hits.def', String],
  ['gun_rounds', 'stats.gun-rounds', 'stats.gun-rounds.def', String],
  ['gun_hits', 'stats.gun-hits', 'stats.gun-hits.def', String],
  ['hits_taken', 'stats.hits-taken', 'stats.hits-taken.def', String],
];

const BESTS: readonly [keyof StatsBests, StringKey, StringKey, (v: number) => string][] = [
  ['longest_sortie_s', 'stats.best.longest', 'stats.best.longest.def', clock],
  [
    'max_alt_m',
    'stats.best.altitude',
    'stats.best.altitude.def',
    (v) => t('debrief.altitude.value', { ft: Math.round(v * FT_PER_M) }),
  ],
  [
    'max_tas_mps',
    'stats.best.speed',
    'stats.best.speed.def',
    (v) => t('stats.best.speed.value', { kn: Math.round(v * KN_PER_MPS) }),
  ],
  ['max_mach', 'stats.best.mach', 'stats.best.mach.def', (v) => v.toFixed(2)],
  ['best_task_s', 'stats.best.task', 'stats.best.task.def', clock],
];

const MODES: Record<MissionResult['mode'], StringKey> = {
  free: 'menu.mode.free',
  duel: 'menu.mode.duel',
};
const OUTCOMES: Record<MissionResult['outcome'], StringKey> = {
  landed: 'stats.outcome.landed',
  ended: 'stats.outcome.ended',
  died: 'stats.outcome.died',
};

/** How many missions the log shows. */
export const LOG_SHOWN = 10;

export class StatisticsScreen {
  readonly #root: HTMLElement;
  readonly #d: StatisticsDeps;
  #panel: HTMLElement | undefined;
  #figures: HTMLElement | undefined;

  constructor(root: HTMLElement, deps: StatisticsDeps) {
    this.#root = root;
    this.#d = deps;
  }

  get isOpen(): boolean {
    return this.#panel !== undefined;
  }

  open(): void {
    if (this.#panel) return;
    const doc = this.#root.ownerDocument;
    const status = h(doc, 'p', {
      class: 'note',
      testid: 'stats-status',
      attrs: { role: 'status', 'aria-live': 'polite' },
    });
    const problems = h(doc, 'ul', { class: 'problems', testid: 'stats-problems' });
    const button = (id: string, label: StringKey, run: () => void): HTMLButtonElement => {
      const b = h(doc, 'button', {
        text: t(label),
        testid: `stats-${id}`,
        attrs: { type: 'button' },
      });
      b.addEventListener('click', run);
      return b;
    };

    const exportButton = button('export', 'stats.export', () => {
      this.#d.save(STATS_FILE_NAME, this.#d.exportText());
      status.textContent = t('stats.exported', { name: STATS_FILE_NAME });
    });
    const file = h(doc, 'input', {
      testid: 'stats-import',
      attrs: { type: 'file', accept: 'application/json,.json', 'aria-label': t('stats.import') },
    });
    file.addEventListener('change', () => {
      const chosen = file.files?.[0];
      if (chosen === undefined) return;
      void (this.#d.readFile?.(chosen) ?? chosen.text())
        .then((text) => this.#d.onImport(text))
        .then((result) => {
          problems.replaceChildren();
          if (result.ok) {
            status.textContent = t('stats.imported');
            this.refresh();
          } else {
            status.textContent = '';
            for (const p of result.problems) {
              problems.append(
                h(doc, 'li', {
                  text: t('stats.import-failed', {
                    problem: p.pointer === '' ? p.message : `${p.pointer}: ${p.message}`,
                  }),
                }),
              );
            }
          }
        });
    });
    const clear = button('clear', 'stats.clear', () => {
      if (clear.dataset['confirming'] !== 'true') {
        clear.dataset['confirming'] = 'true';
        clear.textContent = t('stats.clear.confirm');
        return;
      }
      delete clear.dataset['confirming'];
      clear.textContent = t('stats.clear');
      void this.#d.onClear().then(() => {
        status.textContent = t('stats.cleared');
        this.refresh();
      });
    });
    clear.addEventListener('blur', () => {
      delete clear.dataset['confirming'];
      clear.textContent = t('stats.clear');
    });
    const back = button('back', 'stats.back', () => {
      this.#d.onBack();
    });

    const figures = h(doc, 'div', { class: 'stats-figures', testid: 'stats-figures' });
    this.#figures = figures;
    const panel = h(
      doc,
      'section',
      {
        class: 'panel-screen statistics',
        testid: 'statistics',
        attrs: { role: 'dialog', 'aria-modal': 'true', 'aria-labelledby': 'stats-title' },
      },
      h(doc, 'h1', { text: t('stats.title'), attrs: { id: 'stats-title' } }),
      figures,
      h(doc, 'div', { class: 'actions' }, exportButton),
      h(doc, 'label', { class: 'file' }, h(doc, 'span', { text: t('stats.import') }), file),
      problems,
      h(doc, 'div', { class: 'actions' }, clear, back),
      status,
    );
    panel.addEventListener('keydown', (e) => {
      trapTab(panel, doc, e);
    });
    this.#panel = panel;
    this.#root.append(panel);
    this.refresh();
    back.focus();
  }

  /** Rebuilds the figures from the store. */
  refresh(): void {
    const figures = this.#figures;
    if (figures === undefined) return;
    const doc = this.#root.ownerDocument;
    const s = this.#d.stats();
    const row = (id: string, label: StringKey, def: StringKey, value: string): HTMLElement =>
      h(
        doc,
        'div',
        { class: 'row', testid: id, attrs: { title: t(def) } },
        h(doc, 'dt', { text: t(label) }),
        h(doc, 'dd', { text: value }),
      );
    const totals = TOTALS.map(([k, label, def, write]) =>
      row(`stats-total-${k}`, label, def, write(s.totals[k])),
    );
    const bests = BESTS.map(([k, label, def, write]) => {
      const b = s.bests[k];
      return row(`stats-best-${k}`, label, def, b === null ? t('debrief.none') : write(b.value));
    });
    const log =
      s.log.length === 0
        ? [h(doc, 'li', { testid: 'stats-log-empty', text: t('stats.empty') })]
        : s.log.slice(0, LOG_SHOWN).map((r, i) =>
            h(doc, 'li', {
              testid: `stats-log-${String(i)}`,
              text: t('stats.log.entry', {
                date: r.date_iso.slice(0, 10),
                mode: t(MODES[r.mode]),
                minutes: Math.round(r.duration_s / 60),
                kills: r.combat.kills,
                outcome: t(OUTCOMES[r.outcome]),
              }),
            }),
          );
    const answers = this.#d.feel().answers;
    const m = (pick: (a: (typeof answers)[number]) => number): string => {
      const v = median(answers.map(pick));
      return v === undefined ? t('debrief.none') : String(v);
    };
    figures.replaceChildren(
      h(doc, 'h2', { text: t('stats.totals') }),
      h(doc, 'dl', { class: 'stats-rows' }, ...totals),
      h(doc, 'h2', { text: t('stats.bests') }),
      h(doc, 'dl', { class: 'stats-rows' }, ...bests),
      h(doc, 'h2', { text: t('stats.log') }),
      h(doc, 'ol', { class: 'stats-log', testid: 'stats-log' }, ...log),
      h(doc, 'h2', { text: t('stats.feel') }),
      h(doc, 'p', {
        testid: 'stats-feel-count',
        text: t('stats.feel.count', { n: answers.length }),
      }),
      h(doc, 'p', {
        testid: 'stats-feel-median',
        text: t('stats.feel.median', {
          believable: m((a) => a.believable),
          satisfying: m((a) => a.satisfying),
          easy: m((a) => a.easy),
        }),
      }),
    );
  }

  close(): void {
    this.#panel?.remove();
    this.#panel = undefined;
    this.#figures = undefined;
  }

  destroy(): void {
    this.close();
  }
}
