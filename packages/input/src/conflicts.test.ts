import { describe, expect, it } from 'vitest';
import { type ActionId } from './actions.ts';
import { conflictedBindings, findConflicts, reservedShortcutWarnings } from './conflicts.ts';
import { builtinProfile, builtinProfiles, type Binding, type ControlsProfile } from './profile.ts';
import { sourceKey, type Source } from './sources.ts';

const blank = (bindings: Binding[]): ControlsProfile => ({
  ...builtinProfile('default-km'),
  bindings,
});

/** One example of every kind of source, including both halves of an axis and every hat direction. */
const SOURCES: Source[] = [
  { kind: 'key', code: 'KeyX' },
  { kind: 'key', code: 'F5' },
  { kind: 'mouse-button', button: 0 },
  { kind: 'mouse-button', button: 2 },
  { kind: 'wheel', dir: 'up' },
  { kind: 'wheel', dir: 'down' },
  { kind: 'pad-button', device: 'pad', index: 0 },
  { kind: 'pad-button', device: 'joy:044f:b10a', index: 11 },
  { kind: 'pad-axis', device: 'pad', index: 1 },
  { kind: 'pad-axis', device: 'pad', index: 1, half: 'positive' },
  { kind: 'pad-axis', device: 'pad', index: 1, half: 'negative' },
  { kind: 'hat', device: 'joy', hat: 0, dir: 'up' },
  { kind: 'hat', device: 'joy', hat: 0, dir: 'right' },
  { kind: 'hat', device: 'joy', hat: 0, dir: 'down' },
  { kind: 'hat', device: 'joy', hat: 0, dir: 'left' },
  { kind: 'hat', device: 'joy', hat: 1, dir: 'up' },
];

const ACTION_SAMPLE: ActionId[] = [
  'flight.pitch',
  'flight.gear',
  'flight.brakes',
  'flight.afterburner',
  'flight.throttle_up',
  'system.respawn',
  'system.pause',
];

describe('findConflicts (AC-58.5: every duplicate is found)', () => {
  it('finds one source bound to two different actions, for every source and every action pair', () => {
    let checked = 0;
    for (const source of SOURCES) {
      for (const a of ACTION_SAMPLE) {
        for (const b of ACTION_SAMPLE) {
          if (a === b) continue;
          const p = blank([
            { action: a, source },
            { action: 'flight.recover', source: { kind: 'key', code: 'KeyU' } },
            { action: b, source },
          ]);
          const found = findConflicts(p);
          expect(found, `${sourceKey(source)} ${a}/${b}`).toHaveLength(1);
          expect(found[0]).toMatchObject({ kind: 'shared', bindings: [0, 2], actions: [a, b] });
          checked += 1;
        }
      }
    }
    expect(checked).toBe(SOURCES.length * ACTION_SAMPLE.length * (ACTION_SAMPLE.length - 1));
  });

  it('finds one source bound twice to the same action, for every source', () => {
    for (const source of SOURCES) {
      const p = blank([
        { action: 'flight.gear', source },
        { action: 'flight.gear', source, direction: -1 },
      ]);
      expect(findConflicts(p), sourceKey(source)).toEqual([
        expect.objectContaining({ kind: 'duplicate', bindings: [0, 1], actions: ['flight.gear'] }),
      ]);
    }
  });

  it('finds three or more actions on one control as one conflict', () => {
    const source: Source = { kind: 'key', code: 'KeyX' };
    const p = blank(
      (['flight.gear', 'flight.airbrake', 'flight.recover', 'flight.brakes'] as const).map(
        (action) => ({ action, source }),
      ),
    );
    const found = findConflicts(p);
    expect(found).toHaveLength(1);
    expect(found[0]?.bindings).toEqual([0, 1, 2, 3]);
    expect(found[0]?.actions).toHaveLength(4);
  });

  it('reports each clashing control separately', () => {
    const p = blank([
      { action: 'flight.gear', source: { kind: 'key', code: 'KeyA' } },
      { action: 'flight.recover', source: { kind: 'key', code: 'KeyA' } },
      { action: 'flight.airbrake', source: { kind: 'key', code: 'KeyB' } },
      { action: 'flight.brakes', source: { kind: 'key', code: 'KeyB' } },
      { action: 'flight.parking_brake', source: { kind: 'key', code: 'KeyC' } },
    ]);
    expect(findConflicts(p).map((c) => c.bindings)).toEqual([
      [0, 1],
      [2, 3],
    ]);
  });

  it('does not call different controls a conflict: every source against every other', () => {
    for (const a of SOURCES) {
      for (const b of SOURCES) {
        if (sourceKey(a) === sourceKey(b)) continue;
        const wholeAndHalf =
          a.kind === 'pad-axis' &&
          b.kind === 'pad-axis' &&
          a.index === b.index &&
          a.device === b.device
            ? (a.half === undefined) !== (b.half === undefined)
            : false;
        if (wholeAndHalf) continue; // that one is an overlap, tested below
        const p = blank([
          { action: 'flight.gear', source: a },
          { action: 'flight.recover', source: b },
        ]);
        expect(findConflicts(p), `${sourceKey(a)} vs ${sourceKey(b)}`).toEqual([]);
      }
    }
  });

  it('allows the two halves of an axis on two actions, and the same index on two devices', () => {
    const p = blank([
      {
        action: 'flight.throttle_up',
        source: { kind: 'pad-axis', device: 'pad', index: 1, half: 'negative' },
      },
      {
        action: 'flight.throttle_down',
        source: { kind: 'pad-axis', device: 'pad', index: 1, half: 'positive' },
      },
      { action: 'flight.roll', source: { kind: 'pad-axis', device: 'joy', index: 1 } },
    ]);
    expect(findConflicts(p)).toEqual([]);
  });

  it('calls a whole axis alongside a half of it an overlap', () => {
    const p = blank([
      { action: 'flight.roll', source: { kind: 'pad-axis', device: 'pad', index: 0 } },
      {
        action: 'flight.gear',
        source: { kind: 'pad-axis', device: 'pad', index: 0, half: 'positive' },
      },
      {
        action: 'flight.recover',
        source: { kind: 'pad-axis', device: 'pad', index: 0, half: 'negative' },
      },
    ]);
    const found = findConflicts(p);
    expect(found).toHaveLength(1);
    expect(found[0]).toMatchObject({ kind: 'overlap', bindings: [0, 1, 2] });
  });

  it('lists the bindings to tag in the editor', () => {
    const p = blank([
      { action: 'flight.gear', source: { kind: 'key', code: 'KeyA' } },
      { action: 'flight.recover', source: { kind: 'key', code: 'KeyZ' } },
      { action: 'flight.airbrake', source: { kind: 'key', code: 'KeyA' } },
    ]);
    expect([...conflictedBindings(findConflicts(p))]).toEqual([0, 2]);
  });

  it('finds no conflict in any built-in profile', () => {
    for (const p of builtinProfiles()) expect(findConflicts(p), p.id).toEqual([]);
  });
});

describe('reservedShortcutWarnings (GDD 58.3)', () => {
  const key = (action: ActionId, code: string): Binding => ({
    action,
    source: { kind: 'key', code },
  });

  it('always warns about function keys and OS keys', () => {
    const w = reservedShortcutWarnings(
      blank([
        key('flight.gear', 'F5'),
        key('flight.recover', 'F11'),
        key('flight.airbrake', 'MetaLeft'),
        key('flight.brakes', 'F8'),
      ]),
    );
    expect(w.map((x) => [x.code, x.reason])).toEqual([
      ['F5', 'function-key'],
      ['F11', 'function-key'],
      ['MetaLeft', 'os-key'],
    ]);
  });

  it('warns about letters only when Ctrl or Alt is bound too (Ctrl+W closes the tab)', () => {
    const without = reservedShortcutWarnings(
      blank([key('flight.pitch', 'KeyW'), key('flight.gear', 'KeyL')]),
    );
    expect(without).toEqual([]);
    const withCtrl = reservedShortcutWarnings(
      blank([
        key('flight.pitch', 'KeyW'),
        key('flight.gear', 'KeyL'),
        key('flight.throttle_down', 'ControlLeft'),
        key('flight.recover', 'KeyU'),
        key('flight.trim_reset', 'Backslash'),
      ]),
    );
    expect(withCtrl.map((x) => [x.code, x.reason])).toEqual([
      ['KeyW', 'browser-chord'],
      ['KeyL', 'browser-chord'],
      ['KeyU', 'browser-chord'],
    ]);
    expect(withCtrl[0]).toMatchObject({ binding: 0, action: 'flight.pitch' });
  });

  it('is quiet about the modifier itself and about pad and mouse bindings', () => {
    const w = reservedShortcutWarnings(
      blank([
        key('flight.throttle_down', 'ControlLeft'),
        { action: 'flight.gear', source: { kind: 'pad-button', device: 'pad', index: 3 } },
      ]),
    );
    expect(w).toEqual([]);
  });

  it('warns about the keyboard defaults that Ctrl makes risky, and not about F8 or Escape', () => {
    const km = builtinProfile('default-km');
    const codes = reservedShortcutWarnings(km).map((x) => x.code);
    expect(codes).toEqual(expect.arrayContaining(['KeyW', 'KeyR', 'KeyS', 'KeyL', 'KeyP']));
    expect(codes).not.toContain('F8');
    expect(codes).not.toContain('Escape');
    expect(codes).not.toContain('ControlLeft');
    // PageUp / PageDown are the throttle keys that have no such side effects when pressed alone.
    expect(km.bindings.some((b) => b.source.kind === 'key' && b.source.code === 'PageUp')).toBe(
      true,
    );
  });
});
