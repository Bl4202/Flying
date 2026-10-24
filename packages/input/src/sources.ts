/**
 * Physical inputs a binding can name (ADR-37).
 *
 * A source is one control on one kind of device. Devices are named by class, not by the slot the
 * browser happened to give them: `pad` is any pad with the `standard` mapping, `joy:046d:c215` a
 * raw joystick by vendor and product, `joy` any raw joystick. Plugging a device into another port
 * therefore keeps its bindings.
 */

export type Half = 'positive' | 'negative';
export type HatDirection = 'up' | 'right' | 'down' | 'left';

export type Source =
  | { readonly kind: 'key'; readonly code: string }
  | { readonly kind: 'mouse-button'; readonly button: number }
  | { readonly kind: 'wheel'; readonly dir: 'up' | 'down' }
  | {
      readonly kind: 'pad-button';
      readonly device: string;
      readonly index: number;
    }
  | {
      readonly kind: 'pad-axis';
      readonly device: string;
      readonly index: number;
      /** Used as a button or a one-sided axis: only this half counts. */
      readonly half?: Half;
    }
  | {
      readonly kind: 'hat';
      readonly device: string;
      readonly hat: number;
      readonly dir: HatDirection;
    };

export type SourceKind = Source['kind'];

/** The device a source belongs to, as a class: `keyboard`, `mouse`, or a pad or joystick name. */
export function sourceDevice(s: Source): string {
  switch (s.kind) {
    case 'key':
      return 'keyboard';
    case 'mouse-button':
    case 'wheel':
      return 'mouse';
    default:
      return s.device;
  }
}

/**
 * A string that is the same for the same control and different for different ones. Two bindings
 * with the same key in the same context are a conflict (AC-58.5).
 */
export function sourceKey(s: Source): string {
  switch (s.kind) {
    case 'key':
      return `key:${s.code}`;
    case 'mouse-button':
      return `mouse:${String(s.button)}`;
    case 'wheel':
      return `wheel:${s.dir}`;
    case 'pad-button':
      return `${s.device}:b:${String(s.index)}`;
    case 'pad-axis':
      return `${s.device}:a:${String(s.index)}${s.half === undefined ? '' : s.half === 'positive' ? '+' : '-'}`;
    case 'hat':
      return `${s.device}:h:${String(s.hat)}:${s.dir}`;
  }
}

/** True if two sources are the same control. */
export function sameSource(a: Source, b: Source): boolean {
  return sourceKey(a) === sourceKey(b);
}

/** What the player sees for a source: a short label for the editor (not translated text). */
export function describeSource(s: Source): string {
  switch (s.kind) {
    case 'key':
      return s.code.replace(/^Key/, '').replace(/^Digit/, '');
    case 'mouse-button':
      return `Mouse ${String(s.button + 1)}`;
    case 'wheel':
      return `Wheel ${s.dir}`;
    case 'pad-button':
      return `Button ${String(s.index)}`;
    case 'pad-axis':
      return `Axis ${String(s.index)}${s.half === undefined ? '' : s.half === 'positive' ? '+' : '-'}`;
    case 'hat':
      return `Hat ${String(s.hat)} ${s.dir}`;
  }
}

/** The vendor and product of a gamepad, from the `id` string browsers give, if it has them. */
export interface PadIdentity {
  /** Four lowercase hex digits. */
  vendor: string;
  product: string;
  /** The device name without the identity suffix. */
  name: string;
}

const hex4 = (s: string): string => s.toLowerCase().padStart(4, '0');

/**
 * Reads `Vendor: 045e Product: 028e` (Chrome, Safari) or `045e-028e-Name` (Firefox) from a
 * gamepad id.
 */
export function parsePadId(id: string): PadIdentity | undefined {
  const chrome = /^(.*?)\s*\(.*?Vendor:\s*([0-9a-f]{1,4})\s+Product:\s*([0-9a-f]{1,4})\)\s*$/i.exec(
    id,
  );
  if (chrome?.[2] !== undefined && chrome[3] !== undefined) {
    return {
      vendor: hex4(chrome[2]),
      product: hex4(chrome[3]),
      name: (chrome[1] ?? '').trim() || id,
    };
  }
  const firefox = /^([0-9a-f]{1,4})-([0-9a-f]{1,4})-(.*)$/i.exec(id);
  if (firefox?.[1] !== undefined && firefox[2] !== undefined) {
    return {
      vendor: hex4(firefox[1]),
      product: hex4(firefox[2]),
      name: (firefox[3] ?? '').trim() || id,
    };
  }
  return undefined;
}

/**
 * The device name a pad goes by in bindings: `pad` for a standard-mapping pad, `joy:vendor:product`
 * for a raw device whose identity is known, `joy` otherwise.
 */
export function deviceName(mapping: string, id: string): string {
  if (mapping === 'standard') return 'pad';
  const identity = parsePadId(id);
  return identity === undefined ? 'joy' : `joy:${identity.vendor}:${identity.product}`;
}

/**
 * True if a device name in a binding covers a connected device's name: an exact match, or the
 * generic `joy` for any raw device.
 */
export function deviceMatches(bound: string, actual: string): boolean {
  if (bound === actual) return true;
  return bound === 'joy' && actual.startsWith('joy:');
}
