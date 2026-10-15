/**
 * What the player's controls are bound to (GDD 58.5). Data, not code: the binding editor of a
 * later version edits this, and the pump only reads it.
 *
 * Conventions follow the stick of a real aircraft: pitch +1 is nose up (stick back), roll +1 is
 * right wing down (stick right), yaw +1 is nose right. So `W` (push forward) is nose down and `S`
 * is nose up. Pitch trim runs while held: `]` nose up, `[` nose down; on a pad the d-pad follows
 * the stick (down is nose up).
 *
 * A note on Ctrl. The GDD's default for "throttle down" is Ctrl. In browsers on Windows and Linux,
 * Ctrl+W closes the tab, Ctrl+R reloads and Ctrl+S saves, and pages cannot stop that; holding Ctrl
 * and pressing W (pitch) or R (reset) therefore does something other than flying. Throttle also
 * answers to Page Up and Page Down, which have no such side effects, and the binding editor will
 * warn about reserved combinations (GDD 58.3).
 */
import { type AxisEmulation, DEFAULT_EMULATION } from './keyboard.ts';
import { type AxisShape, LINEAR_SHAPE, STICK_SHAPE } from './shaping.ts';
import { PAD_AXIS, PAD_BUTTON } from './gamepad.ts';

export interface KeyPair {
  /** Keys that push the axis towards -1. */
  negative: readonly string[];
  /** Keys that push the axis towards +1. */
  positive: readonly string[];
}

export interface KeyboardBindings {
  pitch: KeyPair;
  roll: KeyPair;
  yaw: KeyPair;
  throttleUp: readonly string[];
  throttleDown: readonly string[];
  reset: readonly string[];
  /** Held: pitch trim moves at 2 degrees of elevator per second (GDD 11.11). */
  trimNoseUp: readonly string[];
  trimNoseDown: readonly string[];
}

export interface PadAxisBinding {
  /** Index in the standard mapping. */
  axis: number;
  shape: AxisShape;
}

export interface GamepadBindings {
  roll: PadAxisBinding;
  pitch: PadAxisBinding;
  /** Yaw is the right trigger minus the left. */
  yaw: { negativeButton: number; positiveButton: number; shape: AxisShape };
  /** The throttle lever moves while the stick is held off centre. */
  throttle: { axis: number; invert: boolean; ratePerSecond: number; shape: AxisShape };
  reset: { button: number };
  trimNoseUp: { button: number };
  trimNoseDown: { button: number };
}

export interface Bindings {
  keyboard: KeyboardBindings;
  gamepad: GamepadBindings;
  keyboardAxis: AxisEmulation;
  /** How fast a held throttle key moves the lever, fraction of travel per second. */
  throttleKeyRatePerSecond: number;
}

export const DEFAULT_BINDINGS: Bindings = {
  keyboard: {
    pitch: { negative: ['KeyW'], positive: ['KeyS'] },
    roll: { negative: ['KeyA'], positive: ['KeyD'] },
    yaw: { negative: ['KeyQ'], positive: ['KeyE'] },
    throttleUp: ['ShiftLeft', 'ShiftRight', 'PageUp'],
    throttleDown: ['ControlLeft', 'ControlRight', 'PageDown'],
    reset: ['KeyR'],
    trimNoseUp: ['BracketRight'],
    trimNoseDown: ['BracketLeft'],
  },
  gamepad: {
    roll: { axis: PAD_AXIS.RIGHT_X, shape: STICK_SHAPE },
    pitch: { axis: PAD_AXIS.RIGHT_Y, shape: STICK_SHAPE },
    yaw: {
      negativeButton: PAD_BUTTON.LEFT_TRIGGER,
      positiveButton: PAD_BUTTON.RIGHT_TRIGGER,
      shape: LINEAR_SHAPE,
    },
    throttle: { axis: PAD_AXIS.LEFT_Y, invert: true, ratePerSecond: 1, shape: LINEAR_SHAPE },
    reset: { button: PAD_BUTTON.BACK },
    trimNoseUp: { button: PAD_BUTTON.DPAD_DOWN },
    trimNoseDown: { button: PAD_BUTTON.DPAD_UP },
  },
  keyboardAxis: DEFAULT_EMULATION,
  throttleKeyRatePerSecond: 0.5,
};

/** Every key code the bindings use, so the page can stop the browser acting on them. */
export function boundKeyCodes(b: KeyboardBindings): Set<string> {
  return new Set([
    ...b.pitch.negative,
    ...b.pitch.positive,
    ...b.roll.negative,
    ...b.roll.positive,
    ...b.yaw.negative,
    ...b.yaw.positive,
    ...b.throttleUp,
    ...b.throttleDown,
    ...b.reset,
    ...b.trimNoseUp,
    ...b.trimNoseDown,
  ]);
}

/** Key codes bound to more than one action, with the actions (for the binding editor's warnings). */
export function keyConflicts(b: KeyboardBindings): Map<string, string[]> {
  const uses = new Map<string, string[]>();
  const add = (action: string, codes: readonly string[]): void => {
    for (const c of codes) uses.set(c, [...(uses.get(c) ?? []), action]);
  };
  add('pitch-', b.pitch.negative);
  add('pitch+', b.pitch.positive);
  add('roll-', b.roll.negative);
  add('roll+', b.roll.positive);
  add('yaw-', b.yaw.negative);
  add('yaw+', b.yaw.positive);
  add('throttle+', b.throttleUp);
  add('throttle-', b.throttleDown);
  add('reset', b.reset);
  add('trim+', b.trimNoseUp);
  add('trim-', b.trimNoseDown);
  return new Map([...uses].filter(([, actions]) => actions.length > 1));
}
