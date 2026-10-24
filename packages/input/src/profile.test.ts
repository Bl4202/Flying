import { describe, expect, it } from 'vitest';
import { ACTIONS, type ActionId } from './actions.ts';
import {
  BUILTIN_IDS,
  builtinProfile,
  builtinProfiles,
  calibrationKey,
  chooseProfile,
  type ControlsProfile,
  defaultShape,
  MAX_BINDINGS_PER_ACTION,
  parseControlsProfile,
  parseControlsProfileText,
  profileDevices,
  profileSatisfied,
  serializeProfile,
  shapeFor,
} from './profile.ts';
import { LINEAR_SHAPE, STICK_SHAPE } from './shaping.ts';
import { sourceKey } from './sources.ts';

const errorsOf = (json: unknown): [string, string][] => {
  const r = parseControlsProfile(json);
  expect(r.ok).toBe(false);
  return r.ok ? [] : r.errors.map((e) => [e.pointer, e.message]);
};

/** A minimal document that parses, to break one field at a time. */
const minimal = (): Record<string, unknown> => ({
  schema: 'controls/1',
  id: 'mine',
  name: 'Mine',
  deviceClass: 'keyboard-mouse',
  bindings: [],
});

function bindingsOf(p: ControlsProfile, action: ActionId) {
  return p.bindings.filter((b) => b.action === action);
}

describe('built-in profiles', () => {
  it('are the three the plan names, each valid', () => {
    expect(BUILTIN_IDS).toEqual(['default-km', 'gamepad-standard', 'generic-joystick']);
    for (const p of builtinProfiles()) {
      const r = parseControlsProfile(JSON.parse(serializeProfile(p)));
      expect(r.ok, p.id).toBe(true);
    }
  });

  it('give fresh copies, so editing one never changes the next', () => {
    const a = builtinProfile('default-km');
    a.bindings.length = 0;
    a.axes['flight.pitch'] = { ...STICK_SHAPE, gain: 2 };
    const b = builtinProfile('default-km');
    expect(b.bindings.length).toBeGreaterThan(10);
    expect(b.axes['flight.pitch']?.gain).toBe(1);
  });

  it('put pitch, roll and yaw on W/S, A/D and Q/E with the flight-stick signs', () => {
    const km = builtinProfile('default-km');
    const keys = (action: ActionId) =>
      bindingsOf(km, action).map((b) => [
        b.source.kind === 'key' ? b.source.code : '',
        b.direction,
      ]);
    expect(keys('flight.pitch')).toEqual([
      ['KeyW', -1],
      ['KeyS', 1],
    ]);
    expect(keys('flight.roll')).toEqual([
      ['KeyA', -1],
      ['KeyD', 1],
    ]);
    expect(keys('flight.yaw')).toEqual([
      ['KeyQ', -1],
      ['KeyE', 1],
    ]);
  });

  it("bind the plan's keyboard defaults", () => {
    const km = builtinProfile('default-km');
    const codeOf = (action: ActionId): string[] =>
      bindingsOf(km, action).flatMap((b) => (b.source.kind === 'key' ? [b.source.code] : []));
    expect(codeOf('flight.throttle_up')).toEqual(['ShiftLeft', 'ShiftRight', 'PageUp']);
    expect(codeOf('flight.throttle_down')).toEqual(['ControlLeft', 'ControlRight', 'PageDown']);
    expect(codeOf('flight.trim_nose_up')).toEqual(['BracketRight']);
    expect(codeOf('flight.trim_nose_down')).toEqual(['BracketLeft']);
    expect(codeOf('flight.trim_reset')).toEqual(['Backslash']);
    const single: [ActionId, string][] = [
      ['system.respawn', 'KeyR'],
      ['flight.afterburner', 'KeyZ'],
      ['flight.gear', 'KeyL'],
      ['flight.flaps_cycle', 'KeyV'],
      ['flight.brakes', 'KeyB'],
      ['flight.airbrake', 'KeyN'],
      ['flight.parking_brake', 'KeyP'],
      ['flight.recover', 'KeyU'],
      ['flight.mouse_mode', 'KeyO'],
      ['flight.assist_cycle', 'F8'],
      ['system.pause', 'Escape'],
    ];
    for (const [action, code] of single) expect(codeOf(action), action).toEqual([code]);
  });

  it("bind the plan's gamepad defaults", () => {
    const pad = builtinProfile('gamepad-standard');
    const src = (action: ActionId) => bindingsOf(pad, action).map((b) => sourceKey(b.source));
    expect(src('flight.roll')).toContain('pad:a:2');
    expect(src('flight.pitch')).toContain('pad:a:3');
    expect(src('flight.yaw')).toEqual(expect.arrayContaining(['pad:b:7', 'pad:b:6']));
    expect(src('flight.throttle_up')).toContain('pad:a:1-');
    expect(src('flight.throttle_down')).toContain('pad:a:1+');
    expect(src('flight.afterburner')).toContain('pad:b:10');
    expect(src('flight.trim_nose_up')).toContain('pad:h:0:down');
    expect(src('flight.trim_nose_down')).toContain('pad:h:0:up');
    expect(src('flight.flaps_down')).toContain('pad:h:0:right');
    expect(src('flight.flaps_up')).toContain('pad:h:0:left');
    expect(src('flight.gear')).toContain('pad:b:3');
    expect(src('flight.brakes')).toContain('pad:b:2');
    expect(src('flight.airbrake')).toContain('pad:b:1');
    expect(src('flight.parking_brake')).toContain('pad:b:0');
    expect(src('flight.recover')).toContain('pad:b:4');
    expect(src('system.respawn')).toContain('pad:b:8');
    expect(src('system.pause')).toContain('pad:b:9');
    expect(pad.toggles.afterburner).toBe('hold');
  });

  it('give each pad profile the keyboard bindings too', () => {
    for (const id of ['gamepad-standard', 'generic-joystick'] as const) {
      const keyed = builtinProfile(id).bindings.filter((b) => b.source.kind === 'key');
      expect(keyed.length, id).toBeGreaterThanOrEqual(20);
    }
  });

  it('read the joystick as roll and pitch on axes 0 and 1, an inverted lever on 2, yaw on 3 and 5', () => {
    const joy = builtinProfile('generic-joystick');
    const one = (action: ActionId) => bindingsOf(joy, action).map((b) => sourceKey(b.source));
    expect(one('flight.roll')).toContain('joy:a:0');
    expect(one('flight.pitch')).toContain('joy:a:1');
    expect(one('flight.throttle')).toEqual(['joy:a:2']);
    expect(bindingsOf(joy, 'flight.throttle')[0]?.invert).toBe(true);
    expect(one('flight.yaw')).toEqual(expect.arrayContaining(['joy:a:3', 'joy:a:5']));
    expect(joy.deviceClass).toBe('joystick');
  });

  it("never exceed the editor's binding count per action", () => {
    for (const p of builtinProfiles()) {
      for (const a of ACTIONS) {
        expect(bindingsOf(p, a.id).length, `${p.id} ${a.id}`).toBeLessThanOrEqual(
          MAX_BINDINGS_PER_ACTION,
        );
      }
    }
  });

  it('shape pitch and roll as sticks and everything else linearly', () => {
    const km = builtinProfile('default-km');
    expect(shapeFor(km, 'flight.pitch')).toEqual(STICK_SHAPE);
    expect(shapeFor(km, 'flight.roll')).toEqual(STICK_SHAPE);
    expect(shapeFor(km, 'flight.yaw')).toEqual(LINEAR_SHAPE);
    expect(shapeFor(km, 'flight.throttle_up')).toEqual(LINEAR_SHAPE);
    expect(defaultShape('flight.pitch')).toBe(STICK_SHAPE);
    // An action with no entry falls back to its default.
    const bare = { ...km, axes: {} };
    expect(shapeFor(bare, 'flight.roll')).toEqual(STICK_SHAPE);
  });
});

describe('controls/1 JSON', () => {
  it('writes a profile, reads it back, and writes the same bytes again (AC-58.5)', () => {
    for (const p of builtinProfiles()) {
      const text = serializeProfile(p);
      const back = parseControlsProfileText(text);
      expect(back.ok, p.id).toBe(true);
      if (back.ok) {
        expect(back.profile).toEqual(p);
        expect(serializeProfile(back.profile)).toBe(text);
      }
    }
  });

  it('round-trips an edited profile: odd shapes, calibration, rates, an unlimited rate limit', () => {
    const p = builtinProfile('generic-joystick');
    p.name = 'My stick';
    p.axes['flight.pitch'] = {
      deadCentre: 0.07,
      deadOuter: 0.02,
      curve: { kind: 'power', gamma: 1.5 },
      gain: 1.25,
      tauMs: 40,
      maxRatePerSecond: 6,
      invert: true,
      offset: 0.125,
    };
    p.calibration[calibrationKey('joy:046d:c215', 2)] = { min: -0.9, centre: 0.05, max: 0.95 };
    p.calibration[calibrationKey('pad', 0)] = { min: -1, centre: 0, max: 1 };
    p.keyboard = { rampUpPerSecond: 2.5, returnPerSecond: 6 };
    p.throttle = { keyRatePerSecond: 0.75, stickRatePerSecond: 1.5, wheelStep: 0.1 };
    p.toggles = { airbrake: 'hold', afterburner: 'hold' };
    p.bindings.push({
      action: 'flight.recover',
      source: { kind: 'pad-axis', device: 'joy:046d:c215', index: 4, half: 'positive' },
      threshold: 0.25,
      invert: true,
    });
    const text = serializeProfile(p);
    const back = parseControlsProfileText(text);
    expect(back.ok).toBe(true);
    if (!back.ok) return;
    expect(back.profile).toEqual(p);
    expect(serializeProfile(back.profile)).toBe(text);
    // The unlimited rate limit travels as null.
    expect(text).toContain('"maxRatePerSecond": null');
    expect(back.profile.axes['flight.roll']?.maxRatePerSecond).toBe(Infinity);
  });

  it('writes keys in a fixed order whatever order the profile was built in', () => {
    const p = builtinProfile('default-km');
    const scrambled: ControlsProfile = {
      toggles: p.toggles,
      throttle: p.throttle,
      keyboard: p.keyboard,
      calibration: {
        'pad:a:1': { min: -1, centre: 0, max: 1 },
        'pad:a:0': { min: -1, centre: 0, max: 1 },
      },
      axes: Object.fromEntries([...Object.entries(p.axes)].reverse()),
      bindings: p.bindings,
      deviceClass: p.deviceClass,
      name: p.name,
      id: p.id,
      schema: p.schema,
    };
    const lines = serializeProfile(scrambled);
    expect(lines.indexOf('"schema"')).toBeLessThan(lines.indexOf('"id"'));
    expect(lines.indexOf('"pad:a:0"')).toBeLessThan(lines.indexOf('"pad:a:1"'));
    expect(lines.indexOf('"flight.pitch"', lines.indexOf('"axes"'))).toBeLessThan(
      lines.indexOf('"flight.roll"', lines.indexOf('"axes"')),
    );
    expect(lines.endsWith('\n')).toBe(true);
  });

  it('fills in everything a short document leaves out', () => {
    const r = parseControlsProfile(minimal());
    expect(r.ok).toBe(true);
    if (!r.ok) return;
    expect(r.profile.axes).toEqual({});
    expect(r.profile.keyboard).toEqual({ rampUpPerSecond: 3, returnPerSecond: 4 });
    expect(r.profile.throttle.keyRatePerSecond).toBe(0.5);
    expect(r.profile.toggles).toEqual({ airbrake: 'toggle', afterburner: 'toggle' });
  });

  it('lets a partial shape override only what it names', () => {
    const doc = { ...minimal(), axes: { 'flight.pitch': { gain: 1.5 } } };
    const r = parseControlsProfile(doc);
    expect(r.ok).toBe(true);
    if (r.ok) {
      expect(r.profile.axes['flight.pitch']).toEqual({ ...STICK_SHAPE, gain: 1.5 });
    }
  });
});

describe('controls/1 errors name the place with a JSON pointer', () => {
  it('rejects text that is not JSON, and a document that is not an object', () => {
    const r = parseControlsProfileText('{ nope');
    expect(r.ok).toBe(false);
    if (!r.ok) expect(r.errors[0]?.pointer).toBe('');
    expect(errorsOf([1, 2])).toEqual([['', 'expected an object']]);
    expect(errorsOf(null)).toEqual([['', 'expected an object']]);
  });

  it('rejects the wrong schema, a bad id, a bad name and a bad device class', () => {
    const bad = {
      ...minimal(),
      schema: 'controls/2',
      id: 'Has Space',
      name: '',
      deviceClass: 'foot',
    };
    const e = errorsOf(bad).map(([p]) => p);
    expect(e).toEqual(expect.arrayContaining(['/schema', '/id', '/name', '/deviceClass']));
  });

  it('reports missing required fields', () => {
    const e = errorsOf({ schema: 'controls/1' }).map(([p]) => p);
    expect(e).toEqual(expect.arrayContaining(['/id', '/name', '/deviceClass', '/bindings']));
  });

  it('rejects unknown keys wherever they are', () => {
    expect(errorsOf({ ...minimal(), colour: 1 })).toEqual([['/colour', 'unknown key "colour"']]);
    const doc = {
      ...minimal(),
      bindings: [{ action: 'flight.gear', source: { kind: 'key', code: 'KeyL' }, treshold: 0.5 }],
    };
    expect(errorsOf(doc)).toEqual([['/bindings/0/treshold', 'unknown key "treshold"']]);
  });

  it('points into the binding that is wrong', () => {
    const doc = {
      ...minimal(),
      bindings: [
        { action: 'flight.gear', source: { kind: 'key', code: 'KeyL' } },
        { action: 'flight.fly_away', source: { kind: 'key', code: 'KeyL' } },
        { action: 'flight.gear', source: { kind: 'knob', code: 'KeyL' } },
        { action: 'flight.gear', source: { kind: 'pad-axis', device: 'mouse', index: 0 } },
        { action: 'flight.gear', source: { kind: 'pad-axis', device: 'pad', index: 99 } },
        { action: 'flight.gear', source: { kind: 'hat', device: 'pad', hat: 0, dir: 'sideways' } },
        { action: 'flight.pitch', source: { kind: 'key', code: 'KeyW' }, direction: 2 },
        {
          action: 'flight.gear',
          source: { kind: 'pad-button', device: 'pad', index: 0 },
          threshold: 3,
        },
        { action: 'flight.gear' },
      ],
    };
    const e = errorsOf(doc).map(([p]) => p);
    expect(e).toEqual([
      '/bindings/1/action',
      '/bindings/2/source/kind',
      '/bindings/3/source/device',
      '/bindings/4/source/index',
      '/bindings/5/source/dir',
      '/bindings/6/direction',
      '/bindings/7/threshold',
      '/bindings/8/source',
    ]);
  });

  it('needs an axis or a button for the absolute throttle', () => {
    const doc = {
      ...minimal(),
      bindings: [{ action: 'flight.throttle', source: { kind: 'key', code: 'KeyT' } }],
    };
    expect(errorsOf(doc)[0]?.[0]).toBe('/bindings/0/source/kind');
    const ok = {
      ...minimal(),
      bindings: [
        { action: 'flight.throttle', source: { kind: 'pad-axis', device: 'joy', index: 2 } },
      ],
    };
    expect(parseControlsProfile(ok).ok).toBe(true);
  });

  it('limits the bindings per action', () => {
    const keys = ['KeyA', 'KeyB', 'KeyC', 'KeyD', 'KeyE'];
    const doc = {
      ...minimal(),
      bindings: keys.map((code) => ({ action: 'flight.gear', source: { kind: 'key', code } })),
    };
    expect(errorsOf(doc)).toEqual([
      [
        '/bindings/4/action',
        `"flight.gear" has more than ${String(MAX_BINDINGS_PER_ACTION)} bindings`,
      ],
    ]);
  });

  it('checks shapes: ranges, curve kinds, and travel left after the dead zones', () => {
    const doc = {
      ...minimal(),
      axes: {
        'flight.pitch': { deadCentre: 0.7, gain: 9, curve: { kind: 'expo', k: 2 }, tauMs: 'slow' },
        'flight.roll': { deadCentre: 0.5, deadOuter: 0.5 },
        'flight.yaw': { curve: { kind: 'wobble' } },
        'flight.gear': {},
        'flight.fly_away': {},
      },
    };
    const e = errorsOf(doc).map(([p]) => p);
    expect(e).toEqual(
      expect.arrayContaining([
        '/axes/flight.pitch/deadCentre',
        '/axes/flight.pitch/gain',
        '/axes/flight.pitch/curve/k',
        '/axes/flight.pitch/tauMs',
        '/axes/flight.roll',
        '/axes/flight.yaw/curve/kind',
        '/axes/flight.gear',
        '/axes/flight.fly_away',
      ]),
    );
  });

  it('checks calibration: the key form and the order min <= centre <= max', () => {
    const doc = {
      ...minimal(),
      calibration: {
        'bogus-key': { min: -1, centre: 0, max: 1 },
        'pad:a:0': { min: 1, centre: 0, max: -1 },
        'pad:a:1': { min: -0.05, centre: 0, max: 0.05 },
        'pad:a:2': { min: -1, centre: 0 },
      },
    };
    expect(errorsOf(doc).map(([p]) => p)).toEqual(
      expect.arrayContaining([
        '/calibration/bogus-key',
        '/calibration/pad:a:0',
        '/calibration/pad:a:1',
        '/calibration/pad:a:2/max',
      ]),
    );
  });

  it('checks the keyboard, throttle and toggle sections', () => {
    const doc = {
      ...minimal(),
      keyboard: { rampUpPerSecond: 0 },
      throttle: { wheelStep: 5 },
      toggles: { airbrake: 'maybe' },
    };
    expect(errorsOf(doc).map(([p]) => p)).toEqual([
      '/keyboard/rampUpPerSecond',
      '/throttle/wheelStep',
      '/toggles/airbrake',
    ]);
  });

  it('escapes / and ~ in pointers', () => {
    const doc = { ...minimal(), calibration: { 'a/b~c': {} } };
    expect(errorsOf(doc)[0]?.[0]).toBe('/calibration/a~1b~0c');
  });

  it('lists every problem at once, not only the first', () => {
    const doc = {
      schema: 'nope',
      id: '!',
      name: 5,
      deviceClass: 'x',
      bindings: 'none',
      axes: 3,
    };
    expect(errorsOf(doc).length).toBeGreaterThanOrEqual(6);
  });
});

describe('choosing a profile for the devices connected', () => {
  const all = builtinProfiles();

  it('uses the keyboard profile with nothing else connected', () => {
    expect(chooseProfile(all, []).id).toBe('default-km');
  });

  it('uses the gamepad profile for a standard pad and the joystick profile for a raw device', () => {
    expect(chooseProfile(all, ['pad']).id).toBe('gamepad-standard');
    expect(chooseProfile(all, ['joy:044f:b10a']).id).toBe('generic-joystick');
    expect(chooseProfile(all, ['pad', 'joy']).id).toBe('generic-joystick');
  });

  it('keeps the active profile while its devices are there, and falls back when they are not', () => {
    const mine = builtinProfile('gamepad-standard');
    mine.id = 'mine';
    const profiles = [...all, mine];
    expect(chooseProfile(profiles, ['pad'], 'mine').id).toBe('mine');
    expect(chooseProfile(profiles, [], 'mine').id).toBe('default-km');
    // The player's choice is not forgotten: the pad coming back returns it (AC-58.6).
    expect(chooseProfile(profiles, ['pad'], 'mine').id).toBe('mine');
  });

  it('keeps a keyboard profile whatever is connected', () => {
    const mine = builtinProfile('default-km');
    mine.id = 'mine';
    expect(chooseProfile([...all, mine], ['pad'], 'mine').id).toBe('mine');
  });

  it('ignores an active id that does not exist', () => {
    expect(chooseProfile(all, ['pad'], 'gone').id).toBe('gamepad-standard');
  });

  it('finds the devices a profile binds, and whether they are all connected', () => {
    const pad = builtinProfile('gamepad-standard');
    expect(profileDevices(pad)).toEqual(['pad']);
    expect(profileDevices(builtinProfile('default-km'))).toEqual([]);
    expect(profileSatisfied(pad, ['pad'])).toBe(true);
    expect(profileSatisfied(pad, ['joy'])).toBe(false);
    const joy = builtinProfile('generic-joystick');
    expect(profileSatisfied(joy, ['joy:046d:c215'])).toBe(true);
    expect(profileSatisfied(joy, [])).toBe(false);
  });
});
