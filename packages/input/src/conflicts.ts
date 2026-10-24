/**
 * Conflicts between bindings, and warnings about keys the browser keeps for itself
 * (GDD 58.3, 58.5, AC-58.5).
 *
 * A conflict is two bindings that one press or movement would set off together: the same control
 * bound to more than one action, or twice to the same action. The check covers every pair, in any
 * profile, imported or built by hand.
 */
import { type ActionId } from './actions.ts';
import { type Binding, type ControlsProfile } from './profile.ts';
import { sourceKey, type Source } from './sources.ts';

export type ConflictKind =
  /** One control, several actions. */
  | 'shared'
  /** One control bound to the same action twice. */
  | 'duplicate'
  /** A whole axis and a half of the same axis. */
  | 'overlap';

export interface Conflict {
  kind: ConflictKind;
  /** The control (the first binding's source). */
  source: Source;
  /** Indices into `profile.bindings`, ascending. */
  bindings: number[];
  /** The actions involved, in binding order, without repeats. */
  actions: ActionId[];
}

function make(kind: ConflictKind, p: ControlsProfile, indices: number[]): Conflict {
  const actions: ActionId[] = [];
  for (const i of indices) {
    const a = p.bindings[i]?.action;
    if (a !== undefined && !actions.includes(a)) actions.push(a);
  }
  const first = p.bindings[indices[0] ?? 0];
  return {
    kind,
    source: first?.source ?? { kind: 'key', code: '' },
    bindings: indices,
    actions,
  };
}

/** The key of the physical control behind a source, ignoring which half of an axis is used. */
function controlKey(s: Source): string {
  return s.kind === 'pad-axis' ? `${s.device}:a:${String(s.index)}` : sourceKey(s);
}

/** Every conflict in a profile, in the order the offending bindings first appear. */
export function findConflicts(p: ControlsProfile): Conflict[] {
  const out: Conflict[] = [];

  // The same source more than once.
  const bySource = new Map<string, number[]>();
  p.bindings.forEach((b, i) => {
    const k = sourceKey(b.source);
    const list = bySource.get(k);
    if (list === undefined) bySource.set(k, [i]);
    else list.push(i);
  });
  for (const indices of bySource.values()) {
    if (indices.length < 2) continue;
    const c = make('shared', p, indices);
    out.push(c.actions.length === 1 ? { ...c, kind: 'duplicate' } : c);
  }

  // A whole axis alongside a half of it (two halves, one each way, are fine).
  const byAxis = new Map<string, number[]>();
  p.bindings.forEach((b, i) => {
    if (b.source.kind !== 'pad-axis') return;
    const k = controlKey(b.source);
    const list = byAxis.get(k);
    if (list === undefined) byAxis.set(k, [i]);
    else list.push(i);
  });
  for (const indices of byAxis.values()) {
    const whole = indices.filter((i) => {
      const s = p.bindings[i]?.source;
      return s?.kind === 'pad-axis' && s.half === undefined;
    });
    const halves = indices.filter((i) => !whole.includes(i));
    if (whole.length > 0 && halves.length > 0) {
      out.push(
        make(
          'overlap',
          p,
          [...whole, ...halves].sort((a, b) => a - b),
        ),
      );
    }
  }

  return out;
}

/** The bindings (by index) that take part in any conflict, for the editor to tag. */
export function conflictedBindings(conflicts: readonly Conflict[]): Set<number> {
  const out = new Set<number>();
  for (const c of conflicts) for (const i of c.bindings) out.add(i);
  return out;
}

export type ShortcutReason = 'function-key' | 'os-key' | 'browser-chord';

export interface ShortcutWarning {
  /** Index into `profile.bindings`. */
  binding: number;
  action: ActionId;
  code: string;
  reason: ShortcutReason;
}

/** Keys the browser or the OS acts on whatever the page does. */
const FUNCTION_KEYS: ReadonlySet<string> = new Set([
  'F1',
  'F3',
  'F5',
  'F6',
  'F7',
  'F10',
  'F11',
  'F12',
]);
const OS_KEYS: ReadonlySet<string> = new Set(['MetaLeft', 'MetaRight', 'ContextMenu']);
/** Modifiers whose use turns letters into browser shortcuts (Ctrl+W closes the tab). */
const CHORD_MODIFIERS: ReadonlySet<string> = new Set([
  'ControlLeft',
  'ControlRight',
  'AltLeft',
  'AltRight',
]);
const CTRL_SHORTCUTS: ReadonlySet<string> = new Set([
  ...'ABDEFGHJKLNOPQRSTUW'.split('').map((c) => `Key${c}`),
  'Tab',
  'PageUp',
  'PageDown',
  ...Array.from({ length: 9 }, (_, i) => `Digit${String(i + 1)}`),
  'ArrowLeft',
  'ArrowRight',
  'Home',
]);

/**
 * Warnings for bindings that clash with the browser (GDD 58.3). Function keys and OS keys always
 * warn. Where the profile binds Ctrl or Alt (the default throttle-down is Ctrl), a letter that is
 * also a browser shortcut warns: Ctrl+W closes the tab.
 */
export function reservedShortcutWarnings(p: ControlsProfile): ShortcutWarning[] {
  const keyed = (b: Binding): string | undefined =>
    b.source.kind === 'key' ? b.source.code : undefined;
  const modifierBound = p.bindings.some((b) => CHORD_MODIFIERS.has(keyed(b) ?? ''));
  const out: ShortcutWarning[] = [];
  p.bindings.forEach((b, i) => {
    const code = keyed(b);
    if (code === undefined) return;
    let reason: ShortcutReason | undefined;
    if (FUNCTION_KEYS.has(code)) reason = 'function-key';
    else if (OS_KEYS.has(code)) reason = 'os-key';
    else if (modifierBound && CTRL_SHORTCUTS.has(code)) reason = 'browser-chord';
    if (reason !== undefined) out.push({ binding: i, action: b.action, code, reason });
  });
  return out;
}
