/**
 * The debug overlay (GDD 63): live numbers from every part of the app, refreshed four times a
 * second while it is open, toggled with Ctrl+Shift+D. It builds its DOM the first time it opens
 * and does nothing while closed. Rows are single text nodes, so the whole panel stays well under
 * the 150-node budget of AC-53.4.
 */
import { h } from './dom.ts';
import {
  buildOverlayRows,
  formatHashReport,
  type OverlayModel,
  type OverlayRow,
} from './overlay-rows.ts';
import { t } from './strings.ts';

export const OVERLAY_REFRESH_MS = 250;
const LABEL_WIDTH = 13;

export interface OverlayActions {
  hash(): Promise<{ tick: number; hashes: Readonly<Record<string, string>> }>;
  selfTest(): Promise<{ ok: boolean; text: string }>;
  /** Absent until the benchmark exists; the button is not shown without it. */
  benchmark?: () => Promise<string>;
  togglePause(): void;
}

export interface IntervalTimers {
  setInterval(callback: () => void, ms: number): number;
  clearInterval(id: number): void;
}

const BROWSER_INTERVALS: IntervalTimers = {
  setInterval: (cb, ms) => window.setInterval(cb, ms),
  clearInterval: (id) => {
    window.clearInterval(id);
  },
};

export interface DebugOverlayDeps {
  model(): OverlayModel;
  actions: OverlayActions;
  /** Called when the overlay opens or closes (the app turns stage profiling on and off). */
  onVisibleChange?: (visible: boolean) => void;
  timers?: IntervalTimers;
}

export class DebugOverlay {
  readonly #root: HTMLElement;
  readonly #d: DebugOverlayDeps;
  readonly #timers: IntervalTimers;
  readonly #rowEls = new Map<string, HTMLElement>();
  #panel: HTMLElement | undefined;
  #rowsEl: HTMLElement | undefined;
  #pauseButton: HTMLButtonElement | undefined;
  #interval: number | undefined;
  #visible = false;

  constructor(root: HTMLElement, deps: DebugOverlayDeps) {
    this.#root = root;
    this.#d = deps;
    this.#timers = deps.timers ?? BROWSER_INTERVALS;
  }

  get visible(): boolean {
    return this.#visible;
  }

  show(): void {
    if (this.#visible) return;
    this.#visible = true;
    const panel = (this.#panel ??= this.#build());
    panel.hidden = false;
    this.refresh();
    this.#interval = this.#timers.setInterval(() => {
      this.refresh();
    }, OVERLAY_REFRESH_MS);
    this.#d.onVisibleChange?.(true);
  }

  hide(): void {
    if (!this.#visible) return;
    this.#visible = false;
    if (this.#panel) this.#panel.hidden = true;
    if (this.#interval !== undefined) this.#timers.clearInterval(this.#interval);
    this.#interval = undefined;
    this.#d.onVisibleChange?.(false);
  }

  toggle(): void {
    if (this.#visible) this.hide();
    else this.show();
  }

  destroy(): void {
    this.hide();
    this.#panel?.remove();
    this.#panel = undefined;
    this.#rowEls.clear();
  }

  /** Reads the model and rewrites whatever changed. */
  refresh(): void {
    if (!this.#rowsEl) return;
    const model = this.#d.model();
    for (const row of buildOverlayRows(model)) {
      this.#setRow(row);
    }
    if (this.#pauseButton) {
      this.#pauseButton.textContent = t(model.paused ? 'overlay.resume' : 'overlay.pause');
    }
  }

  #setRow(row: OverlayRow): void {
    let el = this.#rowEls.get(row.id);
    if (!el) {
      el = h(this.#root.ownerDocument, 'div', { class: 'row', testid: `ov-${row.id}` });
      this.#rowEls.set(row.id, el);
      this.#rowsEl?.append(el);
    }
    const text = `${row.label.padEnd(LABEL_WIDTH)}${row.value}`;
    if (el.textContent !== text) {
      el.textContent = text;
      el.dataset['value'] = row.value;
    }
  }

  #build(): HTMLElement {
    const doc = this.#root.ownerDocument;
    const rows = h(doc, 'div', { class: 'rows', testid: 'ov-rows' });
    this.#rowsEl = rows;

    const out = (id: string): HTMLElement => h(doc, 'pre', { class: 'out', testid: id });
    const hashOut = out('ov-hash-out');
    const testOut = out('ov-selftest-out');
    const benchOut = out('ov-benchmark-out');

    const buttons = h(doc, 'div', { class: 'buttons' });
    const button = (
      id: string,
      label: string,
      onClick: (self: HTMLButtonElement) => void,
    ): HTMLButtonElement => {
      const b = h(doc, 'button', {
        class: 'button',
        testid: id,
        text: label,
        attrs: { type: 'button' },
      });
      b.addEventListener('click', () => {
        onClick(b);
      });
      buttons.append(b);
      return b;
    };
    /** A click handler for a slow action: the button is disabled meanwhile, the result shown in `into`. */
    const slow =
      (label: string, into: HTMLElement, job: () => Promise<string>) =>
      (b: HTMLButtonElement): void => {
        b.disabled = true;
        b.textContent = t('overlay.busy');
        job()
          .then(
            (text) => {
              into.textContent = text;
            },
            (e: unknown) => {
              into.textContent = `${t('overlay.fail')}: ${e instanceof Error ? e.message : String(e)}`;
            },
          )
          .finally(() => {
            b.disabled = false;
            b.textContent = label;
          });
      };

    const a = this.#d.actions;
    button(
      'ov-hash',
      t('overlay.hash'),
      slow(t('overlay.hash'), hashOut, async () => formatHashReport(await a.hash())),
    );
    button(
      'ov-selftest',
      t('overlay.selftest'),
      slow(t('overlay.selftest'), testOut, async () => {
        const r = await a.selfTest();
        return `${t(r.ok ? 'overlay.pass' : 'overlay.fail')}\n${r.text}`;
      }),
    );
    if (a.benchmark) {
      button(
        'ov-benchmark',
        t('overlay.benchmark'),
        slow(t('overlay.benchmark'), benchOut, a.benchmark),
      );
    }
    this.#pauseButton = button('ov-pause', t('overlay.pause'), () => {
      a.togglePause();
    });

    const panel = h(
      doc,
      'aside',
      {
        class: 'overlay',
        testid: 'overlay',
        attrs: { 'aria-label': t('overlay.title') },
      },
      h(doc, 'div', { class: 'title', text: `${t('overlay.title')} — ${t('overlay.hint')}` }),
      rows,
      buttons,
      hashOut,
      testOut,
      benchOut,
    );
    this.#root.append(panel);
    return panel;
  }
}

/** The Ctrl+Shift+D chord. Ignores auto-repeat so holding it does not flicker. */
export function isOverlayChord(e: {
  code: string;
  ctrlKey: boolean;
  shiftKey: boolean;
  altKey: boolean;
  metaKey: boolean;
  repeat?: boolean;
}): boolean {
  return (
    e.code === 'KeyD' && e.ctrlKey && e.shiftKey && !e.altKey && !e.metaKey && e.repeat !== true
  );
}

/** Toggles `overlay` on the chord. Returns a function that removes the listener. */
export function installOverlayHotkey(target: EventTarget, overlay: { toggle(): void }): () => void {
  const onKey = (e: Event): void => {
    const event = e as KeyboardEvent;
    if (isOverlayChord(event)) {
      event.preventDefault();
      overlay.toggle();
    }
  };
  target.addEventListener('keydown', onKey);
  return () => {
    target.removeEventListener('keydown', onKey);
  };
}
