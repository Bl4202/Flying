import { describe, expect, it } from 'vitest';
import { applyCalibration, CalibrationWizard, CENTRE_SAMPLES } from './calibrate.ts';
import { GamepadHub } from './devices/gamepad.ts';
import { builtinProfile, calibrationKey } from './profile.ts';
import { FakePads, fakeStick } from './test-support.ts';

function rig() {
  const pads = new FakePads();
  const hub = new GamepadHub(pads.get);
  const wizard = new CalibrationWizard();
  const feed = (axes: Record<number, number>): void => {
    pads.set(0, fakeStick({ axes }));
    hub.poll();
    const slot = hub.slots[0];
    if (slot) wizard.update(slot);
  };
  const rest = (axes: Record<number, number>, n = CENTRE_SAMPLES): void => {
    for (let i = 0; i < n; i++) feed(axes);
  };
  return { feed, rest, wizard, hub };
}

describe('CalibrationWizard', () => {
  it('records where an axis rests and how far it travels', () => {
    const r = rig();
    r.rest({ 0: 0.05, 2: -0.1 });
    expect(r.wizard.next()).toBe('range');
    r.feed({ 0: -0.9, 2: 0.7 });
    r.feed({ 0: 0.95, 2: -0.6 });
    r.feed({ 0: 0.05, 2: -0.1 });
    expect(r.wizard.next()).toBe('done');
    const a0 = r.wizard.result(0);
    expect(a0?.centre).toBeCloseTo(0.05, 9);
    expect(a0?.min).toBeCloseTo(-0.9, 9);
    expect(a0?.max).toBeCloseTo(0.95, 9);
    const a2 = r.wizard.result(2);
    expect(a2?.min).toBeCloseTo(-0.6, 9);
    expect(a2?.centre).toBeCloseTo(-0.1, 9);
    expect(a2?.max).toBeCloseTo(0.7, 9);
  });

  it('will not leave the centre step before it has enough readings', () => {
    const r = rig();
    r.rest({ 0: 0 }, CENTRE_SAMPLES - 1);
    expect(r.wizard.next()).toBe('centre');
    r.feed({ 0: 0 });
    expect(r.wizard.next()).toBe('range');
  });

  it('leaves an axis that did not move through its travel uncalibrated', () => {
    const r = rig();
    r.rest({ 0: 0, 1: 0 });
    r.wizard.next();
    r.feed({ 0: -1, 1: 0.1 });
    r.feed({ 0: 1, 1: -0.1 });
    r.wizard.next();
    expect(r.wizard.result(0)).toBeDefined();
    expect(r.wizard.result(1)).toBeUndefined();
    expect(r.wizard.results().map(([axis]) => axis)).toEqual([0]);
  });

  it('flags an axis that wandered while everything was at rest as a ghost, and skips it', () => {
    const r = rig();
    for (let i = 0; i < CENTRE_SAMPLES; i++) r.feed({ 0: 0, 3: i % 2 === 0 ? -0.3 : 0.3 });
    expect(r.wizard.isGhost(3)).toBe(true);
    expect(r.wizard.isGhost(0)).toBe(false);
    expect(r.wizard.ghosts()).toEqual([3]);
    r.wizard.next();
    r.feed({ 0: -1, 3: -1 });
    r.feed({ 0: 1, 3: 1 });
    r.wizard.next();
    expect(r.wizard.result(3)).toBeUndefined();
    expect(r.wizard.result(0)).toBeDefined();
  });

  it('skips the axis a hat is read from', () => {
    const r = rig();
    r.rest({ 0: 0 });
    r.wizard.next();
    r.feed({ 0: -1, 9: -1 });
    r.feed({ 0: 1, 9: 1 });
    r.wizard.next();
    expect(r.wizard.result(9)).toBeUndefined();
  });

  it('gives no results before it is done, and ignores readings after', () => {
    const r = rig();
    r.rest({ 0: 0 });
    r.wizard.next();
    r.feed({ 0: -1 });
    r.feed({ 0: 1 });
    expect(r.wizard.result(0)).toBeUndefined();
    r.wizard.next();
    const before = r.wizard.result(0);
    r.feed({ 0: -5 });
    expect(r.wizard.result(0)).toEqual(before);
    expect(r.wizard.next()).toBe('done');
  });

  it("stores the results in a copy of the profile under the device's name", () => {
    const r = rig();
    r.rest({ 0: 0 });
    r.wizard.next();
    r.feed({ 0: -1 });
    r.feed({ 0: 1 });
    r.wizard.next();
    const base = builtinProfile('generic-joystick');
    const out = applyCalibration(base, 'joy:044f:b10a', r.wizard);
    expect(out.calibration[calibrationKey('joy:044f:b10a', 0)]).toEqual({
      min: -1,
      centre: 0,
      max: 1,
    });
    expect(base.calibration).toEqual({}); // the original is untouched
  });
});
