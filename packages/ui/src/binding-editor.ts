/**
 * The binding editor, v1 (GDD 58.5, ADR-37): plain DOM over `EditorModel`.
 *
 * What it offers: profiles (choose, new, duplicate, rename, delete, export, import); every action
 * with up to four bindings shown as chips (add by pressing the control, remove, restore the
 * defaults of one action, a group or everything); conflicts and browser-reserved keys marked in
 * words as well as a shape; per-axis dead zone, expo, gain, smoothing and invert; the connected
 * devices with a live view of their axes, buttons and hats; and a calibration for each.
 *
 * Everything is a native control, so Tab, Enter, Space and the arrow keys work. The DOM is built
 * when the editor opens and removed when it closes. All text goes in through `textContent` and
 * comes from `strings.ts`.
 */
import { type ActionId, type AxisShape, defaultShape, type GamepadHub } from '@flying/input';
import { h } from './dom.ts';
import { type IntervalTimers } from './debug-overlay.ts';
import {
  type CaptureStatus,
  type EditorChip,
  type EditorModel,
  type EditorRow,
} from './editor-model.ts';
import { t, type StringKey } from './strings.ts';

/** How often the live view, capture and calibration look at the devices, milliseconds. */
export const EDITOR_POLL_MS = 50;

export interface BindingEditorDeps {
  model: EditorModel;
  hub: GamepadHub;
  /** Gives the user's browser the file to save. */
  download?: (name: string, text: string) => void;
  /** Reads a chosen file. */
  readFile?: (file: File) => Promise<string>;
  timers?: IntervalTimers;
  onClose(): void;
}

const BROWSER_INTERVALS: IntervalTimers = {
  setInterval: (cb, ms) => window.setInterval(cb, ms),
  clearInterval: (id) => {
    window.clearInterval(id);
  },
};

const HAT_GLYPH: Record<number, string> = {
  1: '↑',
  2: '→',
  4: '↓',
  8: '←',
  3: '↗',
  6: '↘',
  12: '↙',
  9: '↖',
};

function defaultDownload(doc: Document): (name: string, text: string) => void {
  return (name, text) => {
    const url = URL.createObjectURL(new Blob([text], { type: 'application/json' }));
    const a = doc.createElement('a');
    a.href = url;
    a.download = name;
    a.click();
    URL.revokeObjectURL(url);
  };
}

export class BindingEditor {
  readonly #root: HTMLElement;
  readonly #d: BindingEditorDeps;
  readonly #timers: IntervalTimers;
  #panel: HTMLElement | undefined;
  #interval: number | undefined;
  #search = '';
  #removeKeyListener: (() => void) | undefined;
  #liveRevision = -1;
  // Parts redrawn on their own.
  #bar: HTMLElement | undefined;
  #list: HTMLElement | undefined;
  #captureBox: HTMLElement | undefined;
  #devices: HTMLElement | undefined;
  #live: HTMLElement | undefined;
  #errors: HTMLElement | undefined;
  #conflicts: HTMLElement | undefined;

  constructor(root: HTMLElement, deps: BindingEditorDeps) {
    this.#root = root;
    this.#d = deps;
    this.#timers = deps.timers ?? BROWSER_INTERVALS;
  }

  get isOpen(): boolean {
    return this.#panel !== undefined;
  }

  /** True while the editor is waiting for a control: Esc then cancels the capture, not the menu. */
  get isCapturing(): boolean {
    return this.#d.model.capturing !== undefined;
  }

  get element(): HTMLElement | undefined {
    return this.#panel;
  }

  open(): void {
    if (this.#panel) return;
    this.#search = '';
    this.#liveRevision = -1;
    this.#panel = this.#build();
    this.#root.append(this.#panel);
    this.#renderAll();
    this.#interval = this.#timers.setInterval(() => {
      this.#tick();
    }, EDITOR_POLL_MS);
    this.#panel.querySelector<HTMLElement>('[data-testid="be-close"]')?.focus();
  }

  close(): void {
    if (!this.#panel) return;
    this.#d.model.cancelCapture();
    this.#d.model.cancelCalibration();
    this.#removeKeyListener?.();
    this.#removeKeyListener = undefined;
    if (this.#interval !== undefined) this.#timers.clearInterval(this.#interval);
    this.#interval = undefined;
    this.#panel.remove();
    this.#panel = undefined;
  }

  destroy(): void {
    this.close();
  }

  /** Redraws everything from the model (a profile changed under the editor). */
  refresh(): void {
    if (this.#panel) this.#renderAll();
  }

  // -- structure ---------------------------------------------------------------------------

  get #doc(): Document {
    return this.#root.ownerDocument;
  }

  #build(): HTMLElement {
    const doc = this.#doc;
    const close = h(doc, 'button', {
      text: t('bindings.close'),
      testid: 'be-close',
      attrs: { type: 'button' },
    });
    close.addEventListener('click', () => {
      this.#d.onClose();
    });
    const search = h(doc, 'input', {
      testid: 'be-search',
      attrs: {
        type: 'search',
        'aria-label': t('bindings.search'),
        placeholder: t('bindings.search'),
      },
    });
    search.addEventListener('input', () => {
      this.#search = search.value;
      this.#renderList();
    });

    this.#bar = h(doc, 'div', { class: 'profile-bar', testid: 'be-bar' });
    this.#conflicts = h(doc, 'p', { class: 'conflicts', testid: 'be-conflicts' });
    this.#errors = h(doc, 'ul', { class: 'import-errors', testid: 'be-import-errors' });
    this.#devices = h(doc, 'div', { class: 'devices', testid: 'be-devices' });
    this.#live = h(doc, 'div', { class: 'live', testid: 'be-live' });
    this.#list = h(doc, 'div', { class: 'actions', testid: 'be-list' });
    this.#captureBox = h(doc, 'div', {
      class: 'capture',
      testid: 'be-capture',
      attrs: { role: 'alertdialog' },
    });
    this.#captureBox.hidden = true;
    this.#captureBox.addEventListener('pointerdown', (e) => {
      this.#after(this.#d.model.feedMouse(e.button));
    });
    this.#captureBox.addEventListener('wheel', (e) => {
      e.preventDefault();
      this.#after(this.#d.model.feedWheel(e.deltaY < 0 ? 'up' : 'down'));
    });

    const panel = h(
      doc,
      'section',
      {
        class: 'binding-editor',
        testid: 'binding-editor',
        attrs: { role: 'dialog', 'aria-modal': 'true', 'aria-labelledby': 'be-title' },
      },
      h(
        doc,
        'header',
        {},
        h(doc, 'h2', { text: t('bindings.title'), attrs: { id: 'be-title' } }),
        close,
      ),
      this.#bar,
      this.#errors,
      this.#conflicts,
      this.#devices,
      this.#live,
      search,
      this.#list,
      this.#captureBox,
    );
    return panel;
  }

  #renderAll(): void {
    this.#renderBar();
    this.#renderErrors();
    this.#renderList();
    this.#renderDevices();
    this.#renderCapture();
  }

  // -- the profile bar ---------------------------------------------------------------------

  #profileName(p: { id: string; name: string }): string {
    const builtin: Record<string, StringKey> = {
      'default-km': 'bindings.profile.default-km',
      'gamepad-standard': 'bindings.profile.gamepad-standard',
      'generic-joystick': 'bindings.profile.generic-joystick',
    };
    const key = builtin[p.id];
    return key === undefined ? p.name : t(key);
  }

  #renderBar(): void {
    const bar = this.#bar;
    if (!bar) return;
    const doc = this.#doc;
    const m = this.#d.model;
    bar.replaceChildren();

    const select = h(doc, 'select', {
      testid: 'be-profile',
      attrs: { 'aria-label': t('bindings.profile') },
    });
    for (const p of m.profiles) {
      select.append(h(doc, 'option', { text: this.#profileName(p), attrs: { value: p.id } }));
    }
    select.value = m.profile.id;
    select.addEventListener('change', () => {
      m.select(select.value);
      this.#renderAll();
    });

    const name = h(doc, 'input', {
      testid: 'be-name',
      attrs: { type: 'text', maxlength: '64', 'aria-label': t('bindings.name') },
    });
    name.value = this.#profileName(m.profile);
    const klass = h(doc, 'select', {
      testid: 'be-class',
      attrs: { 'aria-label': t('bindings.class') },
    });
    for (const c of ['keyboard-mouse', 'gamepad', 'joystick'] as const) {
      klass.append(h(doc, 'option', { text: t(`bindings.class.${c}`), attrs: { value: c } }));
    }
    klass.value = m.profile.deviceClass;

    const act = (
      id: string,
      label: StringKey,
      run: () => void,
      disabled = false,
    ): HTMLButtonElement => {
      const b = h(doc, 'button', { text: t(label), testid: id, attrs: { type: 'button' } });
      b.disabled = disabled;
      b.addEventListener('click', run);
      return b;
    };

    const file = h(doc, 'input', {
      testid: 'be-import',
      attrs: { type: 'file', accept: 'application/json,.json', 'aria-label': t('bindings.import') },
    });
    file.addEventListener('change', () => {
      const chosen = file.files?.[0];
      if (!chosen) return;
      const read = this.#d.readFile ?? ((f: File) => f.text());
      void read(chosen).then(async (text) => {
        await m.importText(text);
        this.#renderAll();
      });
    });

    const toNew = klass;
    bar.append(
      h(doc, 'label', {}, h(doc, 'span', { text: t('bindings.profile') }), select),
      h(doc, 'label', {}, h(doc, 'span', { text: t('bindings.name') }), name),
      act(
        'be-rename',
        'bindings.rename',
        () => {
          m.rename(name.value);
          this.#renderAll();
        },
        m.isBuiltin,
      ),
      act('be-duplicate', 'bindings.duplicate', () => {
        m.duplicate(name.value);
        this.#renderAll();
      }),
      h(doc, 'label', {}, h(doc, 'span', { text: t('bindings.class') }), toNew),
      act('be-new', 'bindings.new', () => {
        m.create(klass.value as 'keyboard-mouse' | 'gamepad' | 'joystick', name.value);
        this.#renderAll();
      }),
      act(
        'be-delete',
        'bindings.delete',
        () => {
          m.delete();
          this.#renderAll();
        },
        m.isBuiltin,
      ),
      act('be-export', 'bindings.export', () => {
        const f = m.exportFile();
        (this.#d.download ?? defaultDownload(doc))(f.name, f.text);
      }),
      h(doc, 'label', {}, h(doc, 'span', { text: t('bindings.import') }), file),
      act('be-restore-all', 'bindings.restore-all', () => {
        m.restoreAll();
        this.#renderAll();
      }),
    );
    if (m.isBuiltin) {
      bar.append(
        h(doc, 'p', { class: 'note', testid: 'be-builtin-note', text: t('bindings.builtin-note') }),
      );
    }
  }

  #renderErrors(): void {
    const el = this.#errors;
    if (!el) return;
    el.replaceChildren();
    const errors = this.#d.model.importErrors;
    el.hidden = errors.length === 0;
    if (errors.length === 0) return;
    const doc = this.#doc;
    el.append(h(doc, 'li', { text: t('bindings.import-failed', { count: errors.length }) }));
    for (const e of errors.slice(0, 20)) {
      el.append(
        h(doc, 'li', {
          testid: 'be-import-error',
          text: t('bindings.import-error', {
            pointer: e.pointer === '' ? '/' : e.pointer,
            message: e.message,
          }),
        }),
      );
    }
  }

  // -- the action list ---------------------------------------------------------------------

  #renderList(): void {
    const list = this.#list;
    if (!list) return;
    const doc = this.#doc;
    const m = this.#d.model;
    list.replaceChildren();
    const groups = m.groups(this.#search);
    if (groups.length === 0) {
      list.append(h(doc, 'p', { testid: 'be-none', text: t('bindings.none-found') }));
    }
    for (const g of groups) {
      const restoreGroup = h(doc, 'button', {
        text: t('bindings.restore-group'),
        testid: `be-restore-group-${g.group}`,
        attrs: { type: 'button' },
      });
      restoreGroup.addEventListener('click', () => {
        m.restoreGroup(g.group);
        this.#renderAll();
      });
      const fieldset = h(
        doc,
        'fieldset',
        { class: 'group', testid: `be-group-${g.group}` },
        h(doc, 'legend', { text: t(`bindings.group.${g.group}`) }),
        restoreGroup,
      );
      for (const row of g.rows) fieldset.append(this.#row(row));
      list.append(fieldset);
    }
    const n = m.conflictCount();
    if (this.#conflicts) {
      this.#conflicts.textContent = n === 0 ? '' : t('bindings.conflicts', { count: n });
      this.#conflicts.hidden = n === 0;
    }
  }

  #row(row: EditorRow): HTMLElement {
    const doc = this.#doc;
    const m = this.#d.model;
    const chips = row.chips.map((c) => this.#chip(row, c));
    const add = (direction: 1 | -1, label: StringKey, id: string): HTMLButtonElement => {
      const b = h(doc, 'button', { text: t(label), testid: id, attrs: { type: 'button' } });
      b.disabled = !row.canAdd;
      b.addEventListener('click', () => {
        if (m.beginCapture(row.id, direction)) this.#renderCapture();
      });
      return b;
    };
    const buttons = row.bipolar
      ? [
          add(-1, 'bindings.add-negative', `be-add-negative-${row.id}`),
          add(1, 'bindings.add-positive', `be-add-positive-${row.id}`),
        ]
      : [add(1, 'bindings.add', `be-add-${row.id}`)];
    const restore = h(doc, 'button', {
      text: t('bindings.restore-action'),
      testid: `be-restore-${row.id}`,
      attrs: { type: 'button' },
    });
    restore.addEventListener('click', () => {
      m.restoreAction(row.id);
      this.#renderAll();
    });
    const el = h(
      doc,
      'div',
      { class: 'row', testid: `be-row-${row.id}` },
      h(doc, 'span', { class: 'label', text: row.label }),
      h(doc, 'span', { class: 'chips' }, ...chips),
      ...buttons,
      restore,
    );
    if (row.shaped) el.append(this.#shape(row.id));
    return el;
  }

  #chip(row: EditorRow, chip: EditorChip): HTMLElement {
    const doc = this.#doc;
    const sign = chip.direction === undefined ? '' : chip.direction < 0 ? ' −' : ' +';
    const text = `${chip.device === undefined ? '' : `${chip.device} `}${chip.label}${sign}`;
    const remove = h(doc, 'button', {
      text: '×',
      testid: `be-remove-${row.id}-${String(chip.index)}`,
      attrs: { type: 'button', 'aria-label': t('bindings.remove', { name: text }) },
    });
    remove.addEventListener('click', () => {
      this.#d.model.remove(chip.index);
      this.#renderAll();
    });
    const chipEl = h(
      doc,
      'span',
      { class: 'chip', data: { conflict: String(chip.conflict), reserved: String(chip.reserved) } },
      h(doc, 'span', { text }),
    );
    if (chip.conflict) {
      chipEl.append(
        h(doc, 'span', {
          class: 'tag',
          testid: 'be-conflict',
          text: `⚠ ${t('bindings.conflict')}`,
        }),
      );
    }
    if (chip.reserved) {
      chipEl.append(
        h(doc, 'span', {
          class: 'tag',
          testid: 'be-reserved',
          text: `⚠ ${t('bindings.reserved')}`,
        }),
      );
    }
    chipEl.append(remove);
    return chipEl;
  }

  /** The shaping controls of an axis or rate action, in a collapsed section. */
  #shape(action: ActionId): HTMLElement {
    const doc = this.#doc;
    const m = this.#d.model;
    const current = (): AxisShape => m.profile.axes[action] ?? defaultShape(action);
    const slider = (
      id: string,
      label: StringKey,
      min: number,
      max: number,
      step: number,
      read: (s: AxisShape) => number,
      write: (value: number) => Partial<AxisShape>,
    ): HTMLElement => {
      const input = h(doc, 'input', {
        testid: `be-${id}-${action}`,
        attrs: {
          type: 'range',
          min: String(min),
          max: String(max),
          step: String(step),
          'aria-label': t(label),
        },
      });
      input.value = String(read(current()));
      const out = h(doc, 'output', { text: String(read(current())) });
      input.addEventListener('input', () => {
        out.textContent = input.value;
      });
      input.addEventListener('change', () => {
        const v = Number(input.value);
        if (Number.isFinite(v)) {
          m.setShape(action, write(v));
          this.#renderBar();
        }
      });
      return h(doc, 'label', { class: 'slider' }, h(doc, 'span', { text: t(label) }), input, out);
    };
    const shape = current();
    const invert = h(doc, 'input', {
      testid: `be-invert-${action}`,
      attrs: { type: 'checkbox' },
    });
    invert.checked = shape.invert;
    invert.addEventListener('change', () => {
      m.setShape(action, { invert: invert.checked });
      this.#renderBar();
    });
    const body: HTMLElement[] = [
      slider(
        'dead',
        'bindings.shape.dead',
        0,
        0.5,
        0.01,
        (s) => s.deadCentre,
        (v) => ({ deadCentre: v }),
      ),
      slider(
        'outer',
        'bindings.shape.outer',
        0,
        0.5,
        0.01,
        (s) => s.deadOuter,
        (v) => ({ deadOuter: v }),
      ),
      shape.curve.kind === 'expo'
        ? slider(
            'curve',
            'bindings.shape.expo',
            0,
            1,
            0.05,
            (s) => (s.curve.kind === 'expo' ? s.curve.k : 0),
            (v) => ({ curve: { kind: 'expo', k: v } }),
          )
        : slider(
            'curve',
            'bindings.shape.gamma',
            0.2,
            5,
            0.1,
            (s) => (s.curve.kind === 'power' ? s.curve.gamma : 1),
            (v) => ({ curve: { kind: 'power', gamma: v } }),
          ),
      slider(
        'gain',
        'bindings.shape.gain',
        0.5,
        2,
        0.05,
        (s) => s.gain,
        (v) => ({ gain: v }),
      ),
      slider(
        'smooth',
        'bindings.shape.smooth',
        0,
        120,
        5,
        (s) => s.tauMs,
        (v) => ({ tauMs: v }),
      ),
      h(
        doc,
        'label',
        { class: 'check' },
        invert,
        h(doc, 'span', { text: t('bindings.shape.invert') }),
      ),
    ];
    return h(
      doc,
      'details',
      { class: 'shape', testid: `be-shape-${action}` },
      h(doc, 'summary', { text: t('bindings.shape') }),
      ...body,
    );
  }

  // -- capture -----------------------------------------------------------------------------

  #renderCapture(): void {
    const box = this.#captureBox;
    if (!box) return;
    const doc = this.#doc;
    const target = this.#d.model.capturing;
    box.replaceChildren();
    box.hidden = target === undefined;
    this.#removeKeyListener?.();
    this.#removeKeyListener = undefined;
    if (target === undefined) return;
    const cancel = h(doc, 'button', {
      text: t('bindings.cancel'),
      testid: 'be-capture-cancel',
      attrs: { type: 'button' },
    });
    cancel.addEventListener('click', () => {
      this.#d.model.cancelCapture();
      this.#renderCapture();
    });
    box.append(
      h(doc, 'p', {
        testid: 'be-capture-prompt',
        text: t('bindings.capture', { action: t(`action.${target.action}`) }),
      }),
      cancel,
    );
    const onKey = (e: Event): void => {
      const event = e as KeyboardEvent;
      event.preventDefault();
      event.stopImmediatePropagation();
      if (event.repeat) return;
      this.#after(this.#d.model.feedKey(event.code));
    };
    const win = doc.defaultView;
    win?.addEventListener('keydown', onKey, true);
    this.#removeKeyListener = () => {
      win?.removeEventListener('keydown', onKey, true);
    };
  }

  /** Reacts to what a capture attempt came to. */
  #after(status: CaptureStatus): void {
    if (status.state === 'waiting') return;
    this.#renderCapture();
    if (status.state === 'rejected') {
      this.#d.model.clearImportErrors();
      const el = this.#errors;
      if (el) {
        el.hidden = false;
        el.replaceChildren(
          h(this.#doc, 'li', { testid: 'be-rejected', text: t('bindings.throttle-needs-axis') }),
        );
      }
      return;
    }
    this.#renderAll();
    if (status.state === 'added') {
      const id = status.binding.action;
      this.#panel?.querySelector<HTMLElement>(`[data-testid="be-row-${id}"] button`)?.focus();
    }
  }

  // -- devices and the live view -----------------------------------------------------------

  #renderDevices(): void {
    const el = this.#devices;
    if (!el) return;
    const doc = this.#doc;
    const m = this.#d.model;
    const hub = this.#d.hub;
    el.replaceChildren(h(doc, 'h3', { text: t('bindings.devices') }));
    let any = false;
    for (const [i, slot] of hub.slots.entries()) {
      if (!slot.present) continue;
      any = true;
      const calibrate = h(doc, 'button', {
        text: t('bindings.calibrate'),
        testid: `be-calibrate-${String(i)}`,
        attrs: { type: 'button' },
      });
      calibrate.addEventListener('click', () => {
        if (m.beginCalibration(i)) this.#renderDevices();
      });
      el.append(
        h(
          doc,
          'div',
          { class: 'device', testid: `be-device-${String(i)}` },
          h(doc, 'span', { text: slot.id === '' ? slot.device : slot.id }),
          calibrate,
        ),
      );
    }
    if (!any) el.append(h(doc, 'p', { testid: 'be-no-devices', text: t('bindings.no-devices') }));
    const cal = m.calibrating;
    if (cal) {
      const next = h(doc, 'button', {
        text: t(cal.step === 'range' ? 'bindings.calibrate-done' : 'bindings.calibrate-next'),
        testid: 'be-calibrate-next',
        attrs: { type: 'button' },
      });
      next.addEventListener('click', () => {
        m.advanceCalibration();
        this.#renderAll();
      });
      const stop = h(doc, 'button', {
        text: t('bindings.cancel'),
        testid: 'be-calibrate-cancel',
        attrs: { type: 'button' },
      });
      stop.addEventListener('click', () => {
        m.cancelCalibration();
        this.#renderDevices();
      });
      el.append(
        h(
          doc,
          'div',
          { class: 'calibration', testid: 'be-calibration' },
          h(doc, 'p', {
            testid: 'be-calibrate-text',
            text: t(
              cal.step === 'range' ? 'bindings.calibrate-range' : 'bindings.calibrate-centre',
            ),
          }),
          next,
          stop,
        ),
      );
    }
    this.#liveRevision = hub.revision;
  }

  /** The axes, buttons and hats of each device, redrawn in place. */
  #renderLive(): void {
    const el = this.#live;
    if (!el) return;
    const doc = this.#doc;
    const hub = this.#d.hub;
    if (el.childElementCount !== hub.count) {
      el.replaceChildren();
      hub.slots.forEach((slot, i) => {
        if (!slot.present) return;
        const meters: HTMLElement[] = [];
        for (let a = 0; a < Math.min(slot.axisCount, 8); a++) {
          meters.push(
            h(doc, 'meter', {
              testid: `be-axis-${String(i)}-${String(a)}`,
              attrs: {
                min: '-1',
                max: '1',
                value: '0',
                'aria-label': t('bindings.source.axis', { n: a }),
              },
            }),
          );
        }
        el.append(
          h(
            doc,
            'div',
            { class: 'live-device', testid: `be-live-${String(i)}` },
            ...meters,
            h(doc, 'span', { testid: `be-pressed-${String(i)}` }),
          ),
        );
      });
    }
    hub.slots.forEach((slot, i) => {
      if (!slot.present) return;
      const box = el.querySelector<HTMLElement>(`[data-testid="be-live-${String(i)}"]`);
      if (!box) return;
      for (let a = 0; a < Math.min(slot.axisCount, 8); a++) {
        const meter = box.querySelector<HTMLMeterElement>(
          `[data-testid="be-axis-${String(i)}-${String(a)}"]`,
        );
        if (meter) meter.setAttribute('value', String(slot.axes[a] ?? 0));
      }
      let pressed = '';
      for (let b = 0; b < slot.buttonCount; b++)
        if ((slot.buttons[b] ?? 0) > 0.5) pressed += ` ${String(b)}`;
      const hat = HAT_GLYPH[slot.hats[0] ?? 0];
      const text = `${pressed === '' ? '' : t('bindings.live-buttons', { list: pressed.trim() })}${hat === undefined ? '' : ` ${hat}`}`;
      const span = box.querySelector<HTMLElement>(`[data-testid="be-pressed-${String(i)}"]`);
      if (span && span.textContent !== text) span.textContent = text;
    });
  }

  #tick(): void {
    const m = this.#d.model;
    if (this.#d.hub.revision !== this.#liveRevision) {
      this.#renderDevices();
      this.#live?.replaceChildren();
    }
    this.#renderLive();
    if (m.capturing) this.#after(m.pollPads());
    if (m.calibrating) m.sampleCalibration();
  }
}
