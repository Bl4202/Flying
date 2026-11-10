/**
 * The main menu (GDD 53, ADR-44, ADR-49): where a visit starts after the first run. Fly begins a
 * sortie in the chosen arena under the chosen flight assist, in free flight or a dogfight against the
 * chosen bandit (the bandit is offered only for a dogfight); Settings opens the settings screen and
 * Statistics the statistics. When the
 * browser offers to install the game, a new build is waiting, or the browser is keeping sound off
 * until the player does something, a row for it appears here and nowhere else, so nothing of the
 * kind is ever put over a flight.
 *
 * The menu builds its DOM when it opens and removes it when it closes. Every control is a native
 * button: Tab moves between them (and wraps), Enter and Space press them.
 */
import { ARENA, ASSIST_LEVEL, SCENARIO } from '@flying/sim-bridge';
import { h } from './dom.ts';
import { Form, trapTab, type Choice } from './form.ts';
import type { Settings, SettingsPatch } from './settings.ts';
import { t } from './strings.ts';

/** A row that exists only while `available()` is true. */
export interface OptionalRow {
  available(): boolean;
  run(): void;
}

export interface MainMenuDeps {
  settings(): Readonly<Settings>;
  onChange(patch: SettingsPatch): void;
  onFly(): void;
  onSettings(): void;
  onStatistics(): void;
  /** The build, for the footer. */
  build: string;
  /** Offered where the browser can install the game. */
  install?: OptionalRow;
  /** Offered while a new build waits to take over. */
  update?: OptionalRow;
  /** Offered while the browser keeps sound off until the player acts. */
  audio?: OptionalRow;
}

const ARENAS: Choice<number>[] = [
  { value: ARENA.RUGGED, label: 'menu.arena.rugged', preview: 'menu.arena.rugged.preview' },
  { value: ARENA.PLAIN, label: 'menu.arena.plain', preview: 'menu.arena.plain.preview' },
];
const MODES: Choice<number>[] = [
  { value: SCENARIO.FREE, label: 'menu.mode.free', preview: 'menu.mode.free.preview' },
  { value: SCENARIO.DUEL, label: 'menu.mode.duel', preview: 'menu.mode.duel.preview' },
];
const BANDITS: Choice<number>[] = [
  { value: 1, label: 'menu.bandit.1', preview: 'menu.bandit.1.preview' },
  { value: 2, label: 'menu.bandit.2', preview: 'menu.bandit.2.preview' },
  { value: 3, label: 'menu.bandit.3', preview: 'menu.bandit.3.preview' },
];
const ASSISTS: Choice<number>[] = [
  { value: ASSIST_LEVEL.FULL, label: 'assist.full', preview: 'assist.full.preview' },
  { value: ASSIST_LEVEL.STANDARD, label: 'assist.standard', preview: 'assist.standard.preview' },
  { value: ASSIST_LEVEL.REDUCED, label: 'assist.reduced', preview: 'assist.reduced.preview' },
  { value: ASSIST_LEVEL.MANUAL, label: 'assist.manual', preview: 'assist.manual.preview' },
];

export class MainMenu {
  readonly #root: HTMLElement;
  readonly #d: MainMenuDeps;
  #panel: HTMLElement | undefined;
  #form: Form | undefined;
  #rows: { row: HTMLElement; source: OptionalRow }[] = [];
  /** The bandit's choice, shown only for a dogfight. */
  #bandit: HTMLElement | undefined;

  constructor(root: HTMLElement, deps: MainMenuDeps) {
    this.#root = root;
    this.#d = deps;
  }

  get isOpen(): boolean {
    return this.#panel !== undefined;
  }

  get element(): HTMLElement | undefined {
    return this.#panel;
  }

  open(): void {
    if (this.#panel) return;
    const panel = this.#build();
    this.#panel = panel;
    this.#root.append(panel);
    panel.addEventListener('keydown', (e) => {
      trapTab(panel, this.#root.ownerDocument, e);
    });
    panel.querySelector<HTMLElement>('[data-testid="menu-fly"]')?.focus();
  }

  close(): void {
    this.#panel?.remove();
    this.#panel = undefined;
    this.#form = undefined;
    this.#rows = [];
    this.#bandit = undefined;
  }

  destroy(): void {
    this.close();
  }

  /** Re-reads the settings and which optional rows apply, for a change made elsewhere. */
  refresh(): void {
    this.#form?.refresh();
    for (const { row, source } of this.#rows) row.hidden = !source.available();
    if (this.#bandit) this.#bandit.hidden = this.#d.settings().scenario === SCENARIO.FREE;
  }

  #build(): HTMLElement {
    const doc = this.#root.ownerDocument;
    const form = new Form(doc, 'menu');
    this.#form = form;
    const settings = (): Readonly<Settings> => this.#d.settings();

    const fly = form.button('fly', 'menu.fly', () => {
      this.#d.onFly();
    });
    fly.classList.add('primary');

    const optional = (
      id: string,
      source: OptionalRow | undefined,
      label: 'menu.install' | 'menu.update' | 'menu.audio-on',
      lead?: 'menu.audio-off',
    ): HTMLElement | undefined => {
      if (source === undefined) return undefined;
      const children: (Node | string)[] = [];
      if (lead !== undefined) children.push(h(doc, 'span', { text: t(lead) }));
      children.push(
        form.button(id, label, () => {
          source.run();
          this.refresh();
        }),
      );
      const row = h(doc, 'p', { class: 'row', testid: `menu-${id}-row` }, ...children);
      row.hidden = !source.available();
      this.#rows.push({ row, source });
      return row;
    };

    const bandit = form.group(
      'bandit',
      'menu.bandit',
      BANDITS,
      () => settings().banditSkill,
      (banditSkill) => {
        this.#d.onChange({ banditSkill });
      },
    );
    bandit.hidden = settings().scenario === SCENARIO.FREE;
    this.#bandit = bandit;

    const rows = [
      optional('update', this.#d.update, 'menu.update'),
      optional('install', this.#d.install, 'menu.install'),
      optional('audio', this.#d.audio, 'menu.audio-on', 'menu.audio-off'),
    ].filter((r): r is HTMLElement => r !== undefined);

    return h(
      doc,
      'section',
      {
        class: 'panel-screen main-menu',
        testid: 'main-menu',
        attrs: { role: 'dialog', 'aria-modal': 'true', 'aria-labelledby': 'main-menu-title' },
      },
      h(doc, 'h1', { text: t('menu.title'), attrs: { id: 'main-menu-title' } }),
      h(doc, 'p', { class: 'tagline', text: t('menu.tagline') }),
      ...rows,
      fly,
      form.group(
        'arena',
        'menu.arena',
        ARENAS,
        () => settings().arena,
        (arena) => {
          this.#d.onChange({ arena });
        },
      ),
      form.group(
        'mode',
        'menu.mode',
        MODES,
        () => settings().scenario,
        (scenario) => {
          this.#d.onChange({ scenario });
          this.refresh();
        },
      ),
      bandit,
      form.group(
        'assist',
        'menu.assist',
        ASSISTS,
        () => settings().assist,
        (assist) => {
          this.#d.onChange({ assist });
        },
      ),
      h(
        doc,
        'div',
        { class: 'actions' },
        form.button('settings', 'menu.settings', () => {
          this.#d.onSettings();
        }),
        form.button('statistics', 'menu.statistics', () => {
          this.#d.onStatistics();
        }),
      ),
      h(doc, 'p', {
        class: 'note',
        testid: 'menu-version',
        text: t('menu.version', { build: this.#d.build.slice(0, 8) }),
      }),
    );
  }
}
