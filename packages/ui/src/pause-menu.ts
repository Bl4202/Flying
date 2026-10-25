/**
 * The pause menu (GDD 53, ADR-37): opened with Esc, or when the pointer lock is lost. It holds the
 * settings a pilot changes between flights and mid-flight: the assist level, where a respawn
 * goes, the mouse, and the g-effects, and leads on to the controls and a respawn.
 *
 * The menu builds its DOM when it opens and removes it when it closes, so it costs nothing in
 * flight. Every control is a native button or slider: Tab moves between them, Enter and Space
 * press them, and the arrows move a slider. Tab wraps inside the menu while it is open.
 */
import { ASSIST_LEVEL, G_EFFECTS, SPAWN_KIND } from '@flying/sim-bridge';
import { h } from './dom.ts';
import { type MouseModeSetting, type Settings } from './settings.ts';
import { t, type StringKey } from './strings.ts';

export type SettingsPatch = Partial<Omit<Settings, 'version'>>;

export interface PauseMenuDeps {
  /** The settings now. */
  settings(): Readonly<Settings>;
  /** The assist level the aircraft is flying under now (0 if there is none). */
  assistInForce?(): number;
  /** The player changed settings. */
  onChange(patch: SettingsPatch): void;
  onResume(): void;
  /** Open the binding editor. */
  onControls(): void;
  onRespawn(): void;
  /** Whether automatic ground-collision avoidance exists; if not, its option is left out. */
  autoGcas?: boolean;
}

interface Choice<V> {
  value: V;
  label: StringKey;
  /** Shown under the group while the choice has focus or the pointer. */
  preview?: StringKey;
}

const ASSISTS: Choice<number>[] = [
  { value: ASSIST_LEVEL.FULL, label: 'assist.full', preview: 'assist.full.preview' },
  { value: ASSIST_LEVEL.STANDARD, label: 'assist.standard', preview: 'assist.standard.preview' },
  { value: ASSIST_LEVEL.REDUCED, label: 'assist.reduced', preview: 'assist.reduced.preview' },
  { value: ASSIST_LEVEL.MANUAL, label: 'assist.manual', preview: 'assist.manual.preview' },
];
const SPAWNS: Choice<number>[] = [
  { value: SPAWN_KIND.RUNWAY, label: 'pause.spawn.runway' },
  { value: SPAWN_KIND.AIR, label: 'pause.spawn.air' },
];
const MICE: Choice<MouseModeSetting>[] = [
  { value: 'off', label: 'pause.mouse.off', preview: 'pause.mouse.off.preview' },
  { value: 'aim', label: 'pause.mouse.aim', preview: 'pause.mouse.aim.preview' },
  { value: 'stick', label: 'pause.mouse.stick', preview: 'pause.mouse.stick.preview' },
];
const G_CHOICES: Choice<number>[] = [
  { value: G_EFFECTS.OFF, label: 'pause.g.off', preview: 'pause.g.off.preview' },
  { value: G_EFFECTS.REDUCED, label: 'pause.g.reduced', preview: 'pause.g.reduced.preview' },
  { value: G_EFFECTS.FULL, label: 'pause.g.full', preview: 'pause.g.full.preview' },
];

export class PauseMenu {
  readonly #root: HTMLElement;
  readonly #d: PauseMenuDeps;
  #panel: HTMLElement | undefined;
  #sync: (() => void)[] = [];
  #onKey: ((e: Event) => void) | undefined;

  constructor(root: HTMLElement, deps: PauseMenuDeps) {
    this.#root = root;
    this.#d = deps;
  }

  get isOpen(): boolean {
    return this.#panel !== undefined;
  }

  /** The panel's element, while the menu is open. */
  get element(): HTMLElement | undefined {
    return this.#panel;
  }

  open(): void {
    if (this.#panel) return;
    this.#panel = this.#build();
    this.#root.append(this.#panel);
    this.#onKey = (e: Event): void => {
      this.#trapTab(e as KeyboardEvent);
    };
    this.#panel.addEventListener('keydown', this.#onKey);
    this.#panel.querySelector<HTMLElement>('[data-testid="pause-resume"]')?.focus();
  }

  close(): void {
    this.#panel?.remove();
    this.#panel = undefined;
    this.#sync = [];
    this.#onKey = undefined;
  }

  destroy(): void {
    this.close();
  }

  /** Re-reads the settings and the assist level in force, for a change made elsewhere. */
  refresh(): void {
    for (const fn of this.#sync) fn();
  }

  #build(): HTMLElement {
    const doc = this.#root.ownerDocument;
    const settings = (): Readonly<Settings> => this.#d.settings();

    const group = <V>(
      id: string,
      title: StringKey,
      choices: Choice<V>[],
      current: () => V,
      pick: (value: V) => void,
      marker?: () => V | undefined,
    ): HTMLElement => {
      const legend = h(doc, 'legend', { text: t(title) });
      const buttons = choices.map((c) =>
        h(doc, 'button', {
          class: 'chip',
          text: t(c.label),
          testid: `pause-${id}-${String(c.value)}`,
          attrs: { type: 'button', 'aria-pressed': 'false' },
        }),
      );
      const preview = h(doc, 'p', { class: 'preview', testid: `pause-${id}-preview` });
      const showPreview = (value: V): void => {
        const key = choices.find((c) => c.value === value)?.preview;
        preview.textContent = key === undefined ? '' : t(key);
      };
      const refresh = (): void => {
        const now = current();
        const flying = marker?.();
        buttons.forEach((b, i) => {
          const choice = choices[i];
          if (choice === undefined) return;
          b.setAttribute('aria-pressed', String(choice.value === now));
          if (marker) b.dataset['flying'] = String(choice.value === flying);
        });
        showPreview(now);
      };
      buttons.forEach((b, i) => {
        const choice = choices[i];
        if (choice === undefined) return;
        b.addEventListener('click', () => {
          pick(choice.value);
          refresh();
        });
        const preview_ = (): void => {
          showPreview(choice.value);
        };
        b.addEventListener('focus', preview_);
        b.addEventListener('mouseenter', preview_);
        b.addEventListener('blur', refresh);
        b.addEventListener('mouseleave', refresh);
      });
      this.#sync.push(refresh);
      refresh();
      return h(
        doc,
        'fieldset',
        { class: 'group', testid: `pause-${id}` },
        legend,
        ...buttons,
        preview,
      );
    };

    const slider = (
      id: string,
      title: StringKey,
      min: number,
      max: number,
      step: number,
      current: () => number,
      pick: (value: number) => void,
      format: (value: number) => string,
    ): HTMLElement => {
      const input = h(doc, 'input', {
        testid: `pause-${id}`,
        attrs: {
          type: 'range',
          min: String(min),
          max: String(max),
          step: String(step),
          'aria-label': t(title),
        },
      });
      const out = h(doc, 'output', { testid: `pause-${id}-value` });
      const refresh = (): void => {
        input.value = String(current());
        out.textContent = format(current());
      };
      input.addEventListener('input', () => {
        const v = Number(input.value);
        if (Number.isFinite(v)) pick(v);
        out.textContent = format(current());
      });
      this.#sync.push(refresh);
      refresh();
      return h(doc, 'label', { class: 'slider' }, h(doc, 'span', { text: t(title) }), input, out);
    };

    const button = (id: string, label: StringKey, action: () => void): HTMLButtonElement => {
      const b = h(doc, 'button', {
        text: t(label),
        testid: id,
        attrs: { type: 'button' },
      });
      b.addEventListener('click', action);
      return b;
    };

    const parts: HTMLElement[] = [
      h(doc, 'h2', { text: t('pause.title'), attrs: { id: 'pause-title' } }),
      button('pause-resume', 'pause.resume', () => {
        this.#d.onResume();
      }),
      group(
        'assist',
        'pause.assist',
        ASSISTS,
        () => settings().assist,
        (assist) => {
          this.#d.onChange({ assist });
        },
        () => {
          const flying = this.#d.assistInForce?.() ?? 0;
          return flying === 0 ? undefined : flying;
        },
      ),
      group(
        'spawn',
        'pause.spawn',
        SPAWNS,
        () => settings().spawn,
        (spawn) => {
          this.#d.onChange({ spawn });
        },
      ),
      group(
        'mouse',
        'pause.mouse',
        MICE,
        () => settings().mouseMode,
        (mouseMode) => {
          this.#d.onChange({ mouseMode });
        },
      ),
      slider(
        'sensitivity',
        'pause.sensitivity',
        0,
        3,
        0.05,
        () => settings().mouseSensitivity,
        (mouseSensitivity) => {
          this.#d.onChange({ mouseSensitivity });
        },
        (v) => `${v.toFixed(2)}×`,
      ),
      slider(
        'spring',
        'pause.spring',
        0,
        1,
        0.05,
        () => settings().mouseSpring,
        (mouseSpring) => {
          this.#d.onChange({ mouseSpring });
        },
        (v) => `${String(Math.round(v * 100))}%`,
      ),
      group(
        'g',
        'pause.g',
        G_CHOICES,
        () => settings().gEffects,
        (gEffects) => {
          this.#d.onChange({ gEffects });
        },
      ),
      slider(
        'strength',
        'pause.strength',
        0,
        100,
        5,
        () => settings().gfxStrength,
        (gfxStrength) => {
          this.#d.onChange({ gfxStrength });
        },
        (v) => `${String(Math.round(v))}%`,
      ),
    ];

    if (this.#d.autoGcas === true) {
      const box = h(doc, 'input', { testid: 'pause-autogcas', attrs: { type: 'checkbox' } });
      const refresh = (): void => {
        box.checked = settings().autoGcas;
      };
      box.addEventListener('change', () => {
        this.#d.onChange({ autoGcas: box.checked });
      });
      this.#sync.push(refresh);
      refresh();
      parts.push(
        h(doc, 'label', { class: 'check' }, box, h(doc, 'span', { text: t('pause.autogcas') })),
      );
    }

    parts.push(
      h(
        doc,
        'div',
        { class: 'actions' },
        button('pause-controls', 'pause.controls', () => {
          this.#d.onControls();
        }),
        button('pause-respawn', 'pause.respawn', () => {
          this.#d.onRespawn();
        }),
      ),
    );

    return h(
      doc,
      'section',
      {
        class: 'pause-menu',
        testid: 'pause-menu',
        attrs: { role: 'dialog', 'aria-modal': 'true', 'aria-labelledby': 'pause-title' },
      },
      ...parts,
    );
  }

  /** Keeps Tab inside the menu: past the last control it goes to the first, and back. */
  #trapTab(e: KeyboardEvent): void {
    if (e.key !== 'Tab' || !this.#panel) return;
    const items = [
      ...this.#panel.querySelectorAll<HTMLElement>(
        'button, input, [tabindex]:not([tabindex="-1"])',
      ),
    ].filter((el) => !el.hasAttribute('disabled'));
    const first = items[0];
    const last = items.at(-1);
    if (first === undefined || last === undefined) return;
    const active = this.#root.ownerDocument.activeElement;
    if (e.shiftKey && active === first) {
      e.preventDefault();
      last.focus();
    } else if (!e.shiftKey && active === last) {
      e.preventDefault();
      first.focus();
    }
  }
}
