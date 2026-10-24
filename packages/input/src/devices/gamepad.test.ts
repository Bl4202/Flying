import { describe, expect, it } from 'vitest';
import { FakePads, fakePad, fakeStick, HAT_CENTRED, PAD_ID, STICK_ID } from '../test-support.ts';
import {
  decodeHatAxis,
  decodeHatButtons,
  GamepadHub,
  HAT_BIT,
  MAX_AXES,
  MAX_PADS,
  PAD_AXIS,
  PAD_BUTTON,
} from './gamepad.ts';

const hubOf = (pads: FakePads): GamepadHub => new GamepadHub(pads.get);

describe('hat decoding', () => {
  it('reads the eight positions of an axis-encoded hat, clockwise from up', () => {
    const positions = [-1, -0.7142857, -0.4285714, -0.1428571, 0.1428571, 0.4285714, 0.7142857, 1];
    const masks = [
      HAT_BIT.up,
      HAT_BIT.up | HAT_BIT.right,
      HAT_BIT.right,
      HAT_BIT.right | HAT_BIT.down,
      HAT_BIT.down,
      HAT_BIT.down | HAT_BIT.left,
      HAT_BIT.left,
      HAT_BIT.left | HAT_BIT.up,
    ];
    positions.forEach((v, i) => {
      expect(decodeHatAxis(v), String(v)).toBe(masks[i]);
    });
  });

  it('reads centred as 1.29 (Chrome) or 3.29 (Firefox) and rejects values a hat never makes', () => {
    expect(decodeHatAxis(HAT_CENTRED)).toBe(0);
    expect(decodeHatAxis(3.2857142857142856)).toBe(0);
    expect(decodeHatAxis(0)).toBeUndefined();
    expect(decodeHatAxis(0.5)).toBeUndefined();
    expect(decodeHatAxis(-1.5)).toBeUndefined();
    expect(decodeHatAxis(Number.NaN)).toBeUndefined();
  });

  it('builds a hat from four buttons, diagonals included', () => {
    expect(decodeHatButtons(false, false, false, false)).toBe(0);
    expect(decodeHatButtons(true, false, false, false)).toBe(HAT_BIT.up);
    expect(decodeHatButtons(true, true, false, false)).toBe(HAT_BIT.up | HAT_BIT.right);
    expect(decodeHatButtons(false, false, true, true)).toBe(HAT_BIT.down | HAT_BIT.left);
  });
});

describe('GamepadHub', () => {
  it('reads axes and analogue button values from a standard pad into its slot', () => {
    const pads = new FakePads();
    pads.set(0, fakePad({ axes: { 2: 0.5, 3: -0.25 }, buttons: { 7: 0.75, 8: 1 }, timestamp: 42 }));
    const hub = hubOf(pads);
    hub.poll();
    const s = hub.slots[0];
    expect(s?.present).toBe(true);
    expect(s?.device).toBe('pad');
    expect(s?.axes[PAD_AXIS.RIGHT_X]).toBe(0.5);
    expect(s?.axes[PAD_AXIS.RIGHT_Y]).toBe(-0.25);
    expect(s?.buttons[PAD_BUTTON.RIGHT_TRIGGER]).toBe(0.75);
    expect(s?.buttons[PAD_BUTTON.BACK]).toBe(1);
    expect(s?.timestamp).toBe(42);
    expect(hub.count).toBe(1);
  });

  it('is empty with no pad, a null slot or a disconnected one', () => {
    const pads = new FakePads();
    const hub = hubOf(pads);
    hub.poll();
    expect(hub.count).toBe(0);
    pads.set(1, null);
    hub.poll();
    pads.set(0, fakePad({ connected: false }));
    hub.poll();
    expect(hub.count).toBe(0);
    expect(hub.revision).toBe(0);
  });

  it('survives getGamepads throwing', () => {
    const hub = new GamepadHub(() => {
      throw new Error('not allowed');
    });
    expect(() => {
      hub.poll();
    }).not.toThrow();
    expect(hub.count).toBe(0);
  });

  it('names devices by class: pad, joy:vendor:product, joy', () => {
    const pads = new FakePads();
    pads.set(0, fakePad());
    pads.set(1, fakeStick());
    pads.set(2, fakeStick({ id: 'Mystery Stick' }));
    const hub = hubOf(pads);
    hub.poll();
    expect(hub.deviceNames()).toEqual(['pad', 'joy:044f:b10a', 'joy']);
  });

  it('counts every kind of change in `revision`: a device arriving, leaving, or being swapped', () => {
    const pads = new FakePads();
    const hub = hubOf(pads);
    hub.poll();
    const r0 = hub.revision;
    pads.set(0, fakePad());
    hub.poll();
    const r1 = hub.revision;
    expect(r1).toBeGreaterThan(r0);
    hub.poll(); // nothing changed
    expect(hub.revision).toBe(r1);
    pads.set(0, fakePad({ axes: { 0: 0.9 } })); // same device, new reading
    hub.poll();
    expect(hub.revision).toBe(r1);
    pads.set(0, fakeStick({ index: 0 })); // another device in the same slot
    hub.poll();
    const r2 = hub.revision;
    expect(r2).toBeGreaterThan(r1);
    pads.set(0, null);
    hub.poll();
    expect(hub.revision).toBeGreaterThan(r2);
    expect(hub.slots[0]?.present).toBe(false);
    expect(hub.slots[0]?.axes[0]).toBe(0);
  });

  it("finds the slot a binding's device covers, the lowest first", () => {
    const pads = new FakePads();
    pads.set(0, fakePad());
    pads.set(1, fakeStick());
    pads.set(2, fakeStick({ id: '046d-c215-Logitech Extreme 3D Pro', index: 2 }));
    const hub = hubOf(pads);
    hub.poll();
    expect(hub.find('pad')).toBe(0);
    expect(hub.find('joy')).toBe(1);
    expect(hub.find('joy:046d:c215')).toBe(2);
    expect(hub.find('joy:0000:0000')).toBe(-1);
  });

  it('decodes the d-pad of a standard pad into hat 0', () => {
    const pads = new FakePads();
    pads.set(0, fakePad({ buttons: { 12: 1, 15: 1 } }));
    const hub = hubOf(pads);
    hub.poll();
    expect(hub.slots[0]?.hatCount).toBe(1);
    expect(hub.slots[0]?.hats[0]).toBe(HAT_BIT.up | HAT_BIT.right);
    pads.set(0, fakePad());
    hub.poll();
    expect(hub.slots[0]?.hats[0]).toBe(0);
  });

  it('decodes an axis-encoded hat on a raw joystick (axes[9]) into hat 0', () => {
    const pads = new FakePads();
    pads.set(0, fakeStick());
    const hub = hubOf(pads);
    hub.poll();
    expect(hub.slots[0]?.hatAxis).toBe(9);
    expect(hub.slots[0]?.hats[0]).toBe(0);
    pads.set(0, fakeStick({ axes: { 9: -0.4285714 } }));
    hub.poll();
    expect(hub.slots[0]?.hats[0]).toBe(HAT_BIT.right);
    pads.set(0, fakeStick({ axes: { 9: 0.7142857 } }));
    hub.poll();
    expect(hub.slots[0]?.hats[0]).toBe(HAT_BIT.left);
  });

  it('finds no hat on a raw joystick whose axis 9 is not one', () => {
    const pads = new FakePads();
    pads.set(0, fakeStick({ axes: { 9: 0.05 } }));
    const hub = hubOf(pads);
    hub.poll();
    expect(hub.slots[0]?.hatAxis).toBe(-1);
    expect(hub.slots[0]?.hatCount).toBe(0);
    pads.set(0, fakeStick({ axisCount: 4 }));
    hub.poll();
    expect(hub.slots[0]?.hatCount).toBe(0);
  });

  it('turns non-finite readings into zero and treats a pressed button with value 0 as 1', () => {
    const pads = new FakePads();
    const pad = fakePad({ axes: { 0: Number.NaN, 1: Number.POSITIVE_INFINITY } });
    (pad.buttons as { pressed: boolean; value: number }[])[0] = { pressed: true, value: 0 };
    pads.set(0, pad);
    const hub = hubOf(pads);
    hub.poll();
    expect([hub.slots[0]?.axes[0], hub.slots[0]?.axes[1]]).toEqual([0, 0]);
    expect(hub.slots[0]?.buttons[0]).toBe(1);
  });

  it('keeps to its slot sizes however big the device', () => {
    const pads = new FakePads();
    pads.set(0, fakeStick({ axisCount: 100 }));
    for (let i = 1; i < 12; i++) pads.set(i, fakePad({ index: i }));
    const hub = hubOf(pads);
    hub.poll();
    expect(hub.slots[0]?.axisCount).toBe(MAX_AXES);
    expect(hub.slots.length).toBe(MAX_PADS);
    expect(hub.count).toBe(MAX_PADS);
  });

  it('reuses its slots: no new arrays between polls', () => {
    const pads = new FakePads();
    pads.set(0, fakePad());
    const hub = hubOf(pads);
    hub.poll();
    const axes = hub.slots[0]?.axes;
    const buttons = hub.slots[0]?.buttons;
    hub.poll();
    expect(hub.slots[0]?.axes).toBe(axes);
    expect(hub.slots[0]?.buttons).toBe(buttons);
  });

  it('knows the sample ids', () => {
    expect(PAD_ID).toContain('Vendor: 045e');
    expect(STICK_ID).toContain('Vendor: 044f');
  });
});
