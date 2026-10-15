import { describe, expect, it } from 'vitest';
import { boundKeyCodes, DEFAULT_BINDINGS, keyConflicts } from './bindings.ts';
import { PAD_AXIS, PAD_BUTTON } from './gamepad.ts';

describe('default bindings (GDD 58.5)', () => {
  const k = DEFAULT_BINDINGS.keyboard;

  it('puts pitch, roll and yaw on W/S, A/D and Q/E', () => {
    expect([k.pitch.negative, k.pitch.positive]).toEqual([['KeyW'], ['KeyS']]);
    expect([k.roll.negative, k.roll.positive]).toEqual([['KeyA'], ['KeyD']]);
    expect([k.yaw.negative, k.yaw.positive]).toEqual([['KeyQ'], ['KeyE']]);
  });

  it('puts throttle on Shift and Ctrl, with Page Up and Down as keys that do not collide with browser shortcuts', () => {
    expect(k.throttleUp).toEqual(expect.arrayContaining(['ShiftLeft', 'ShiftRight', 'PageUp']));
    expect(k.throttleDown).toEqual(
      expect.arrayContaining(['ControlLeft', 'ControlRight', 'PageDown']),
    );
    expect(k.reset).toEqual(['KeyR']);
    expect([k.trimNoseUp, k.trimNoseDown]).toEqual([['BracketRight'], ['BracketLeft']]);
  });

  it('puts the pad on the right stick, the triggers and the left stick', () => {
    const g = DEFAULT_BINDINGS.gamepad;
    expect(g.pitch.axis).toBe(PAD_AXIS.RIGHT_Y);
    expect(g.roll.axis).toBe(PAD_AXIS.RIGHT_X);
    expect([g.yaw.negativeButton, g.yaw.positiveButton]).toEqual([
      PAD_BUTTON.LEFT_TRIGGER,
      PAD_BUTTON.RIGHT_TRIGGER,
    ]);
    expect(g.throttle.axis).toBe(PAD_AXIS.LEFT_Y);
    expect(g.throttle.invert).toBe(true); // stick up raises the throttle
    expect(g.reset.button).toBe(PAD_BUTTON.BACK);
  });

  it('uses the documented stick shape on pitch and roll and a linear one on the rest', () => {
    const g = DEFAULT_BINDINGS.gamepad;
    expect(g.pitch.shape.curve).toEqual({ kind: 'expo', k: 0.3 });
    expect(g.roll.shape.deadCentre).toBe(0.03);
    expect(g.yaw.shape.curve).toEqual({ kind: 'expo', k: 0 });
    expect(g.throttle.shape.curve).toEqual({ kind: 'expo', k: 0 });
  });

  it('has no key bound to two actions', () => {
    expect([...keyConflicts(k)]).toEqual([]);
  });

  it('lists every bound key for the page to claim', () => {
    const codes = boundKeyCodes(k);
    for (const c of [
      'KeyW',
      'KeyS',
      'KeyA',
      'KeyD',
      'KeyQ',
      'KeyE',
      'ShiftLeft',
      'ControlLeft',
      'KeyR',
      'PageUp',
    ]) {
      expect(codes.has(c)).toBe(true);
    }
    expect(codes.has('KeyP')).toBe(false);
  });

  it('keyConflicts finds duplicates', () => {
    const dup = { ...k, reset: ['KeyW'] };
    expect([...keyConflicts(dup)]).toEqual([['KeyW', ['pitch-', 'reset']]]);
  });

  it('keeps the documented keyboard rates', () => {
    expect(DEFAULT_BINDINGS.keyboardAxis).toEqual({ rampUpPerSecond: 3, returnPerSecond: 4 });
    expect(DEFAULT_BINDINGS.throttleKeyRatePerSecond).toBeGreaterThan(0);
  });
});
