/**
 * Form controls the menu screens share (main menu, first run, settings): a group of chips that is
 * one choice, a slider with its value, a checkbox and a button. Each is a native control, so Tab
 * moves between them, Enter and Space press them, and the arrows move a slider; none has a handler
 * that depends on the pointer. A control reads its value from the settings when asked to `refresh`,
 * so a change made elsewhere shows.
 *
 * Test ids are `<prefix>-<id>` (`<prefix>-<id>-<value>` for a chip, `-value` for a slider's output).
 */
import { h } from './dom.ts';
import { t, type StringKey } from './strings.ts';

export interface Choice<V> {
  value: V;
  label: StringKey;
  /** Shown under the group while the choice has focus or the pointer is over it. */
  preview?: StringKey;
}

export class Form {
  readonly #doc: Document;
  readonly #prefix: string;
  readonly #sync: (() => void)[] = [];

  constructor(doc: Document, prefix: string) {
    this.#doc = doc;
    this.#prefix = prefix;
  }

  /** Re-reads every control's value. */
  refresh(): void {
    for (const fn of this.#sync) fn();
  }

  #id(id: string): string {
    return `${this.#prefix}-${id}`;
  }

  /** One choice among several, as buttons that are pressed or not. */
  group<V extends string | number>(
    id: string,
    title: StringKey,
    choices: readonly Choice<V>[],
    current: () => V,
    pick: (value: V) => void,
    note?: StringKey,
  ): HTMLElement {
    const doc = this.#doc;
    const legend = h(doc, 'legend', { text: t(title) });
    const buttons = choices.map((c) =>
      h(doc, 'button', {
        class: 'chip',
        text: t(c.label),
        testid: this.#id(`${id}-${String(c.value)}`),
        attrs: { type: 'button', 'aria-pressed': 'false' },
      }),
    );
    const preview = h(doc, 'p', { class: 'preview', testid: this.#id(`${id}-preview`) });
    const showPreview = (value: V): void => {
      const key = choices.find((c) => c.value === value)?.preview;
      preview.textContent = key === undefined ? '' : t(key);
    };
    const refresh = (): void => {
      const now = current();
      buttons.forEach((b, i) => {
        const choice = choices[i];
        if (choice !== undefined) b.setAttribute('aria-pressed', String(choice.value === now));
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
      const peek = (): void => {
        showPreview(choice.value);
      };
      b.addEventListener('focus', peek);
      b.addEventListener('mouseenter', peek);
      b.addEventListener('blur', refresh);
      b.addEventListener('mouseleave', refresh);
    });
    this.#sync.push(refresh);
    refresh();
    const fieldset = h(
      doc,
      'fieldset',
      { class: 'group', testid: this.#id(id) },
      legend,
      ...buttons,
      preview,
    );
    if (note !== undefined) {
      fieldset.append(
        h(doc, 'p', { class: 'note', testid: this.#id(`${id}-note`), text: t(note) }),
      );
    }
    return fieldset;
  }

  slider(
    id: string,
    title: StringKey,
    range: { min: number; max: number; step: number },
    current: () => number,
    pick: (value: number) => void,
    format: (value: number) => string,
  ): HTMLElement {
    const doc = this.#doc;
    const input = h(doc, 'input', {
      testid: this.#id(id),
      attrs: {
        type: 'range',
        min: String(range.min),
        max: String(range.max),
        step: String(range.step),
        'aria-label': t(title),
      },
    });
    const out = h(doc, 'output', { testid: this.#id(`${id}-value`) });
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
  }

  check(
    id: string,
    title: StringKey,
    current: () => boolean,
    pick: (value: boolean) => void,
  ): HTMLElement {
    const doc = this.#doc;
    const box = h(doc, 'input', { testid: this.#id(id), attrs: { type: 'checkbox' } });
    const refresh = (): void => {
      box.checked = current();
    };
    box.addEventListener('change', () => {
      pick(box.checked);
    });
    this.#sync.push(refresh);
    refresh();
    return h(doc, 'label', { class: 'check' }, box, h(doc, 'span', { text: t(title) }));
  }

  button(id: string, label: StringKey, action: () => void, text?: string): HTMLButtonElement {
    const b = h(this.#doc, 'button', {
      text: text ?? t(label),
      testid: this.#id(id),
      attrs: { type: 'button' },
    });
    b.addEventListener('click', action);
    return b;
  }
}

/** Keeps Tab inside `panel`: past the last control it goes to the first, and back. */
export function trapTab(panel: HTMLElement, doc: Document, e: KeyboardEvent): void {
  if (e.key !== 'Tab') return;
  const items = [
    ...panel.querySelectorAll<HTMLElement>(
      'button, input, select, a[href], [tabindex]:not([tabindex="-1"])',
    ),
  ].filter((el) => !el.hasAttribute('disabled') && !el.hidden);
  const first = items[0];
  const last = items.at(-1);
  if (first === undefined || last === undefined) return;
  if (e.shiftKey && doc.activeElement === first) {
    e.preventDefault();
    last.focus();
  } else if (!e.shiftKey && doc.activeElement === last) {
    e.preventDefault();
    first.focus();
  }
}
