import { describe, expect, it } from 'vitest';
import { memoryKv } from '@flying/platform-web';
import {
  bindingsOf,
  builtinProfile,
  GamepadHub,
  SETTLE_MS,
  type ControlsProfile,
} from '@flying/input';
import { ControlsStore } from './controls-store.ts';
import { deviceLabel, EditorModel, sourceLabel } from './editor-model.ts';
import { FakePads, fakePad, fakeStick } from './test-pads.ts';

const flush = (): Promise<void> => new Promise((resolve) => setTimeout(resolve, 0));

async function setup(initial = 'default-km') {
  const kv = memoryKv();
  const store = new ControlsStore(kv);
  await store.load();
  const pads = new FakePads();
  const hub = new GamepadHub(pads.get);
  const seen: ControlsProfile[] = [];
  let now = 0;
  const model = new EditorModel(
    {
      store,
      hub,
      onProfile: (p) => seen.push(p),
      nowMs: () => now,
    },
    initial,
  );
  return {
    kv,
    store,
    pads,
    hub,
    model,
    seen,
    advance: (ms: number) => {
      now += ms;
    },
    /** Lets capture settle with the pads at rest. */
    settle: () => {
      hub.poll();
      model.pollPads();
      now += SETTLE_MS + 1;
      hub.poll();
      model.pollPads();
    },
  };
}

const rowOf = (m: EditorModel, id: string, search = '') =>
  m
    .groups(search)
    .flatMap((g) => g.rows)
    .find((r) => r.id === id);

describe('labels', () => {
  it('names sources and devices for a chip', () => {
    expect(sourceLabel({ kind: 'key', code: 'KeyW' })).toBe('W');
    expect(sourceLabel({ kind: 'mouse-button', button: 1 })).toBe('Mouse 2');
    expect(sourceLabel({ kind: 'wheel', dir: 'up' })).toBe('Wheel up');
    expect(sourceLabel({ kind: 'pad-button', device: 'pad', index: 3 })).toBe('Button 3');
    expect(sourceLabel({ kind: 'pad-axis', device: 'pad', index: 1, half: 'negative' })).toBe(
      'Axis 1−',
    );
    expect(sourceLabel({ kind: 'pad-axis', device: 'pad', index: 1, half: 'positive' })).toBe(
      'Axis 1+',
    );
    expect(sourceLabel({ kind: 'pad-axis', device: 'pad', index: 1 })).toBe('Axis 1');
    expect(sourceLabel({ kind: 'hat', device: 'pad', hat: 0, dir: 'left' })).toBe('Hat 0 left');
    expect(deviceLabel('pad')).toBe('Pad');
    expect(deviceLabel('joy')).toBe('Joystick');
    expect(deviceLabel('joy:044f:b10a')).toBe('Joystick 044f:b10a');
  });
});

describe('EditorModel: what is listed', () => {
  it('lists every action once, in groups, with the chips of the profile', async () => {
    const { model } = await setup();
    const groups = model.groups();
    expect(groups.map((g) => g.group)).toEqual([
      'axes',
      'engine',
      'systems',
      'brakes',
      'view',
      'system',
    ]);
    const ids = groups.flatMap((g) => g.rows.map((r) => r.id));
    expect(new Set(ids).size).toBe(ids.length);
    expect(ids).toHaveLength(24);
    const pitch = rowOf(model, 'flight.pitch');
    expect(pitch?.bipolar).toBe(true);
    expect(pitch?.shaped).toBe(true);
    expect(pitch?.chips.map((c) => `${c.label}${c.direction === -1 ? '-' : '+'}`)).toEqual([
      'W-',
      'S+',
    ]);
    expect(pitch?.label).toBe('Pitch (− forward, + back)');
    const gear = rowOf(model, 'flight.gear');
    expect(gear?.bipolar).toBe(false);
    expect(gear?.shaped).toBe(false);
    expect(gear?.chips.map((c) => c.label)).toEqual(['L']);
    expect(gear?.chips[0]?.direction).toBeUndefined();
  });

  it('filters by what the person types: label, id or a binding', async () => {
    const { model } = await setup();
    const ids = (search: string) => model.groups(search).flatMap((g) => g.rows.map((r) => r.id));
    expect(ids('landing gear')).toEqual(['flight.gear']);
    expect(ids('GEAR')).toEqual(['flight.gear']);
    expect(ids('flight.afterburner')).toEqual(['flight.afterburner']);
    expect(ids('flaps')).toEqual(['flight.flaps_down', 'flight.flaps_up', 'flight.flaps_cycle']);
    expect(ids('f8')).toEqual(['flight.assist_cycle']);
    expect(ids('flaps cycle')).toEqual(['flight.flaps_cycle']);
    expect(ids('zzz nothing')).toEqual([]);
    expect(model.groups('zzz nothing')).toEqual([]);
  });

  it('marks a chip that shares its control, and counts them', async () => {
    const { model } = await setup();
    expect(model.conflictCount()).toBe(0);
    model.beginCapture('flight.recover');
    model.feedKey('KeyL'); // the gear key
    const gear = rowOf(model, 'flight.gear');
    const recover = rowOf(model, 'flight.recover');
    expect(gear?.chips[0]?.conflict).toBe(true);
    expect(recover?.chips.at(-1)?.conflict).toBe(true);
    expect(model.conflictCount()).toBe(2);
  });

  it('marks keys the browser keeps with Ctrl held', async () => {
    const { model } = await setup();
    // W, with Ctrl bound for throttle down; F8 and Escape are nobody's shortcut.
    expect(rowOf(model, 'flight.pitch')?.chips[0]?.reserved).toBe(true);
    expect(rowOf(model, 'flight.assist_cycle')?.chips[0]?.reserved).toBe(false);
    expect(rowOf(model, 'system.pause')?.chips[0]?.reserved).toBe(false);
  });

  it('says how many bindings an action can still take', async () => {
    const { model } = await setup();
    expect(rowOf(model, 'flight.throttle_up')?.canAdd).toBe(false); // already four
    expect(rowOf(model, 'flight.gear')?.canAdd).toBe(true);
  });
});

describe('EditorModel: capture', () => {
  it('adds the key pressed, as a decrease or an increase on a bipolar axis', async () => {
    const { model } = await setup();
    expect(model.beginCapture('flight.yaw', -1)).toBe(true);
    expect(model.capturing).toEqual({ action: 'flight.yaw', direction: -1 });
    expect(model.feedKey('KeyJ')).toEqual({
      state: 'added',
      binding: { action: 'flight.yaw', source: { kind: 'key', code: 'KeyJ' }, direction: -1 },
    });
    expect(model.capturing).toBeUndefined();
    model.beginCapture('flight.yaw'); // default: increase
    model.feedKey('KeyK');
    expect(rowOf(model, 'flight.yaw')?.chips.map((c) => [c.label, c.direction])).toEqual([
      ['Q', -1],
      ['E', 1],
      ['J', -1],
      ['K', 1],
    ]);
  });

  it('gives no sign to an action that is not a bipolar axis', async () => {
    const { model } = await setup();
    model.beginCapture('flight.gear');
    const r = model.feedKey('KeyG');
    expect(r).toMatchObject({ state: 'added', binding: { source: { kind: 'key', code: 'KeyG' } } });
    expect(r.state === 'added' && 'direction' in r.binding).toBe(false);
  });

  it('refuses to start when the action is full, and Esc cancels', async () => {
    const { model } = await setup();
    expect(model.beginCapture('flight.throttle_up')).toBe(false);
    expect(model.capturing).toBeUndefined();
    model.beginCapture('flight.gear');
    expect(model.feedKey('Escape')).toEqual({ state: 'cancelled' });
    expect(model.capturing).toBeUndefined();
    expect(rowOf(model, 'flight.gear')?.chips).toHaveLength(1);
  });

  it('can be cancelled from outside', async () => {
    const { model } = await setup();
    model.beginCapture('flight.gear');
    model.cancelCapture();
    expect(model.capturing).toBeUndefined();
    expect(model.feedKey('KeyG')).toEqual({ state: 'waiting' });
  });

  it('takes the mouse and the wheel', async () => {
    const { model } = await setup();
    model.beginCapture('flight.gear');
    expect(model.feedMouse(2)).toMatchObject({
      binding: { source: { kind: 'mouse-button', button: 2 } },
    });
    model.beginCapture('flight.recover');
    expect(model.feedWheel('down')).toMatchObject({
      binding: { source: { kind: 'wheel', dir: 'down' } },
    });
  });

  it('takes a pad axis whole for an axis action and as a half for a button', async () => {
    const t = await setup();
    t.pads.set(0, fakePad());
    t.model.beginCapture('flight.roll', 1);
    t.settle();
    t.pads.set(0, fakePad({ axes: { 0: 0.9 } }));
    t.hub.poll();
    const roll = t.model.pollPads();
    expect(roll).toMatchObject({
      state: 'added',
      binding: { action: 'flight.roll', source: { kind: 'pad-axis', device: 'pad', index: 0 } },
    });
    expect(roll.state === 'added' && 'direction' in roll.binding).toBe(false); // an axis has its own sign

    t.pads.set(0, fakePad());
    t.model.beginCapture('flight.gear');
    t.settle();
    t.pads.set(0, fakePad({ axes: { 1: -0.9 } }));
    t.hub.poll();
    expect(t.model.pollPads()).toMatchObject({
      binding: { source: { kind: 'pad-axis', device: 'pad', index: 1, half: 'negative' } },
    });
  });

  it('takes a pad button for a bipolar axis with the sign asked for', async () => {
    const t = await setup();
    t.pads.set(0, fakePad());
    t.model.beginCapture('flight.yaw', -1);
    t.settle();
    t.pads.set(0, fakePad({ buttons: { 6: 1 } }));
    t.hub.poll();
    expect(t.model.pollPads()).toMatchObject({
      binding: { source: { kind: 'pad-button', index: 6 }, direction: -1 },
    });
  });

  it('refuses a key for the throttle lever, which needs an axis', async () => {
    const { model } = await setup();
    model.beginCapture('flight.throttle');
    expect(model.feedKey('KeyT')).toEqual({ state: 'rejected', reason: 'throttle-needs-axis' });
    expect(rowOf(model, 'flight.throttle')?.chips).toHaveLength(0);
    expect(model.capturing).toBeUndefined();
  });
});

describe('EditorModel: changes', () => {
  it('copies a built-in profile on the first change, and keeps the original', async () => {
    const t = await setup();
    const km = builtinProfile('default-km');
    t.model.beginCapture('flight.gear');
    t.model.feedKey('KeyG');
    await flush();
    expect(t.model.isBuiltin).toBe(false);
    expect(t.model.profile.id).toBe('keyboard-and-mouse-custom');
    expect(t.model.profile.name).toBe('Keyboard and mouse (custom)');
    expect(t.store.activeId).toBe('keyboard-and-mouse-custom');
    expect(bindingsOf(t.model.profile, 'flight.gear')).toHaveLength(2);
    expect(t.store.find('default-km')).toEqual(km);
    expect(t.seen.at(-1)?.id).toBe('keyboard-and-mouse-custom');
    // The next change goes to the copy, not to a second copy.
    t.model.remove(bindingsOf(t.model.profile, 'flight.gear')[0]?.index ?? 0);
    await flush();
    expect(t.store.user()).toHaveLength(1);
  });

  it('stores what it changes, so a reload finds it', async () => {
    const t = await setup();
    t.model.setShape('flight.pitch', { gain: 1.5, invert: true }); // makes the copy
    await flush();
    const again = new ControlsStore(t.kv);
    await again.load();
    expect(again.user()).toHaveLength(1);
    expect(again.user()[0]?.axes['flight.pitch']).toMatchObject({ gain: 1.5, invert: true });
    expect(again.activeId).toBe('keyboard-and-mouse-custom');
  });

  it('makes no copy for a change that changes nothing', async () => {
    const t = await setup();
    t.model.restoreAction('flight.gear');
    t.model.restoreGroup('systems');
    t.model.restoreAll();
    t.model.setShape('flight.pitch', {});
    await flush();
    expect(t.model.isBuiltin).toBe(true);
    expect(t.store.user()).toHaveLength(0);
    expect(t.seen).toEqual([]);
  });

  it('removes a binding, restores an action, a group and everything', async () => {
    const t = await setup();
    const gearAt = bindingsOf(t.model.profile, 'flight.gear')[0]?.index ?? 0;
    t.model.remove(gearAt);
    expect(rowOf(t.model, 'flight.gear')?.chips).toHaveLength(0);
    t.model.restoreAction('flight.gear');
    expect(rowOf(t.model, 'flight.gear')?.chips.map((c) => c.label)).toEqual(['L']);
    t.model.remove(bindingsOf(t.model.profile, 'flight.brakes')[0]?.index ?? 0);
    t.model.remove(bindingsOf(t.model.profile, 'flight.parking_brake')[0]?.index ?? 0);
    t.model.restoreGroup('brakes');
    expect(rowOf(t.model, 'flight.brakes')?.chips).toHaveLength(1);
    expect(rowOf(t.model, 'flight.parking_brake')?.chips).toHaveLength(1);
    t.model.beginCapture('flight.gear');
    t.model.feedKey('KeyG');
    t.model.restoreAll();
    expect(t.model.profile.bindings).toEqual(builtinProfile('default-km').bindings);
    await flush();
  });

  it("renames a profile of the player's own, not a built-in", async () => {
    const t = await setup();
    t.model.rename('Nope');
    expect(t.store.user()).toHaveLength(0);
    t.model.setShape('flight.roll', { gain: 1.2 }); // makes the copy
    t.model.rename('  My keys ');
    await flush();
    expect(t.model.profile.name).toBe('My keys');
    t.model.rename('   ');
    expect(t.model.profile.name).toBe('My keys');
  });

  it('duplicates, creates and deletes profiles, and says so to the app', async () => {
    const t = await setup('gamepad-standard');
    t.model.duplicate('Pad two');
    await flush();
    expect(t.model.profile.id).toBe('pad-two');
    expect(t.model.profile.deviceClass).toBe('gamepad');
    expect(t.store.activeId).toBe('pad-two');
    t.model.create('joystick', 'Hotas');
    await flush();
    expect(t.model.profile.id).toBe('hotas');
    expect(t.model.profile.deviceClass).toBe('joystick');
    expect(t.model.profile.bindings).toEqual(builtinProfile('generic-joystick').bindings);
    t.model.delete();
    await flush();
    expect(t.model.profile.id).toBe('generic-joystick');
    expect(t.store.find('hotas')).toBeUndefined();
    expect(t.seen.at(-1)?.id).toBe('generic-joystick');
    t.model.delete(); // a built-in: nothing
    expect(t.store.find('generic-joystick')).toBeDefined();
  });

  it('selects another profile and makes it the one in use', async () => {
    const t = await setup();
    t.model.select('gamepad-standard');
    await flush();
    expect(t.model.profile.id).toBe('gamepad-standard');
    expect(t.store.activeId).toBe('gamepad-standard');
    expect(t.seen.at(-1)?.id).toBe('gamepad-standard');
    t.model.select('no-such-profile');
    expect(t.model.profile.id).toBe('gamepad-standard');
  });

  it('starts on the profile it is told, or the first', async () => {
    expect((await setup('generic-joystick')).model.profile.id).toBe('generic-joystick');
    expect((await setup('missing')).model.profile.id).toBe('default-km');
  });
});

describe('EditorModel: files', () => {
  it('exports the profile as a file named after it, and imports it back', async () => {
    const t = await setup();
    t.model.setShape('flight.roll', { gain: 1.2 });
    t.model.rename('Mine');
    await flush();
    const file = t.model.exportFile();
    expect(file.name).toBe('keyboard-and-mouse-custom.controls.json');
    expect(JSON.parse(file.text)).toMatchObject({ schema: 'controls/1', name: 'Mine' });
    const r = await t.model.importText(file.text);
    expect(r).toEqual({ ok: true });
    expect(t.model.profile.id).toBe('keyboard-and-mouse-custom-2');
    expect(t.model.profile.axes['flight.roll']?.gain).toBe(1.2);
    expect(t.model.importErrors).toEqual([]);
  });

  it('lists every problem of a bad file by pointer, and keeps the profile it had', async () => {
    const t = await setup();
    const r = await t.model.importText(
      JSON.stringify({
        schema: 'controls/1',
        id: 'x',
        name: 'x',
        deviceClass: 'foot',
        bindings: [{}],
      }),
    );
    expect(r.ok).toBe(false);
    expect(t.model.importErrors.map((e) => e.pointer)).toEqual([
      '/deviceClass',
      '/bindings/0/action',
      '/bindings/0/source',
    ]);
    expect(t.model.profile.id).toBe('default-km');
    t.model.clearImportErrors();
    expect(t.model.importErrors).toEqual([]);
  });
});

describe('EditorModel: calibration', () => {
  it('walks the wizard on a connected stick and stores the result under the device', async () => {
    const t = await setup('generic-joystick');
    expect(t.model.beginCalibration(0)).toBe(false); // nothing there yet
    t.pads.set(0, fakeStick({ axes: { 0: 0.05 } }));
    t.hub.poll();
    expect(t.model.beginCalibration(0)).toBe(true);
    expect(t.model.calibrating).toEqual({ device: 'joy:044f:b10a', step: 'centre' });
    t.model.advanceCalibration(); // too early: not enough readings
    expect(t.model.calibrating?.step).toBe('centre');
    for (let i = 0; i < 12; i++) t.model.sampleCalibration();
    t.model.advanceCalibration();
    expect(t.model.calibrating?.step).toBe('range');
    t.pads.set(0, fakeStick({ axes: { 0: -0.9 } }));
    t.hub.poll();
    t.model.sampleCalibration();
    t.pads.set(0, fakeStick({ axes: { 0: 0.95 } }));
    t.hub.poll();
    t.model.sampleCalibration();
    t.model.advanceCalibration();
    expect(t.model.calibrating).toBeUndefined();
    await flush();
    const cal = t.model.profile.calibration['joy:044f:b10a:a:0'];
    expect(cal?.centre).toBeCloseTo(0.05, 9);
    expect(cal?.min).toBeCloseTo(-0.9, 9);
    expect(cal?.max).toBeCloseTo(0.95, 9);
    expect(t.model.isBuiltin).toBe(false); // the first change copied the built-in profile
  });

  it('can be cancelled', async () => {
    const t = await setup();
    t.pads.set(0, fakeStick());
    t.hub.poll();
    t.model.beginCalibration(0);
    t.model.cancelCalibration();
    expect(t.model.calibrating).toBeUndefined();
  });
});
