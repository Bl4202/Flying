import { describe, expect, it } from 'vitest';
import { GamepadReader, PAD_AXIS, PAD_BUTTON, type GamepadLike } from './gamepad.ts';

const pad = (extra: Partial<GamepadLike> = {}): GamepadLike => ({
  connected: true,
  mapping: 'standard',
  index: 0,
  id: 'Test Pad (STANDARD GAMEPAD)',
  axes: [0, 0, 0.5, -0.25],
  buttons: Array.from({ length: 17 }, (_, i) => ({
    pressed: i === 8,
    value: i === 7 ? 0.75 : i === 8 ? 1 : 0,
  })),
  ...extra,
});

describe('GamepadReader', () => {
  it('reads axes and analogue button values from a standard pad', () => {
    const r = new GamepadReader(() => [pad()]);
    const s = r.poll();
    expect(s?.index).toBe(0);
    expect(s?.axes[PAD_AXIS.RIGHT_X]).toBe(0.5);
    expect(s?.axes[PAD_AXIS.RIGHT_Y]).toBe(-0.25);
    expect(s?.buttons[PAD_BUTTON.RIGHT_TRIGGER]).toBe(0.75);
    expect(s?.buttons[PAD_BUTTON.BACK]).toBe(1);
  });

  it('is empty with no pad, a null slot, or a disconnected one', () => {
    expect(new GamepadReader(() => []).poll()).toBeNull();
    expect(new GamepadReader(() => [null, null]).poll()).toBeNull();
    expect(new GamepadReader(() => [pad({ connected: false })]).poll()).toBeNull();
  });

  it('skips pads without the standard mapping and remembers them', () => {
    const joystick = pad({ mapping: '', id: 'Acme Stick (Vendor: 0001)', index: 0 });
    const real = pad({ index: 1, id: 'Real Pad' });
    const r = new GamepadReader(() => [joystick, real]);
    expect(r.poll()?.id).toBe('Real Pad');
    expect(r.ignored).toEqual(['Acme Stick (Vendor: 0001)']);
    expect(new GamepadReader(() => [joystick]).poll()).toBeNull();
  });

  it('takes the first usable pad', () => {
    const r = new GamepadReader(() => [
      null,
      pad({ index: 1, id: 'one' }),
      pad({ index: 2, id: 'two' }),
    ]);
    expect(r.poll()?.id).toBe('one');
  });

  it('survives getGamepads throwing', () => {
    const r = new GamepadReader(() => {
      throw new Error('not allowed');
    });
    expect(r.poll()).toBeNull();
  });

  it('follows a pad that changes between polls', () => {
    let rx = 0;
    const r = new GamepadReader(() => [pad({ axes: [0, 0, rx, 0] })]);
    expect(r.poll()?.axes[2]).toBe(0);
    rx = 1;
    expect(r.poll()?.axes[2]).toBe(1);
  });
});
