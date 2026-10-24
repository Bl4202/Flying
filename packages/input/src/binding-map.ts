/**
 * A profile compiled for the pump (ADR-37).
 *
 * The pump reads the bindings every few milliseconds, so they are laid out as flat typed arrays
 * grouped by action: `first(a)` to `last(a)` are the bindings of action `a`, and `value(i, ...)`
 * reads binding `i` from the devices. Nothing here allocates after `compileBindings`, which also
 * collects the key codes the page must claim from the browser.
 *
 * Pad sources name a device class; `resolve(hub)` turns that into the slot of a connected device
 * (and its calibration). It runs again whenever the hub's `revision` changes, so a device that is
 * plugged back in, or on another port, picks its bindings up again.
 */
import { ACTION_COUNT, actionIndex } from './actions.ts';
import { HAT_BIT, type GamepadHub, type PadSlot } from './devices/gamepad.ts';
import { type MouseSource } from './devices/mouse.ts';
import { type KeyboardState } from './keyboard.ts';
import { calibrationKey, type ControlsProfile } from './profile.ts';
import { calibrateScalar } from './shaping.ts';

export const SRC = {
  KEY: 0,
  MOUSE_BUTTON: 1,
  WHEEL_UP: 2,
  WHEEL_DOWN: 3,
  PAD_BUTTON: 4,
  PAD_AXIS: 5,
  HAT: 6,
} as const;

/** What the bindings read. */
export interface Devices {
  keyboard: KeyboardState;
  mouse: MouseSource | undefined;
  pads: GamepadHub | undefined;
}

export class BindingMap {
  readonly profile: ControlsProfile;
  readonly count: number;
  /** Where `read` leaves its result, in element 0. */
  readonly out = new Float64Array(1);
  /** Key codes the profile uses, for the page to stop the browser acting on them. */
  readonly keyCodes: ReadonlySet<string>;
  readonly #start: Int32Array;
  readonly #src: Uint8Array;
  readonly #code: string[];
  readonly #index: Int16Array;
  readonly #dir: Int8Array;
  readonly #half: Int8Array;
  readonly #invert: Uint8Array;
  readonly #threshold: Float64Array;
  readonly #hatBit: Uint8Array;
  readonly #device: string[];
  readonly #slot: Int8Array;
  readonly #calMin: Float64Array;
  readonly #calCentre: Float64Array;
  readonly #calMax: Float64Array;

  constructor(profile: ControlsProfile) {
    this.profile = profile;
    const n = profile.bindings.length;
    this.count = n;
    this.#start = new Int32Array(ACTION_COUNT + 1);
    this.#src = new Uint8Array(n);
    this.#code = new Array<string>(n).fill('');
    this.#index = new Int16Array(n);
    this.#dir = new Int8Array(n).fill(1);
    this.#half = new Int8Array(n);
    this.#invert = new Uint8Array(n);
    this.#threshold = new Float64Array(n).fill(0.5);
    this.#hatBit = new Uint8Array(n);
    this.#device = new Array<string>(n).fill('');
    this.#slot = new Int8Array(n).fill(-1);
    this.#calMin = new Float64Array(n).fill(-1);
    this.#calCentre = new Float64Array(n);
    this.#calMax = new Float64Array(n).fill(1);

    // Group by action (a counting sort, keeping the profile's order within an action).
    const counts = new Int32Array(ACTION_COUNT);
    for (const b of profile.bindings) {
      const a = actionIndex(b.action);
      if (a >= 0) counts[a] = (counts[a] ?? 0) + 1;
    }
    let total = 0;
    for (let a = 0; a < ACTION_COUNT; a++) {
      this.#start[a] = total;
      total += counts[a] ?? 0;
    }
    this.#start[ACTION_COUNT] = total;
    const fill = this.#start.slice(0, ACTION_COUNT);
    const keys = new Set<string>();
    for (const b of profile.bindings) {
      const a = actionIndex(b.action);
      if (a < 0) continue;
      const i = fill[a] ?? 0;
      fill[a] = i + 1;
      const s = b.source;
      this.#dir[i] = b.direction ?? 1;
      this.#invert[i] = b.invert === true ? 1 : 0;
      this.#threshold[i] = b.threshold ?? 0.5;
      switch (s.kind) {
        case 'key':
          this.#src[i] = SRC.KEY;
          this.#code[i] = s.code;
          keys.add(s.code);
          break;
        case 'mouse-button':
          this.#src[i] = SRC.MOUSE_BUTTON;
          this.#index[i] = s.button;
          break;
        case 'wheel':
          this.#src[i] = s.dir === 'up' ? SRC.WHEEL_UP : SRC.WHEEL_DOWN;
          break;
        case 'pad-button':
          this.#src[i] = SRC.PAD_BUTTON;
          this.#index[i] = s.index;
          this.#device[i] = s.device;
          break;
        case 'pad-axis':
          this.#src[i] = SRC.PAD_AXIS;
          this.#index[i] = s.index;
          this.#device[i] = s.device;
          this.#half[i] = s.half === undefined ? 0 : s.half === 'positive' ? 1 : -1;
          break;
        case 'hat':
          this.#src[i] = SRC.HAT;
          this.#index[i] = s.hat;
          this.#device[i] = s.device;
          this.#hatBit[i] = HAT_BIT[s.dir];
          break;
      }
    }
    this.keyCodes = keys;
  }

  /** Looks the pad sources up among the connected devices. Call again when `hub.revision` moves. */
  resolve(hub: GamepadHub | undefined): void {
    for (let i = 0; i < this.count; i++) {
      const src = this.#src[i];
      if (src !== SRC.PAD_BUTTON && src !== SRC.PAD_AXIS && src !== SRC.HAT) continue;
      const slot = hub === undefined ? -1 : hub.find(this.#device[i] ?? '');
      this.#slot[i] = slot;
      if (src !== SRC.PAD_AXIS) continue;
      let min = -1;
      let centre = 0;
      let max = 1;
      const device = hub?.slots[slot]?.device;
      const cal =
        (device === undefined
          ? undefined
          : this.profile.calibration[calibrationKey(device, this.#index[i] ?? 0)]) ??
        this.profile.calibration[calibrationKey(this.#device[i] ?? '', this.#index[i] ?? 0)];
      if (cal !== undefined) {
        min = cal.min;
        centre = cal.centre;
        max = cal.max;
      }
      this.#calMin[i] = min;
      this.#calCentre[i] = centre;
      this.#calMax[i] = max;
    }
  }

  /** The first binding of action `a` (an index into `ACTIONS`). */
  first(a: number): number {
    return this.#start[a] ?? 0;
  }

  /** One past the last binding of action `a`. */
  last(a: number): number {
    return this.#start[a + 1] ?? 0;
  }

  /** True for sources that are on or off (keys, mouse buttons, wheel, hats). */
  isDigital(i: number): boolean {
    const s = this.#src[i];
    return s !== SRC.PAD_BUTTON && s !== SRC.PAD_AXIS;
  }

  isWheel(i: number): boolean {
    const s = this.#src[i];
    return s === SRC.WHEEL_UP || s === SRC.WHEEL_DOWN;
  }

  /** +1 or -1: which way a key, button or hat pushes an axis, or a reading's sign. */
  direction(i: number): number {
    return this.#dir[i] ?? 1;
  }

  /** The reading above which an analogue source counts as pressed. */
  threshold(i: number): number {
    return this.#threshold[i] ?? 0.5;
  }

  /** True while the device behind a pad source is connected. */
  present(i: number): boolean {
    return (this.#slot[i] ?? -1) >= 0;
  }

  /**
   * Reads binding `i` into `out[0]`: 0 or 1 for a digital source (the number of notches for the
   * wheel), 0 to 1 for a pad button, and for a pad axis -1 to 1 (or 0 to 1 on a half), after
   * calibration and the binding's own inversion. A source on a missing device reads 0.
   *
   * The result goes into an array rather than a return value because the pump calls this for every
   * binding every few milliseconds, and a fractional number returned across a call the compiler
   * does not inline is a heap allocation.
   */
  read(i: number, d: Devices): void {
    const out = this.out;
    switch (this.#src[i]) {
      case SRC.KEY:
        out[0] = d.keyboard.isDown(this.#code[i] ?? '') ? 1 : 0;
        return;
      case SRC.MOUSE_BUTTON:
        out[0] = d.mouse?.isDown(this.#index[i] ?? 0) === true ? 1 : 0;
        return;
      case SRC.WHEEL_UP:
        out[0] = d.mouse?.wheelUp ?? 0;
        return;
      case SRC.WHEEL_DOWN:
        out[0] = d.mouse?.wheelDown ?? 0;
        return;
      case SRC.PAD_BUTTON: {
        const slot = this.#padSlot(i, d);
        out[0] = slot?.buttons[this.#index[i] ?? 0] ?? 0;
        return;
      }
      case SRC.PAD_AXIS: {
        const slot = this.#padSlot(i, d);
        if (slot === undefined) {
          out[0] = 0;
          return;
        }
        const raw = slot.axes[this.#index[i] ?? 0] ?? 0;
        let x = calibrateScalar(
          raw,
          this.#calMin[i] ?? -1,
          this.#calCentre[i] ?? 0,
          this.#calMax[i] ?? 1,
        );
        if (this.#invert[i] === 1) x = -x;
        const half = this.#half[i];
        if (half === 1) x = Math.max(0, x);
        else if (half === -1) x = Math.max(0, -x);
        out[0] = x + 0;
        return;
      }
      case SRC.HAT: {
        const slot = this.#padSlot(i, d);
        out[0] = ((slot?.hats[this.#index[i] ?? 0] ?? 0) & (this.#hatBit[i] ?? 0)) !== 0 ? 1 : 0;
        return;
      }
      default:
        out[0] = 0;
    }
  }

  /** `read` as a return value, for tests and the editor. */
  value(i: number, d: Devices): number {
    this.read(i, d);
    return this.out[0] ?? 0;
  }

  /**
   * `read` for an action that runs from 0 to 1 (the throttle, a brake): a whole pad axis maps its
   * -1 to 1 travel onto 0 to 1, as a lever does.
   */
  readUnipolar(i: number, d: Devices): void {
    this.read(i, d);
    if (this.#src[i] === SRC.PAD_AXIS && this.#half[i] === 0) {
      this.out[0] = ((this.out[0] ?? 0) + 1) / 2;
    }
  }

  /** `readUnipolar` as a return value, for tests and the editor. */
  unipolar(i: number, d: Devices): number {
    this.readUnipolar(i, d);
    return this.out[0] ?? 0;
  }

  /** True if binding `i` is pressed: its reading is past the threshold (either way on a whole axis). */
  active(i: number, d: Devices): boolean {
    this.read(i, d);
    return Math.abs(this.out[0] ?? 0) > (this.#threshold[i] ?? 0.5);
  }

  /** The slot behind a pad source, or `undefined` (a negative index would be a slow lookup). */
  #padSlot(i: number, d: Devices): PadSlot | undefined {
    const slot = this.#slot[i] ?? -1;
    return slot < 0 ? undefined : d.pads?.slots[slot];
  }
}

/** Compiles a profile. */
export function compileBindings(profile: ControlsProfile): BindingMap {
  return new BindingMap(profile);
}
