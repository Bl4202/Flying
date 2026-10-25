import { describe, expect, it } from 'vitest';
import { ACTIONS } from './actions.ts';
import { findConflicts } from './conflicts.ts';
import {
  addBinding,
  bindingsOf,
  defaultsFor,
  duplicateProfile,
  hasRoom,
  newProfile,
  removeBinding,
  renameProfile,
  restoreAction,
  restoreAll,
  restoreGroup,
  setShape,
  uniqueProfileId,
} from './profile-edit.ts';
import {
  builtinProfile,
  MAX_BINDINGS_PER_ACTION,
  parseControlsProfileText,
  serializeProfile,
} from './profile.ts';
import { STICK_SHAPE } from './shaping.ts';

describe('profile edits', () => {
  const km = () => builtinProfile('default-km');

  it('never change the profile they are given', () => {
    const p = km();
    const before = serializeProfile(p);
    addBinding(p, { action: 'flight.gear', source: { kind: 'key', code: 'KeyG' } });
    removeBinding(p, 0);
    restoreAll(p);
    setShape(p, 'flight.pitch', { gain: 2 });
    renameProfile(p, 'x');
    expect(serializeProfile(p)).toBe(before);
  });

  it('adds a binding up to the most an action can have, then refuses', () => {
    let p = km();
    const room = MAX_BINDINGS_PER_ACTION - bindingsOf(p, 'flight.gear').length;
    for (let i = 0; i < room; i++) {
      const next = addBinding(p, {
        action: 'flight.gear',
        source: { kind: 'key', code: `Digit${String(i + 1)}` },
      });
      expect(next).toBeDefined();
      if (next) p = next;
    }
    expect(hasRoom(p, 'flight.gear')).toBe(false);
    expect(
      addBinding(p, { action: 'flight.gear', source: { kind: 'key', code: 'KeyG' } }),
    ).toBeUndefined();
    expect(bindingsOf(p, 'flight.gear')).toHaveLength(MAX_BINDINGS_PER_ACTION);
  });

  it('removes the binding at a position', () => {
    const p = km();
    const [first] = bindingsOf(p, 'flight.gear');
    expect(first).toBeDefined();
    const q = removeBinding(p, first?.index ?? -1);
    expect(bindingsOf(q, 'flight.gear')).toHaveLength(0);
    expect(q.bindings).toHaveLength(p.bindings.length - 1);
  });

  it('hands back the same profile when a restore has nothing to restore', () => {
    const p = km();
    expect(restoreAction(p, 'flight.gear')).toBe(p);
    expect(restoreGroup(p, 'systems')).toBe(p);
    expect(restoreAction(p, 'flight.pitch')).toBe(p);
  });

  it('restores one action: its bindings and its shape, nothing else', () => {
    let p = km();
    p = removeBinding(p, bindingsOf(p, 'flight.gear')[0]?.index ?? 0);
    p = removeBinding(p, bindingsOf(p, 'flight.recover')[0]?.index ?? 0);
    p = setShape(p, 'flight.pitch', { gain: 2, invert: true });
    const q = restoreAction(p, 'flight.gear');
    expect(bindingsOf(q, 'flight.gear')).toHaveLength(1);
    expect(bindingsOf(q, 'flight.recover')).toHaveLength(0); // untouched
    expect(q.axes['flight.pitch']?.gain).toBe(2); // untouched
    const r = restoreAction(p, 'flight.pitch');
    expect(r.axes['flight.pitch']).toEqual(STICK_SHAPE);
    expect(bindingsOf(r, 'flight.pitch')).toHaveLength(2);
  });

  it('restores a group', () => {
    let p = km();
    for (const a of ['flight.gear', 'flight.flaps_cycle', 'flight.airbrake'] as const) {
      const found = bindingsOf(p, a)[0];
      if (found) p = removeBinding(p, found.index);
    }
    p = removeBinding(p, bindingsOf(p, 'flight.brakes')[0]?.index ?? 0);
    const q = restoreGroup(p, 'systems');
    expect(bindingsOf(q, 'flight.gear')).toHaveLength(1);
    expect(bindingsOf(q, 'flight.flaps_cycle')).toHaveLength(1);
    expect(bindingsOf(q, 'flight.airbrake')).toHaveLength(1);
    expect(bindingsOf(q, 'flight.brakes')).toHaveLength(0); // another group
  });

  it('restores everything but keeps the id, the name and the class', () => {
    let p = { ...km(), id: 'mine', name: 'Mine' };
    p = { ...p, bindings: [], keyboard: { rampUpPerSecond: 9, returnPerSecond: 9 } };
    const q = restoreAll(p);
    expect([q.id, q.name, q.deviceClass]).toEqual(['mine', 'Mine', 'keyboard-mouse']);
    expect(q.bindings).toEqual(km().bindings);
    expect(q.keyboard).toEqual(km().keyboard);
  });

  it("restores from the defaults of the profile's own class", () => {
    expect(defaultsFor(builtinProfile('gamepad-standard')).id).toBe('gamepad-standard');
    expect(defaultsFor(builtinProfile('generic-joystick')).id).toBe('generic-joystick');
    expect(defaultsFor(km()).id).toBe('default-km');
    const p = removeBinding(
      builtinProfile('gamepad-standard'),
      bindingsOf(builtinProfile('gamepad-standard'), 'flight.gear')[0]?.index ?? 0,
    );
    expect(
      bindingsOf(restoreAction(p, 'flight.gear'), 'flight.gear').map((b) => b.binding.source),
    ).toEqual([
      { kind: 'pad-button', device: 'pad', index: 3 },
      { kind: 'key', code: 'KeyL' },
    ]);
  });

  it('changes a shape field and keeps the others; ignores actions with no shape', () => {
    const p = setShape(km(), 'flight.roll', { gain: 1.5 });
    expect(p.axes['flight.roll']).toEqual({ ...STICK_SHAPE, gain: 1.5 });
    const same = setShape(km(), 'flight.gear', { gain: 1.5 });
    expect(same.axes['flight.gear' as 'flight.pitch']).toBeUndefined();
    // The curve object is not shared with the original.
    const q = setShape(p, 'flight.roll', { curve: { kind: 'power', gamma: 2 } });
    expect(p.axes['flight.roll']?.curve).toEqual({ kind: 'expo', k: 0.3 });
    expect(q.axes['flight.roll']?.curve).toEqual({ kind: 'power', gamma: 2 });
  });

  it("makes ids that are valid, unique, and not the built-ins'", () => {
    expect(uniqueProfileId([], 'My Stick!')).toBe('my-stick');
    expect(uniqueProfileId(['my-stick'], 'My Stick!')).toBe('my-stick-2');
    expect(uniqueProfileId(['my-stick', 'my-stick-2'], 'my stick')).toBe('my-stick-3');
    expect(uniqueProfileId([], 'default-km')).toBe('default-km-2');
    expect(uniqueProfileId([], '???')).toBe('profile');
    expect(uniqueProfileId([], 'x'.repeat(200))).toHaveLength(56);
  });

  it('duplicates, creates and renames profiles that still parse', () => {
    const dup = duplicateProfile(builtinProfile('gamepad-standard'), ['mine'], 'Mine');
    expect(dup.id).toBe('mine-2');
    expect(dup.name).toBe('Mine');
    expect(dup.bindings).toEqual(builtinProfile('gamepad-standard').bindings);
    const fresh = newProfile('joystick', [], '  Hotas  ');
    expect(fresh.id).toBe('hotas');
    expect(fresh.name).toBe('Hotas');
    expect(fresh.deviceClass).toBe('joystick');
    expect(fresh.bindings).toEqual(builtinProfile('generic-joystick').bindings);
    for (const p of [dup, fresh, renameProfile(dup, 'Renamed')]) {
      expect(parseControlsProfileText(serializeProfile(p)).ok).toBe(true);
    }
    expect(renameProfile(dup, 'x'.repeat(100)).name).toHaveLength(64);
  });

  it('leave the built-in defaults free of conflicts, so restoring never creates one', () => {
    for (const id of ['default-km', 'gamepad-standard', 'generic-joystick'] as const) {
      expect(findConflicts(restoreAll(builtinProfile(id)))).toEqual([]);
    }
    expect(ACTIONS.length).toBeGreaterThan(0);
  });
});
