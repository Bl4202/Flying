/**
 * Gamepad polling over the Gamepad API (GDD 58.1, 58.2). Only pads with the `standard` mapping are
 * used in this version; others (joysticks, throttles, pedals) are noted and ignored until the
 * profile-based support arrives.
 *
 * `navigator.getGamepads()` exists only on the main thread, so this is polled there: each animation
 * frame and on a short timer while the tab has focus.
 */

/** Indices of the standard mapping. */
export const PAD_AXIS = { LEFT_X: 0, LEFT_Y: 1, RIGHT_X: 2, RIGHT_Y: 3 } as const;
export const PAD_BUTTON = {
  LEFT_TRIGGER: 6,
  RIGHT_TRIGGER: 7,
  BACK: 8,
  START: 9,
  DPAD_UP: 12,
  DPAD_DOWN: 13,
} as const;

/** The part of a `Gamepad` this module reads. */
export interface GamepadLike {
  readonly connected: boolean;
  readonly mapping: string;
  readonly index: number;
  readonly id: string;
  readonly axes: readonly number[];
  readonly buttons: readonly { readonly pressed: boolean; readonly value: number }[];
}

export interface GamepadSample {
  index: number;
  id: string;
  axes: readonly number[];
  /** Button values, 0 to 1 (analogue triggers report in between). */
  buttons: readonly number[];
}

export class GamepadReader {
  readonly #get: () => readonly (GamepadLike | null)[];
  #ignored = new Set<string>();
  #sample: { index: number; id: string; axes: number[]; buttons: number[] } = {
    index: -1,
    id: '',
    axes: [],
    buttons: [],
  };

  constructor(getGamepads: () => readonly (GamepadLike | null)[] = () => navigator.getGamepads()) {
    this.#get = getGamepads;
  }

  /** Ids of connected devices that were skipped for not having the standard mapping. */
  get ignored(): string[] {
    return [...this.#ignored];
  }

  /**
   * The first connected standard-mapping pad, or `null`. The returned object is reused by the next
   * call, so copy anything that must outlive it.
   */
  poll(): GamepadSample | null {
    let pads: readonly (GamepadLike | null)[];
    try {
      pads = this.#get();
    } catch {
      return null; // some browsers throw when the page is not focused or not secure
    }
    for (const pad of pads) {
      if (!pad?.connected) continue;
      if (pad.mapping !== 'standard') {
        this.#ignored.add(pad.id);
        continue;
      }
      const s = this.#sample;
      s.index = pad.index;
      s.id = pad.id;
      s.axes.length = pad.axes.length;
      for (let i = 0; i < pad.axes.length; i++) s.axes[i] = pad.axes[i] ?? 0;
      s.buttons.length = pad.buttons.length;
      for (let i = 0; i < pad.buttons.length; i++) s.buttons[i] = pad.buttons[i]?.value ?? 0;
      return s;
    }
    return null;
  }
}
