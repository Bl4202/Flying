import { describe, expect, it } from 'vitest';
import { ACTION_COUNT, ACTIONS, actionDef, DEVICE_CLASSES, isActionId } from './actions.ts';

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
