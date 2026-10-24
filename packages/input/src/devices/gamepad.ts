/**
 * Every gamepad, joystick and throttle the browser shows (GDD 58.1, 58.2, ADR-37).
 *
 * `navigator.getGamepads()` exists only on the main thread, so the hub is polled there: every
 * animation frame and on a short timer while the tab has focus. A poll copies each connected
 * device into a fixed slot (the slot is the browser's gamepad index), so nothing is allocated
 * after start-up: bindings read `slot.axes`, `slot.buttons` and `slot.hats` directly.
 *
 * Devices are known by class (`sources.ts`): a standard-mapping pad is `pad`, a raw device
 * `joy:vendor:product`. When a device appears, disappears or is replaced, `revision` increases and
 * the pump looks its bindings up again; until it does, the missing device simply reads as neutral.
 *
 * Two hat encodings are decoded. A standard pad reports its d-pad as buttons 12 to 15. Many raw
 * sticks report the POV hat as a single axis (`axes[9]`) with eight discrete values; those are
 * decoded into the same direction bits.
 */
import { deviceName, deviceMatches, type HatDirection } from '../sources.ts';

/** Indices of the standard mapping. */
export const PAD_AXIS = { LEFT_X: 0, LEFT_Y: 1, RIGHT_X: 2, RIGHT_Y: 3 } as const;
export const PAD_BUTTON = {
  A: 0,
  B: 1,
  X: 2,
  Y: 3,
  LEFT_BUMPER: 4,
  RIGHT_BUMPER: 5,
  LEFT_TRIGGER: 6,
  RIGHT_TRIGGER: 7,
  BACK: 8,
  START: 9,
  LEFT_STICK: 10,
  RIGHT_STICK: 11,
  DPAD_UP: 12,
  DPAD_DOWN: 13,
  DPAD_LEFT: 14,
  DPAD_RIGHT: 15,
  HOME: 16,
} as const;

/** Direction bits of a hat. A diagonal sets two. */
export const HAT_BIT: Readonly<Record<HatDirection, number>> = {
  up: 1,
  right: 2,
  down: 4,
  left: 8,
};

export const MAX_PADS = 8;
export const MAX_AXES = 32;
export const MAX_BUTTONS = 64;
export const MAX_HATS = 4;

/** The axis a raw device usually carries its POV hat on. */
const HAT_AXIS = 9;

// The eight positions of an axis-encoded hat, clockwise from up: -1 + 2k/7.
const HAT_AXIS_MASKS = [
  HAT_BIT.up,
  HAT_BIT.up | HAT_BIT.right,
  HAT_BIT.right,
  HAT_BIT.right | HAT_BIT.down,
  HAT_BIT.down,
  HAT_BIT.down | HAT_BIT.left,
  HAT_BIT.left,
  HAT_BIT.left | HAT_BIT.up,
] as const;
const HAT_AXIS_TOLERANCE = 0.05;

/**
 * Decodes an axis-encoded hat. Returns the direction bits (0 when centred), or `undefined` if the
 * value is not one a hat produces. Centred reads about 1.29 (Chrome) or 3.29 (Firefox).
 */
export function decodeHatAxis(v: number): number | undefined {
  if (!Number.isFinite(v)) return undefined;
  if (v > 1.1 && v <= 3.4) return 0;
  const k = Math.round((v + 1) * 3.5);
  if (k < 0 || k > 7) return undefined;
  if (Math.abs(v - (-1 + (2 * k) / 7)) > HAT_AXIS_TOLERANCE) return undefined;
  return HAT_AXIS_MASKS[k];
}

/** Direction bits of a hat built from four buttons. */
export function decodeHatButtons(
  up: boolean,
  right: boolean,
  down: boolean,
  left: boolean,
): number {
  return (
    (up ? HAT_BIT.up : 0) |
    (right ? HAT_BIT.right : 0) |
    (down ? HAT_BIT.down : 0) |
    (left ? HAT_BIT.left : 0)
  );
}

/** The part of a `Gamepad` this module reads. */
export interface GamepadLike {
  readonly connected: boolean;
  readonly mapping: string;
  readonly index: number;
  readonly id: string;
  readonly timestamp?: number;
  readonly axes: ArrayLike<number>;
  readonly buttons: ArrayLike<{ readonly pressed: boolean; readonly value: number }>;
}

/** One device's latest reading. Reused by every poll. */
export class PadSlot {
  present = false;
  /** The browser's gamepad index. */
  index = -1;
  id = '';
  /** `pad`, `joy:vendor:product` or `joy`. */
  device = '';
  mapping = '';
  /** `Gamepad.timestamp` of the reading, in element 0: when the device last reported (ms). */
  readonly stamp = new Float64Array(1);
  axisCount = 0;
  buttonCount = 0;
  hatCount = 0;
  /** The axis that carries hat 0 on a raw device, or -1. */
  hatAxis = -1;
  readonly axes = new Float64Array(MAX_AXES);
  readonly buttons = new Float64Array(MAX_BUTTONS);
  readonly hats = new Uint8Array(MAX_HATS);

  get timestamp(): number {
    return this.stamp[0] ?? 0;
  }

  clear(): void {
    this.present = false;
    this.index = -1;
    this.id = '';
    this.device = '';
    this.mapping = '';
    this.stamp[0] = 0;
    this.axisCount = 0;
    this.buttonCount = 0;
    this.hatCount = 0;
    this.hatAxis = -1;
    this.axes.fill(0);
    this.buttons.fill(0);
    this.hats.fill(0);
  }
}

export class GamepadHub {
  readonly slots: readonly PadSlot[] = Array.from({ length: MAX_PADS }, () => new PadSlot());
  readonly #get: () => ArrayLike<GamepadLike | null | undefined>;
  #revision = 0;
  #present = 0;

  constructor(
    getGamepads: () => ArrayLike<GamepadLike | null | undefined> = () => navigator.getGamepads(),
  ) {
    this.#get = getGamepads;
  }

  /** Increases whenever a device appears, disappears or changes identity. */
  get revision(): number {
    return this.#revision;
  }

  /** How many devices are connected. */
  get count(): number {
    return this.#present;
  }

  /** The device names of everything connected (allocates: for the editor and profile choice). */
  deviceNames(): string[] {
    const out: string[] = [];
    for (const s of this.slots) if (s.present) out.push(s.device);
    return out;
  }

  /** The lowest slot whose device a binding to `bound` covers, or -1. */
  find(bound: string): number {
    for (let i = 0; i < this.slots.length; i++) {
      const s = this.slots[i];
      if (s?.present === true && deviceMatches(bound, s.device)) return i;
    }
    return -1;
  }

  /** Reads every device. Nothing is allocated here beyond what `getGamepads()` itself returns. */
  poll(): void {
    let pads: ArrayLike<GamepadLike | null | undefined>;
    try {
      pads = this.#get();
    } catch {
      pads = []; // some browsers throw when the page is not focused or not secure
    }
    let present = 0;
    for (let i = 0; i < this.slots.length; i++) {
      const slot = this.slots[i];
      if (slot === undefined) continue;
      const pad = i < pads.length ? pads[i] : undefined;
      if (pad?.connected !== true) {
        if (slot.present) {
          slot.clear();
          this.#revision += 1;
        }
        continue;
      }
      present += 1;
      if (!slot.present || slot.id !== pad.id || slot.mapping !== pad.mapping) {
        this.#identify(slot, pad);
      }
      this.#read(slot, pad);
    }
    this.#present = present;
  }

  #identify(slot: PadSlot, pad: GamepadLike): void {
    slot.present = true;
    slot.index = pad.index;
    slot.id = pad.id;
    slot.mapping = pad.mapping;
    slot.device = deviceName(pad.mapping, pad.id);
    slot.hatAxis = -1;
    slot.hatCount = 0;
    if (pad.mapping === 'standard') {
      slot.hatCount = 1;
    } else if (pad.axes.length > HAT_AXIS && decodeHatAxis(pad.axes[HAT_AXIS] ?? 0) !== undefined) {
      slot.hatAxis = HAT_AXIS;
      slot.hatCount = 1;
    }
    this.#revision += 1;
  }

  #read(slot: PadSlot, pad: GamepadLike): void {
    slot.stamp[0] = pad.timestamp ?? 0;
    const na = Math.min(pad.axes.length, MAX_AXES);
    slot.axisCount = na;
    for (let i = 0; i < na; i++) {
      const v = pad.axes[i] ?? 0;
      slot.axes[i] = Number.isFinite(v) ? v : 0;
    }
    const nb = Math.min(pad.buttons.length, MAX_BUTTONS);
    slot.buttonCount = nb;
    for (let i = 0; i < nb; i++) {
      const b = pad.buttons[i];
      slot.buttons[i] = b === undefined ? 0 : b.pressed && b.value === 0 ? 1 : b.value;
    }
    if (slot.mapping === 'standard' && nb > PAD_BUTTON.DPAD_RIGHT) {
      slot.hats[0] = decodeHatButtons(
        (slot.buttons[PAD_BUTTON.DPAD_UP] ?? 0) > 0.5,
        (slot.buttons[PAD_BUTTON.DPAD_RIGHT] ?? 0) > 0.5,
        (slot.buttons[PAD_BUTTON.DPAD_DOWN] ?? 0) > 0.5,
        (slot.buttons[PAD_BUTTON.DPAD_LEFT] ?? 0) > 0.5,
      );
    } else if (slot.hatAxis >= 0) {
      slot.hats[0] = decodeHatAxis(slot.axes[slot.hatAxis] ?? 0) ?? 0;
    }
  }
}
