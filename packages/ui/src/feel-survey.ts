/**
 * The feel questionnaire (GDD 66.4, ADR-45 G1-owner, ADR-49): three statements rated from 1
 * (strongly disagree) to 5 (strongly agree), and an optional comment. The answers are kept on this
 * device with the assist level, the bandit, the build and the date, and go out with the statistics'
 * export; nothing is sent anywhere.
 *
 * Built when it opens and removed when it closes. The ratings are native radio buttons (Tab reaches
 * a group, the arrows choose in it), the comment a text area; Save asks for all three ratings.
 */
import { h } from './dom.ts';
import { trapTab } from './form.ts';
import { COMMENT_MAX, type FeelAnswer } from './stats/store.ts';
import { t, type StringKey } from './strings.ts';

export interface FeelRatings {
  believable: number;
  satisfying: number;
  easy: number;
  comment: string;
}

/** What the answers are kept with: the flight they rate. */
export interface FeelContext {
  build: string;
  /** `ASSIST_LEVEL` flown, and the bandit (0 in free flight). */
  assist: number;
  skill: number;
  /** The mission rated, or '' if none. */
  missionId: string;
  /** When, ms since 1970 (default: now). */
  wallMs?: number;
}

/** The ratings with the flight they rate, as kept in `feel/1`. */
export function feelAnswer(r: FeelRatings, c: FeelContext): FeelAnswer {
  return {
    date_iso: new Date(c.wallMs ?? Date.now()).toISOString(),
    build: c.build,
    assist: c.assist,
    skill: c.skill,
    believable: r.believable,
    satisfying: r.satisfying,
    easy: r.easy,
    comment: r.comment.slice(0, COMMENT_MAX),
    mission_id: c.missionId,
  };
}

export interface FeelSurveyDeps {
  onSave(ratings: FeelRatings): void;
  onSkip(): void;
}

const QUESTIONS: readonly ['believable' | 'satisfying' | 'easy', StringKey][] = [
  ['believable', 'feel.believable'],
  ['satisfying', 'feel.satisfying'],
  ['easy', 'feel.easy'],
];
const SCALE: readonly StringKey[] = [
  'feel.scale.1',
  'feel.scale.2',
  'feel.scale.3',
  'feel.scale.4',
  'feel.scale.5',
];

export class FeelSurvey {
  readonly #root: HTMLElement;
  readonly #d: FeelSurveyDeps;
  #panel: HTMLElement | undefined;

  constructor(root: HTMLElement, deps: FeelSurveyDeps) {
    this.#root = root;
    this.#d = deps;
  }

  get isOpen(): boolean {
    return this.#panel !== undefined;
  }

  open(): void {
    this.close();
    const doc = this.#root.ownerDocument;
    const groups = QUESTIONS.map(([id, question]) => {
      const radios = SCALE.map((label, i) => {
        const radio = h(doc, 'input', {
          testid: `feel-${id}-${String(i + 1)}`,
          attrs: { type: 'radio', name: `feel-${id}`, value: String(i + 1) },
        });
        return h(doc, 'label', { class: 'scale' }, radio, h(doc, 'span', { text: t(label) }));
      });
      return h(
        doc,
        'fieldset',
        { class: 'group', testid: `feel-${id}` },
        h(doc, 'legend', { text: t(question) }),
        ...radios,
      );
    });
    const comment = h(doc, 'textarea', {
      testid: 'feel-comment',
      attrs: { maxlength: String(COMMENT_MAX), rows: '3', 'aria-label': t('feel.comment') },
    });
    const missing = h(doc, 'p', {
      class: 'note',
      testid: 'feel-missing',
      attrs: { role: 'status', 'aria-live': 'polite' },
    });
    const picked = (id: string): number => {
      const on = this.#panel?.querySelector<HTMLInputElement>(`input[name="feel-${id}"]:checked`);
      return on === null || on === undefined ? 0 : Number(on.value);
    };
    const button = (id: string, label: StringKey, run: () => void): HTMLButtonElement => {
      const b = h(doc, 'button', {
        text: t(label),
        testid: `feel-${id}`,
        attrs: { type: 'button' },
      });
      b.addEventListener('click', run);
      return b;
    };
    const save = button('save', 'feel.save', () => {
      const [believable, satisfying, easy] = QUESTIONS.map(([id]) => picked(id));
      const first = QUESTIONS.findIndex(([id]) => picked(id) === 0);
      if (first >= 0) {
        missing.textContent = t('feel.missing');
        groups[first]?.querySelector<HTMLInputElement>('input')?.focus();
        return;
      }
      this.#d.onSave({
        believable: believable ?? 0,
        satisfying: satisfying ?? 0,
        easy: easy ?? 0,
        comment: comment.value.slice(0, COMMENT_MAX),
      });
    });
    save.classList.add('primary');
    const panel = h(
      doc,
      'section',
      {
        class: 'panel-screen feel-survey',
        testid: 'feel-survey',
        attrs: { role: 'dialog', 'aria-modal': 'true', 'aria-labelledby': 'feel-title' },
      },
      h(doc, 'h1', { text: t('feel.title'), attrs: { id: 'feel-title' } }),
      h(doc, 'p', { class: 'tagline', text: t('feel.intro') }),
      ...groups,
      h(doc, 'label', { class: 'comment' }, h(doc, 'span', { text: t('feel.comment') }), comment),
      missing,
      h(
        doc,
        'div',
        { class: 'actions' },
        save,
        button('skip', 'feel.skip', () => {
          this.#d.onSkip();
        }),
      ),
    );
    panel.addEventListener('keydown', (e) => {
      trapTab(panel, doc, e);
    });
    this.#panel = panel;
    this.#root.append(panel);
    groups[0]?.querySelector<HTMLInputElement>('input')?.focus();
  }

  close(): void {
    this.#panel?.remove();
    this.#panel = undefined;
  }

  destroy(): void {
    this.close();
  }
}
