import { describe, expect, it } from 'vitest';
import {
  ACTION_CATALOGUE_VERSION,
  ACTION_COUNT,
  ACTIONS,
  ACTIONS_ADDED_IN,
  actionDef,
  DEVICE_CLASSES,
  isActionId,
} from './actions.ts';

describe('the action catalogue', () => {
  it('has unique ids of the form group.name', () => {
    const ids = ACTIONS.map((a) => a.id);
    expect(new Set(ids).size).toBe(ids.length);
    for (const id of ids) expect(id).toMatch(/^(flight|system)\.[a-z_]+$/);
    expect(ACTION_COUNT).toBe(ids.length);
  });

  it('names every label by a strings key derived from the id', () => {
    for (const a of ACTIONS) expect(a.label).toBe(`action.${a.id}`);
  });

  it('gives a range to axes and only to axes', () => {
    for (const a of ACTIONS) {
      if (a.kind === 'axis') expect(['bipolar', 'unipolar']).toContain(a.range);
      else expect(a).not.toHaveProperty('range');
    }
  });

  it('lists the three flight axes as bipolar and the throttle and brakes as unipolar', () => {
    for (const id of ['flight.pitch', 'flight.roll', 'flight.yaw']) {
      expect(actionDef(id)?.range).toBe('bipolar');
    }
    for (const id of ['flight.throttle', 'flight.brake_left', 'flight.brake_right']) {
      expect(actionDef(id)?.range).toBe('unipolar');
    }
  });

  it('keeps pause on the menu layer and everything else on the flight layer', () => {
    expect(ACTIONS.filter((a) => a.context === 'menu').map((a) => a.id)).toEqual(['system.pause']);
  });

  it('classifies the toggles, holds and rates the pump relies on', () => {
    const kind = (id: string) => actionDef(id)?.kind;
    expect(kind('flight.afterburner')).toBe('toggle');
    expect(kind('flight.airbrake')).toBe('toggle');
    expect(kind('flight.throttle_up')).toBe('rate');
    expect(kind('flight.throttle_down')).toBe('rate');
    for (const id of ['flight.brakes', 'flight.recover', 'flight.trim_nose_up']) {
      expect(kind(id)).toBe('hold');
    }
    for (const id of ['flight.gear', 'flight.flaps_cycle', 'system.respawn']) {
      expect(kind(id)).toBe('button');
    }
  });

  it('has the view actions in their own group, as presses and holds the pump reports as view input', () => {
    const view = ACTIONS.filter((a) => a.group === 'view').map((a) => a.id);
    expect(view).toEqual([
      'flight.mouse_mode',
      'flight.camera_cycle',
      'flight.free_look',
      'flight.zoom',
      'flight.look_left',
      'flight.look_right',
      'flight.look_up',
      'flight.look_down',
      'flight.look_recentre',
      'flight.hud_declutter',
      'flight.hud_brightness_up',
      'flight.hud_brightness_down',
    ]);
    const kind = (id: string) => actionDef(id)?.kind;
    for (const id of ['flight.free_look', 'flight.zoom', 'flight.look_up', 'flight.look_left']) {
      expect(kind(id)).toBe('hold');
    }
    for (const id of ['flight.camera_cycle', 'flight.look_recentre', 'flight.hud_declutter']) {
      expect(kind(id)).toBe('button');
    }
  });

  it('records which actions each catalogue version added, all of them real', () => {
    expect(ACTION_CATALOGUE_VERSION).toBe(3);
    for (const [version, ids] of Object.entries(ACTIONS_ADDED_IN)) {
      expect(Number(version)).toBeLessThanOrEqual(ACTION_CATALOGUE_VERSION);
      for (const id of ids) expect(isActionId(id), id).toBe(true);
    }
    // Everything in the catalogue was in version 1 or was added later, once.
    const added = Object.values(ACTIONS_ADDED_IN).flat();
    expect(new Set(added).size).toBe(added.length);
  });

  it('looks actions up by id', () => {
    expect(actionDef('flight.gear')?.group).toBe('systems');
    expect(actionDef('flight.nope')).toBeUndefined();
    expect(isActionId('flight.pitch')).toBe(true);
    expect(isActionId('pitch')).toBe(false);
  });

  it('names three device classes', () => {
    expect(DEVICE_CLASSES).toEqual(['keyboard-mouse', 'gamepad', 'joystick']);
  });
});
