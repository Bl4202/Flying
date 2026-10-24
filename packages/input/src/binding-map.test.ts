import { describe, expect, it } from 'vitest';
import { ACTIONS, actionIndex } from './actions.ts';
import { type BindingMap, compileBindings, type Devices } from './binding-map.ts';
import { GamepadHub } from './devices/gamepad.ts';
import { KeyboardState } from './keyboard.ts';
import { builtinProfile, calibrationKey, type Binding, type ControlsProfile } from './profile.ts';
import { FakeMouse, FakePads, fakePad, fakeStick } from './test-support.ts';

function profileOf(bindings: Binding[], extra: Partial<ControlsProfile> = {}): ControlsProfile {
  return { ...builtinProfile('default-km'), bindings, ...extra };
}

interface Rig {
  map: BindingMap;
  dev: Devices;
  keyboard: KeyboardState;
  mouse: FakeMouse;
  pads: FakePads;
  hub: GamepadHub;
}

function rig(bindings: Binding[], extra: Partial<ControlsProfile> = {}): Rig {
  const keyboard = new KeyboardState();
  const mouse = new FakeMouse();
  const pads = new FakePads();
  const hub = new GamepadHub(pads.get);
  const map = compileBindings(profileOf(bindings, extra));
  return { map, dev: { keyboard, mouse, pads: hub }, keyboard, mouse, pads, hub };
}

/** Polls the hub and resolves the map, as the pump does when the revision moves. */
function connect(r: Rig): void {
  r.hub.poll();
  r.map.resolve(r.hub);
}

describe('compileBindings: layout', () => {
  it('groups the bindings by action in the profile order, with the right counts', () => {
    const km = builtinProfile('default-km');
    const map = compileBindings(km);
    expect(map.count).toBe(km.bindings.length);
    let total = 0;
    for (const a of ACTIONS) {
      const idx = actionIndex(a.id);
      const expected = km.bindings.filter((b) => b.action === a.id).length;
      expect(map.last(idx) - map.first(idx), a.id).toBe(expected);
      total += expected;
    }
    expect(total).toBe(km.bindings.length);
    // Contiguous: each action starts where the one before it ended.
    for (let a = 1; a < ACTIONS.length; a++) expect(map.first(a)).toBe(map.last(a - 1));
  });

  it('collects the key codes the page must claim', () => {
    const map = compileBindings(builtinProfile('default-km'));
    for (const code of ['KeyW', 'KeyS', 'ShiftLeft', 'ControlRight', 'PageUp', 'F8', 'Escape']) {
      expect(map.keyCodes.has(code), code).toBe(true);
    }
    expect(map.keyCodes.has('KeyX')).toBe(false);
    // Mouse and pad bindings are not keys.
    const pad = compileBindings(
      profileOf([
        { action: 'flight.gear', source: { kind: 'pad-button', device: 'pad', index: 3 } },
      ]),
    );
    expect(pad.keyCodes.size).toBe(0);
  });

  it('compiles an empty profile', () => {
    const map = compileBindings(profileOf([]));
    expect(map.count).toBe(0);
    expect(map.first(0)).toBe(map.last(0));
  });
});

describe('BindingMap.value: keyboard and mouse', () => {
  const pitchW: Binding = {
    action: 'flight.pitch',
    source: { kind: 'key', code: 'KeyW' },
    direction: -1,
  };
  const a = actionIndex('flight.pitch');

  it('reads a key as 1 while down', () => {
    const r = rig([pitchW]);
    expect(r.map.value(r.map.first(a), r.dev)).toBe(0);
    r.keyboard.keyDown('KeyW', false);
    expect(r.map.value(r.map.first(a), r.dev)).toBe(1);
    expect(r.map.direction(r.map.first(a))).toBe(-1);
    expect(r.map.isDigital(r.map.first(a))).toBe(true);
    expect(r.map.active(r.map.first(a), r.dev)).toBe(true);
  });

  it('reads mouse buttons and counts wheel notches', () => {
    const r = rig([
      { action: 'flight.gear', source: { kind: 'mouse-button', button: 2 } },
      { action: 'flight.throttle_up', source: { kind: 'wheel', dir: 'up' } },
      { action: 'flight.throttle_down', source: { kind: 'wheel', dir: 'down' } },
    ]);
    const gear = r.map.first(actionIndex('flight.gear'));
    const up = r.map.first(actionIndex('flight.throttle_up'));
    const down = r.map.first(actionIndex('flight.throttle_down'));
    expect(r.map.value(gear, r.dev)).toBe(0);
    r.mouse.down.add(2);
    expect(r.map.value(gear, r.dev)).toBe(1);
    r.mouse.notch('up', 3);
    r.mouse.notch('down', 1);
    r.mouse.consume();
    expect(r.map.value(up, r.dev)).toBe(3);
    expect(r.map.value(down, r.dev)).toBe(1);
    expect(r.map.isWheel(up)).toBe(true);
    expect(r.map.isWheel(gear)).toBe(false);
    expect(r.map.active(up, r.dev)).toBe(true);
  });

  it('reads 0 without a mouse', () => {
    const r = rig([{ action: 'flight.gear', source: { kind: 'mouse-button', button: 0 } }]);
    const dev: Devices = { ...r.dev, mouse: undefined };
    expect(r.map.value(r.map.first(actionIndex('flight.gear')), dev)).toBe(0);
  });
});

describe('BindingMap.value: pads', () => {
  const pitch = actionIndex('flight.pitch');

  it('reads nothing from a device that is not there, and says so', () => {
    const r = rig([
      { action: 'flight.pitch', source: { kind: 'pad-axis', device: 'pad', index: 3 } },
    ]);
    connect(r);
    const i = r.map.first(pitch);
    expect(r.map.present(i)).toBe(false);
    expect(r.map.value(i, r.dev)).toBe(0);
  });

  it('reads axes and buttons once the device is resolved', () => {
    const r = rig([
      { action: 'flight.pitch', source: { kind: 'pad-axis', device: 'pad', index: 3 } },
      { action: 'flight.gear', source: { kind: 'pad-button', device: 'pad', index: 3 } },
    ]);
    r.pads.set(0, fakePad({ axes: { 3: -0.5 }, buttons: { 3: 1 } }));
    connect(r);
    const axis = r.map.first(pitch);
    const button = r.map.first(actionIndex('flight.gear'));
    expect(r.map.present(axis)).toBe(true);
    expect(r.map.value(axis, r.dev)).toBe(-0.5);
    expect(r.map.value(button, r.dev)).toBe(1);
    expect(r.map.isDigital(axis)).toBe(false);
    // A move shows without resolving again.
    r.pads.set(0, fakePad({ axes: { 3: 0.75 } }));
    r.hub.poll();
    expect(r.map.value(axis, r.dev)).toBe(0.75);
    expect(r.map.value(button, r.dev)).toBe(0);
  });

  it("takes one half of an axis as a 0..1 value, and inverts one binding's axis", () => {
    const r = rig([
      {
        action: 'flight.throttle_up',
        source: { kind: 'pad-axis', device: 'pad', index: 1, half: 'negative' },
      },
      {
        action: 'flight.throttle_down',
        source: { kind: 'pad-axis', device: 'pad', index: 1, half: 'positive' },
      },
      {
        action: 'flight.roll',
        source: { kind: 'pad-axis', device: 'pad', index: 0 },
        invert: true,
      },
    ]);
    r.pads.set(0, fakePad({ axes: { 0: 0.5, 1: -0.8 } }));
    connect(r);
    const up = r.map.first(actionIndex('flight.throttle_up'));
    const down = r.map.first(actionIndex('flight.throttle_down'));
    const roll = r.map.first(actionIndex('flight.roll'));
    expect(r.map.value(up, r.dev)).toBeCloseTo(0.8, 12);
    expect(r.map.value(down, r.dev)).toBe(0);
    expect(r.map.value(roll, r.dev)).toBe(-0.5);
  });

  it('maps a whole axis onto 0..1 for a lever, and a half or a button as read', () => {
    const r = rig([
      {
        action: 'flight.throttle',
        source: { kind: 'pad-axis', device: 'joy', index: 2 },
        invert: true,
      },
      { action: 'flight.brake_left', source: { kind: 'pad-button', device: 'pad', index: 6 } },
      {
        action: 'flight.brake_right',
        source: { kind: 'pad-axis', device: 'pad', index: 1, half: 'positive' },
      },
    ]);
    r.pads.set(0, fakePad({ axes: { 1: 0.4 }, buttons: { 6: 0.3 } }));
    r.pads.set(1, fakeStick({ axes: { 2: -1 } })); // lever pushed forward
    connect(r);
    const lever = r.map.first(actionIndex('flight.throttle'));
    expect(r.map.unipolar(lever, r.dev)).toBe(1);
    r.pads.set(1, fakeStick({ axes: { 2: 1 } }));
    r.hub.poll();
    expect(r.map.unipolar(lever, r.dev)).toBe(0);
    r.pads.set(1, fakeStick({ axes: { 2: 0 } }));
    r.hub.poll();
    expect(r.map.unipolar(lever, r.dev)).toBe(0.5);
    expect(r.map.unipolar(r.map.first(actionIndex('flight.brake_left')), r.dev)).toBeCloseTo(
      0.3,
      12,
    );
    expect(r.map.unipolar(r.map.first(actionIndex('flight.brake_right')), r.dev)).toBeCloseTo(
      0.4,
      12,
    );
  });

  it("calibrates an axis, preferring the connected device's own calibration", () => {
    const cal = {
      [calibrationKey('joy:044f:b10a', 2)]: { min: -0.5, centre: 0, max: 0.5 },
      [calibrationKey('joy', 2)]: { min: -1, centre: 0, max: 1 },
    };
    const r = rig(
      [{ action: 'flight.roll', source: { kind: 'pad-axis', device: 'joy', index: 2 } }],
      {
        calibration: cal,
      },
    );
    r.pads.set(0, fakeStick({ axes: { 2: 0.25 } }));
    connect(r);
    // The stick's own entry maps +-0.5 onto +-1.
    expect(r.map.value(r.map.first(actionIndex('flight.roll')), r.dev)).toBeCloseTo(0.5, 12);
    // A different raw stick falls back to the entry under the binding's own name.
    const other = rig(
      [{ action: 'flight.roll', source: { kind: 'pad-axis', device: 'joy', index: 2 } }],
      {
        calibration: { [calibrationKey('joy', 2)]: { min: -1, centre: 0, max: 0.5 } },
      },
    );
    other.pads.set(0, fakeStick({ id: 'Mystery Stick', axes: { 2: 0.25 } }));
    connect(other);
    expect(other.map.value(other.map.first(actionIndex('flight.roll')), other.dev)).toBeCloseTo(
      0.5,
      12,
    );
  });

  it('reads hat directions, and a diagonal as both', () => {
    const r = rig([
      {
        action: 'flight.trim_nose_down',
        source: { kind: 'hat', device: 'pad', hat: 0, dir: 'up' },
      },
      { action: 'flight.flaps_down', source: { kind: 'hat', device: 'pad', hat: 0, dir: 'right' } },
      { action: 'flight.flaps_up', source: { kind: 'hat', device: 'pad', hat: 0, dir: 'left' } },
    ]);
    r.pads.set(0, fakePad({ buttons: { 12: 1, 15: 1 } }));
    connect(r);
    const read = (id: Parameters<typeof actionIndex>[0]) =>
      r.map.value(r.map.first(actionIndex(id)), r.dev);
    expect(read('flight.trim_nose_down')).toBe(1);
    expect(read('flight.flaps_down')).toBe(1);
    expect(read('flight.flaps_up')).toBe(0);
  });

  it('follows a device that comes back in another slot', () => {
    const r = rig([
      { action: 'flight.pitch', source: { kind: 'pad-axis', device: 'pad', index: 3 } },
    ]);
    r.pads.set(0, fakePad({ axes: { 3: 0.5 } }));
    connect(r);
    const i = r.map.first(actionIndex('flight.pitch'));
    expect(r.map.value(i, r.dev)).toBe(0.5);
    r.pads.set(0, null);
    r.hub.poll();
    r.map.resolve(r.hub);
    expect(r.map.present(i)).toBe(false);
    expect(r.map.value(i, r.dev)).toBe(0);
    r.pads.set(2, fakePad({ index: 2, axes: { 3: -0.5 } }));
    connect(r);
    expect(r.map.present(i)).toBe(true);
    expect(r.map.value(i, r.dev)).toBe(-0.5);
  });

  it('uses the threshold of the binding to decide a press', () => {
    const r = rig([
      {
        action: 'flight.gear',
        source: { kind: 'pad-button', device: 'pad', index: 7 },
        threshold: 0.2,
      },
      { action: 'flight.recover', source: { kind: 'pad-button', device: 'pad', index: 7 } },
    ]);
    r.pads.set(0, fakePad({ buttons: { 7: 0.3 } }));
    connect(r);
    expect(r.map.active(r.map.first(actionIndex('flight.gear')), r.dev)).toBe(true);
    expect(r.map.active(r.map.first(actionIndex('flight.recover')), r.dev)).toBe(false);
    expect(r.map.threshold(r.map.first(actionIndex('flight.gear')))).toBe(0.2);
  });
});
