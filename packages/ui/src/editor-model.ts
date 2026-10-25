/**
 * The binding editor's state and operations, without the page (GDD 58.5, ADR-37).
 *
 * `BindingEditor` draws this and forwards clicks and keys to it; keeping the logic here means the
 * rules can be tested without a DOM: which profile is being edited, what each action's chips say,
 * what a capture adds and what it refuses, what a change does to the stored profiles.
 *
 * The built-in profiles cannot be changed. The first edit to one makes a copy of it ("... custom"),
 * stores it, selects it, and applies the edit there.
 */
import {
  ACTIONS,
  actionDef,
  type ActionDef,
  type ActionId,
  addBinding,
  applyCalibration,
  bindingsOf,
  CalibrationWizard,
  Capture,
  type CaptureAxis,
  conflictedBindings,
  type ControlsProfile,
  type DeviceClass,
  type Direction,
  duplicateProfile,
  findConflicts,
  type GamepadHub,
  hasRoom,
  keyName,
  newProfile,
  type ProfileError,
  removeBinding,
  renameProfile,
  reservedShortcutWarnings,
  restoreAction,
  restoreAll,
  restoreGroup,
  serializeProfile,
  setShape,
  type AxisShape,
  type Binding,
  type Source,
} from '@flying/input';
import { type ControlsStore } from './controls-store.ts';
import { t } from './strings.ts';

export type EditorGroup = ActionDef['group'];
/** The groups in the order the editor lists them. */
export const EDITOR_GROUPS: readonly EditorGroup[] = [
  'axes',
  'engine',
  'systems',
  'brakes',
  'view',
  'system',
];

/** What a source is called on a chip. */
export function sourceLabel(s: Source): string {
  switch (s.kind) {
    case 'key':
      return keyName(s.code);
    case 'mouse-button':
      return t('bindings.source.mouse', { n: s.button + 1 });
    case 'wheel':
      return t(s.dir === 'up' ? 'bindings.source.wheel-up' : 'bindings.source.wheel-down');
    case 'pad-button':
      return t('bindings.source.button', { n: s.index });
    case 'pad-axis':
      return t(
        s.half === undefined
          ? 'bindings.source.axis'
          : s.half === 'positive'
            ? 'bindings.source.axis-positive'
            : 'bindings.source.axis-negative',
        { n: s.index },
      );
    case 'hat':
      return t('bindings.source.hat', { n: s.hat, dir: t(`bindings.hat.${s.dir}`) });
  }
}

/** What a device class is called: `Pad`, `Joystick`, or `Joystick 044f:b10a`. */
export function deviceLabel(device: string): string {
  if (device === 'pad') return t('bindings.device.pad');
  if (device === 'joy') return t('bindings.device.joy');
  const id = /^joy:(.+)$/.exec(device)?.[1];
  return id === undefined ? device : t('bindings.device.joy-id', { id });
}

export interface EditorChip {
  /** Position in `profile.bindings`. */
  index: number;
  /** `W`, `Axis 2+`, ... */
  label: string;
  /** The device a pad source belongs to, spelled out; undefined for keys and the mouse. */
  device: string | undefined;
  /** The sign a key or button gives a bipolar axis. */
  direction: Direction | undefined;
  /** Another action uses the same control. */
  conflict: boolean;
  /** The browser keeps this key with Ctrl (or Alt) held. */
  reserved: boolean;
}

export interface EditorRow {
  id: ActionId;
  label: string;
  kind: ActionDef['kind'];
  /** A bipolar axis: a key adds to one end or the other. */
  bipolar: boolean;
  /** Has a shape (dead zone, curve, gain, smoothing, invert). */
  shaped: boolean;
  chips: EditorChip[];
  canAdd: boolean;
}

export interface EditorGroupView {
  group: EditorGroup;
  rows: EditorRow[];
}

export type CaptureStatus =
  | { state: 'waiting' }
  | { state: 'added'; binding: Binding }
  | { state: 'cancelled' }
  | { state: 'rejected'; reason: 'throttle-needs-axis' };

export type ImportOutcome = { ok: true } | { ok: false; errors: ProfileError[] };

export interface EditorModelDeps {
  store: ControlsStore;
  hub: GamepadHub;
  /** A profile was chosen or changed: fly with this one. */
  onProfile(profile: ControlsProfile): void;
  /** The clock `Capture` settles on, milliseconds. */
  nowMs(): number;
}

export class EditorModel {
  readonly #d: EditorModelDeps;
  readonly #capture = new Capture();
  #id: string;
  #target: { action: ActionId; direction: Direction } | undefined;
  #wizard: { device: string; wizard: CalibrationWizard; slot: number } | undefined;
  #importErrors: ProfileError[] = [];

  /** `initialId` is the profile to start on (the one in use); an unknown id picks the first. */
  constructor(deps: EditorModelDeps, initialId: string) {
    this.#d = deps;
    this.#id = deps.store.find(initialId)?.id ?? deps.store.all()[0]?.id ?? 'default-km';
  }

  // -- the profile ---------------------------------------------------------------------------

  get profile(): ControlsProfile {
    const p = this.#d.store.find(this.#id) ?? this.#d.store.all()[0];
    if (p === undefined) throw new Error('no profiles');
    return p;
  }

  get isBuiltin(): boolean {
    return this.#d.store.isBuiltin(this.#id);
  }

  /** Every profile, built-ins first. */
  get profiles(): ControlsProfile[] {
    return this.#d.store.all();
  }

  /** Problems found by the last import, by JSON pointer. */
  get importErrors(): readonly ProfileError[] {
    return this.#importErrors;
  }

  /** Switches to another profile and makes it the one in use. */
  select(id: string): void {
    if (this.#d.store.find(id) === undefined) return;
    this.#id = id;
    this.#capture.reset();
    this.#target = undefined;
    this.#wizard = undefined;
    void this.#d.store.setActive(id);
    this.#d.onProfile(this.profile);
  }

  // -- what is shown -------------------------------------------------------------------------

  /** The actions to list, by group, keeping those that match `search` (empty: all). */
  groups(search = ''): EditorGroupView[] {
    const p = this.profile;
    const conflicts = conflictedBindings(findConflicts(p));
    const reserved = new Set(reservedShortcutWarnings(p).map((w) => w.binding));
    const terms = search
      .toLowerCase()
      .split(/\s+/)
      .filter((x) => x !== '');
    const out: EditorGroupView[] = [];
    for (const group of EDITOR_GROUPS) {
      const rows: EditorRow[] = [];
      for (const def of ACTIONS.filter((a) => a.group === group)) {
        const chips: EditorChip[] = bindingsOf(p, def.id).map(({ binding, index }) => ({
          index,
          label: sourceLabel(binding.source),
          device:
            binding.source.kind === 'pad-button' ||
            binding.source.kind === 'pad-axis' ||
            binding.source.kind === 'hat'
              ? deviceLabel(binding.source.device)
              : undefined,
          direction: def.range === 'bipolar' ? (binding.direction ?? 1) : undefined,
          conflict: conflicts.has(index),
          reserved: reserved.has(index),
        }));
        const row: EditorRow = {
          id: def.id,
          label: t(`action.${def.id}`),
          kind: def.kind,
          bipolar: def.range === 'bipolar',
          shaped: def.kind === 'axis' || def.kind === 'rate',
          chips,
          canAdd: hasRoom(p, def.id),
        };
        const haystack = [row.label, def.id, ...chips.map((c) => `${c.label} ${c.device ?? ''}`)]
          .join(' ')
          .toLowerCase();
        if (terms.every((term) => haystack.includes(term))) rows.push(row);
      }
      if (rows.length > 0) out.push({ group, rows });
    }
    return out;
  }

  /** How many bindings are in conflict. */
  conflictCount(): number {
    return conflictedBindings(findConflicts(this.profile)).size;
  }

  // -- capture -------------------------------------------------------------------------------

  /** What is being captured, if anything. */
  get capturing(): { action: ActionId; direction: Direction } | undefined {
    return this.#target;
  }

  /**
   * Starts waiting for a control for `action`. `direction` is the sign a key or button gives a
   * bipolar axis (default +1). Returns false if the action already has as many bindings as it can.
   */
  beginCapture(action: ActionId, direction: Direction = 1): boolean {
    const def = actionDef(action);
    if (def === undefined || !hasRoom(this.profile, action)) return false;
    const axis: CaptureAxis = def.kind === 'axis' ? 'whole' : 'half';
    this.#target = { action, direction };
    this.#capture.start(this.#d.hub, this.#d.nowMs(), axis);
    return true;
  }

  cancelCapture(): void {
    this.#capture.reset();
    this.#target = undefined;
  }

  feedKey(code: string): CaptureStatus {
    this.#capture.keyDown(code);
    return this.#settle();
  }

  feedMouse(button: number): CaptureStatus {
    this.#capture.mouseButton(button);
    return this.#settle();
  }

  feedWheel(dir: 'up' | 'down'): CaptureStatus {
    this.#capture.wheel(dir);
    return this.#settle();
  }

  /** Looks at the connected pads. Call it a few times a second while capturing. */
  pollPads(): CaptureStatus {
    this.#capture.poll(this.#d.hub, this.#d.nowMs());
    return this.#settle();
  }

  #settle(): CaptureStatus {
    const state = this.#capture.state;
    const target = this.#target;
    if (target === undefined || state.state === 'idle' || state.state === 'waiting') {
      return { state: 'waiting' };
    }
    this.#capture.reset();
    this.#target = undefined;
    if (state.state === 'cancelled') return { state: 'cancelled' };

    const source = state.source;
    const def = actionDef(target.action);
    if (
      target.action === 'flight.throttle' &&
      source.kind !== 'pad-axis' &&
      source.kind !== 'pad-button'
    ) {
      return { state: 'rejected', reason: 'throttle-needs-axis' };
    }
    const signed = def?.range === 'bipolar' && source.kind !== 'pad-axis';
    const binding: Binding = signed
      ? { action: target.action, source, direction: target.direction }
      : { action: target.action, source };
    this.#edit((p) => addBinding(p, binding));
    return { state: 'added', binding };
  }

  // -- edits ---------------------------------------------------------------------------------

  /**
   * Applies `change` to the profile. A built-in profile is copied first, and the copy is changed
   * and selected. A change that returns `undefined` is dropped.
   */
  #edit(change: (p: ControlsProfile) => ControlsProfile | undefined): void {
    let base = this.profile;
    if (this.isBuiltin) {
      base = duplicateProfile(
        base,
        this.#d.store.all().map((p) => p.id),
        t('bindings.custom-name', { name: this.profile.name }),
      );
    }
    const next = change(base);
    // A change that changes nothing (restoring what is already the default) makes no copy.
    if (next === undefined || serializeProfile(next) === serializeProfile(base)) return;
    this.#id = next.id;
    void this.#d.store.save(next).then(() => this.#d.store.setActive(next.id));
    this.#d.onProfile(next);
  }

  remove(index: number): void {
    this.#edit((p) => removeBinding(p, index));
  }

  restoreAction(action: ActionId): void {
    this.#edit((p) => restoreAction(p, action));
  }

  restoreGroup(group: EditorGroup): void {
    this.#edit((p) => restoreGroup(p, group));
  }

  restoreAll(): void {
    this.#edit((p) => restoreAll(p));
  }

  setShape(action: ActionId, patch: Partial<AxisShape>): void {
    this.#edit((p) => setShape(p, action, patch));
  }

  rename(name: string): void {
    if (this.isBuiltin || name.trim() === '') return;
    this.#edit((p) => renameProfile(p, name));
  }

  /** A copy of the profile under a new name, selected. */
  duplicate(name: string): void {
    const ids = this.#d.store.all().map((p) => p.id);
    const copy = duplicateProfile(this.profile, ids, name.trim() === '' ? this.profile.name : name);
    this.#id = copy.id;
    void this.#d.store.save(copy).then(() => this.#d.store.setActive(copy.id));
    this.#d.onProfile(copy);
  }

  /** A fresh profile of the given class, with its defaults, selected. */
  create(deviceClass: DeviceClass, name: string): void {
    const ids = this.#d.store.all().map((p) => p.id);
    const fresh = newProfile(deviceClass, ids, name);
    this.#id = fresh.id;
    void this.#d.store.save(fresh).then(() => this.#d.store.setActive(fresh.id));
    this.#d.onProfile(fresh);
  }

  /** Deletes the profile (not a built-in) and moves to the built-in of its class. */
  delete(): void {
    if (this.isBuiltin) return;
    const gone = this.profile;
    const fallback =
      gone.deviceClass === 'gamepad'
        ? 'gamepad-standard'
        : gone.deviceClass === 'joystick'
          ? 'generic-joystick'
          : 'default-km';
    void this.#d.store.remove(gone.id);
    this.#id = fallback;
    void this.#d.store.setActive(fallback);
    this.#d.onProfile(this.profile);
  }

  /** The file to download for the profile: its name and its `controls/1` text. */
  exportFile(): { name: string; text: string } {
    const p = this.profile;
    return { name: `${p.id}.controls.json`, text: this.#d.store.exportText(p) };
  }

  /** Reads a profile file. On success it is selected; on failure the problems are in `importErrors`. */
  async importText(text: string): Promise<ImportOutcome> {
    const result = await this.#d.store.importText(text);
    if (!result.ok) {
      this.#importErrors = result.errors;
      return result;
    }
    this.#importErrors = [];
    this.#id = result.profile.id;
    await this.#d.store.setActive(result.profile.id);
    this.#d.onProfile(result.profile);
    return { ok: true };
  }

  clearImportErrors(): void {
    this.#importErrors = [];
  }

  // -- calibration ---------------------------------------------------------------------------

  /** The calibration in progress: the device, and the wizard's step. */
  get calibrating(): { device: string; step: CalibrationWizard['step'] } | undefined {
    return this.#wizard === undefined
      ? undefined
      : { device: this.#wizard.device, step: this.#wizard.wizard.step };
  }

  /** Starts calibrating the device in slot `slot`. */
  beginCalibration(slot: number): boolean {
    const s = this.#d.hub.slots[slot];
    if (s?.present !== true) return false;
    this.#wizard = { device: s.device, wizard: new CalibrationWizard(), slot };
    return true;
  }

  /** Feeds the wizard the device's current reading. Call it a few times a second. */
  sampleCalibration(): void {
    const w = this.#wizard;
    const slot = w === undefined ? undefined : this.#d.hub.slots[w.slot];
    if (w !== undefined && slot !== undefined) w.wizard.update(slot);
  }

  /** The next step; after the last one the results are stored in the profile. */
  advanceCalibration(): void {
    const w = this.#wizard;
    if (w === undefined) return;
    const step = w.wizard.next();
    if (step === 'done') {
      this.#edit((p) => applyCalibration(p, w.device, w.wizard));
      this.#wizard = undefined;
    }
  }

  cancelCalibration(): void {
    this.#wizard = undefined;
  }
}
