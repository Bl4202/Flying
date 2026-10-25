/** Fake gamepads for the UI tests, shaped as the Gamepad API gives them. */
import { type GamepadLike } from '@flying/input';

export interface PadSpec {
  index?: number;
  id?: string;
  mapping?: string;
  axes?: Record<number, number>;
  axisCount?: number;
  buttons?: Record<number, number>;
  buttonCount?: number;
}

export const PAD_ID = 'Xbox Wireless Controller (STANDARD GAMEPAD Vendor: 045e Product: 0b13)';
export const STICK_ID = 'T.16000M (Vendor: 044f Product: b10a)';
export const HAT_CENTRED = 1.2857142857142856;

export function fakePad(spec: PadSpec = {}): GamepadLike {
  const mapping = spec.mapping ?? 'standard';
  const axisCount = spec.axisCount ?? 4;
  const buttonCount = spec.buttonCount ?? (mapping === 'standard' ? 17 : 12);
  return {
    connected: true,
    mapping,
    index: spec.index ?? 0,
    id: spec.id ?? (mapping === 'standard' ? PAD_ID : STICK_ID),
    timestamp: 0,
    axes: Array.from({ length: axisCount }, (_, i) => spec.axes?.[i] ?? 0),
    buttons: Array.from({ length: buttonCount }, (_, i) => {
      const value = spec.buttons?.[i] ?? 0;
      return { pressed: value > 0.5, value };
    }),
  };
}

/** A raw joystick with a hat on axis 9. */
export function fakeStick(spec: PadSpec = {}): GamepadLike {
  return fakePad({
    mapping: '',
    axisCount: 10,
    ...spec,
    axes: { 9: HAT_CENTRED, ...spec.axes },
  });
}

export class FakePads {
  list: (GamepadLike | null)[] = [];
  readonly get = (): readonly (GamepadLike | null)[] => this.list;

  set(index: number, pad: GamepadLike | null): void {
    while (this.list.length <= index) this.list.push(null);
    this.list[index] = pad;
  }
}
