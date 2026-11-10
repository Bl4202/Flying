/**
 * The debrief (GDD 53.3, 65.7, ADR-49): what a mission came to, shown when a duel's sortie ends on
 * the runway or the player ends it from the pause menu. It holds the simulation like the other
 * menus. Fly again starts the same mission afresh; Rate this flight opens the feel questionnaire;
 * Main menu leaves.
 *
 * Built when it opens and removed when it closes. Every control is a native button: Tab moves
 * between them (and wraps), Enter and Space press them, and Fly again has the focus to begin with.
 */
import { h } from './dom.ts';
import { trapTab } from './form.ts';
import { t, type StringKey } from './strings.ts';
import type { MissionResult } from './stats/types.ts';

export interface DebriefDeps {
  onFlyAgain(): void;
  onRate(): void;
  onMenu(): void;
}

/** The debrief's numbers, as shown (for tests: `window.__flying.debrief()`). */
export interface DebriefNumbers {
  missionId: string;
  outcome: MissionResult['outcome'];
  airborneS: number;
  kills: number;
  gunKills: number;
  missileKills: number;
  gunRounds: number;
  gunHits: number;
  missilesFired: number;
  missileHits: number;
  deaths: number;
  hitsTaken: number;
  landings: number;
  maxAltM: number;
  maxTasMps: number;
  maxMach: number;
  taskS: number | null;
}

const FT_PER_M = 3.280_84;
const KN_PER_MPS = 1.943_844;

const OUTCOMES: Record<MissionResult['outcome'], StringKey> = {
  landed: 'debrief.outcome.landed',
  ended: 'debrief.outcome.ended',
  died: 'debrief.outcome.died',
};

/** `m:ss` (or `h:mm:ss` from an hour) of whole seconds. */
export function clock(seconds: number): string {
  const s = Math.max(Math.round(seconds), 0);
  const ss = String(s % 60).padStart(2, '0');
  const m = Math.floor(s / 60);
  if (m < 60) return `${String(m)}:${ss}`;
  return `${String(Math.floor(m / 60))}:${String(m % 60).padStart(2, '0')}:${ss}`;
}

/** The numbers of a result, over all its sorties. */
export function debriefNumbers(r: MissionResult): DebriefNumbers {
  const c = r.combat;
  const max = (pick: (s: MissionResult['sorties'][number]) => number): number =>
    r.sorties.reduce((m, s) => Math.max(m, pick(s)), 0);
  return {
    missionId: r.mission_id,
    outcome: r.outcome,
    airborneS: r.sorties.reduce((sum, s) => sum + s.airborne_s, 0),
    kills: c.kills,
    gunKills: c.gun_kills,
    missileKills: c.missile_kills,
    gunRounds: c.gun_rounds,
    gunHits: c.gun_hits,
    missilesFired: c.missiles_fired,
    missileHits: c.missile_hits,
    deaths: c.deaths,
    hitsTaken: c.hits_taken,
    landings: r.sorties.filter((s) => s.landed).length,
    maxAltM: max((s) => s.max_alt_m),
    maxTasMps: max((s) => s.max_tas_mps),
    maxMach: max((s) => s.max_mach),
    taskS: r.task_s,
  };
}

/** Each row of the debrief: its id, its label and its value in words. */
export function debriefRows(n: DebriefNumbers): { id: string; label: string; value: string }[] {
  const accuracy =
    n.gunRounds === 0
      ? t('debrief.none')
      : t('debrief.gun.value', {
          pct: Math.round((100 * n.gunHits) / n.gunRounds),
          hits: n.gunHits,
          rounds: n.gunRounds,
        });
  return [
    { id: 'airborne', label: t('debrief.airborne'), value: clock(n.airborneS) },
    {
      id: 'kills',
      label: t('debrief.kills'),
      value: t('debrief.kills.value', { total: n.kills, gun: n.gunKills, missile: n.missileKills }),
    },
    { id: 'gun', label: t('debrief.gun'), value: accuracy },
    {
      id: 'missiles',
      label: t('debrief.missiles'),
      value: t('debrief.missiles.value', { hits: n.missileHits, fired: n.missilesFired }),
    },
    { id: 'deaths', label: t('debrief.deaths'), value: String(n.deaths) },
    { id: 'landings', label: t('debrief.landings'), value: String(n.landings) },
    {
      id: 'altitude',
      label: t('debrief.altitude'),
      value: t('debrief.altitude.value', { ft: Math.round(n.maxAltM * FT_PER_M) }),
    },
    {
      id: 'fastest',
      label: t('debrief.fastest'),
      value: t('debrief.fastest.value', {
        kn: Math.round(n.maxTasMps * KN_PER_MPS),
        mach: n.maxMach.toFixed(2),
      }),
    },
  ];
}

export class Debrief {
  readonly #root: HTMLElement;
  readonly #d: DebriefDeps;
  #panel: HTMLElement | undefined;
  #numbers: DebriefNumbers | null = null;

  constructor(root: HTMLElement, deps: DebriefDeps) {
    this.#root = root;
    this.#d = deps;
  }

  get isOpen(): boolean {
    return this.#panel !== undefined;
  }

  /** What the open debrief shows, or null. */
  get numbers(): DebriefNumbers | null {
    return this.#numbers;
  }

  open(result: MissionResult): void {
    this.close();
    const n = debriefNumbers(result);
    this.#numbers = n;
    const doc = this.#root.ownerDocument;
    const button = (id: string, label: StringKey, run: () => void): HTMLButtonElement => {
      const b = h(doc, 'button', {
        text: t(label),
        testid: `debrief-${id}`,
        attrs: { type: 'button' },
      });
      b.addEventListener('click', run);
      return b;
    };
    const again = button('fly-again', 'debrief.fly-again', () => {
      this.#d.onFlyAgain();
    });
    again.classList.add('primary');
    const rows = debriefRows(n).map((r) =>
      h(
        doc,
        'div',
        { class: 'row', testid: `debrief-row-${r.id}` },
        h(doc, 'dt', { text: r.label }),
        h(doc, 'dd', { text: r.value }),
      ),
    );
    const taskS = n.taskS === null ? 0 : Math.round(n.taskS);
    const task =
      n.taskS === null
        ? t('debrief.task-none')
        : t('debrief.task', {
            mm: String(Math.floor(taskS / 60)).padStart(2, '0'),
            ss: String(taskS % 60).padStart(2, '0'),
          });
    const panel = h(
      doc,
      'section',
      {
        class: 'panel-screen debrief',
        testid: 'debrief',
        attrs: { role: 'dialog', 'aria-modal': 'true', 'aria-labelledby': 'debrief-title' },
      },
      h(doc, 'h1', { text: t('debrief.title'), attrs: { id: 'debrief-title' } }),
      h(doc, 'p', { class: 'tagline', testid: 'debrief-outcome', text: t(OUTCOMES[n.outcome]) }),
      h(doc, 'dl', { class: 'debrief-rows' }, ...rows),
      h(doc, 'p', { class: 'note', testid: 'debrief-task', text: task }),
      h(
        doc,
        'div',
        { class: 'actions' },
        again,
        button('rate', 'debrief.rate', () => {
          this.#d.onRate();
        }),
        button('menu', 'debrief.menu', () => {
          this.#d.onMenu();
        }),
      ),
    );
    panel.addEventListener('keydown', (e) => {
      trapTab(panel, doc, e);
    });
    this.#panel = panel;
    this.#root.append(panel);
    again.focus();
  }

  close(): void {
    this.#panel?.remove();
    this.#panel = undefined;
    this.#numbers = null;
  }

  destroy(): void {
    this.close();
  }
}
