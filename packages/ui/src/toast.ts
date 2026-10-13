/**
 * Short messages that appear and go by themselves. At most three are on screen (GDD 53.4); a new
 * one pushes the oldest out. The kind is stated in words ("Warning:") as well as by colour.
 */
import { h } from './dom.ts';
import { t, type StringKey } from './strings.ts';

export type ToastKind = 'info' | 'warn' | 'error';

export const MAX_TOASTS = 3;

const LABEL: Record<ToastKind, StringKey> = {
  info: 'toast.info',
  warn: 'toast.warn',
  error: 'toast.error',
};

/** How long each kind stays, milliseconds. Problems stay longest. */
const DURATION_MS: Record<ToastKind, number> = { info: 4000, warn: 8000, error: 12_000 };

export interface Timers {
  set(callback: () => void, ms: number): number;
  clear(id: number): void;
}

const BROWSER_TIMERS: Timers = {
  set: (cb, ms) => window.setTimeout(cb, ms),
  clear: (id) => {
    window.clearTimeout(id);
  },
};

export class ToastHost {
  readonly #root: HTMLElement;
  readonly #timers: Timers;
  readonly #live: { el: HTMLElement; timer: number }[] = [];

  constructor(root: HTMLElement, timers: Timers = BROWSER_TIMERS) {
    this.#root = root;
    this.#timers = timers;
    root.setAttribute('aria-live', 'polite');
  }

  show(text: string, kind: ToastKind = 'info'): void {
    const doc = this.#root.ownerDocument;
    const el = h(doc, 'div', {
      class: 'toast',
      testid: 'toast',
      data: { kind },
      attrs: { role: kind === 'error' ? 'alert' : 'status' },
      text: `${t(LABEL[kind])}: ${text}`,
    });
    this.#root.append(el);
    const timer = this.#timers.set(() => {
      this.#remove(el);
    }, DURATION_MS[kind]);
    this.#live.push({ el, timer });
    while (this.#live.length > MAX_TOASTS) {
      const oldest = this.#live[0];
      if (oldest) this.#remove(oldest.el);
    }
  }

  get count(): number {
    return this.#live.length;
  }

  #remove(el: HTMLElement): void {
    const i = this.#live.findIndex((l) => l.el === el);
    const entry = this.#live[i];
    if (entry) {
      this.#timers.clear(entry.timer);
      this.#live.splice(i, 1);
    }
    el.remove();
  }
}
