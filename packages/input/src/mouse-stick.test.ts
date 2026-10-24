import { describe, expect, it } from 'vitest';
import { MouseStick, STICK_RANGE_PX } from './mouse-stick.ts';

describe('MouseStick', () => {
  it('moves right and back with the mouse, full deflection over the stick range', () => {
    const s = new MouseStick();
    s.move(STICK_RANGE_PX / 2, 0, 1);
    expect(s.x).toBeCloseTo(0.5, 12);
    s.move(0, STICK_RANGE_PX / 4, 1);
    expect(s.y).toBeCloseTo(0.25, 12); // back = nose up
  });

  it('scales with the sensitivity and clamps to -1..1', () => {
    const s = new MouseStick();
    s.move(STICK_RANGE_PX / 3, 0, 3);
    expect(s.x).toBe(1);
    s.move(-10 * STICK_RANGE_PX, -10 * STICK_RANGE_PX, 1);
    expect([s.x, s.y]).toEqual([-1, -1]);
  });

  it('stays put with no spring', () => {
    const s = new MouseStick();
    s.move(100, 100, 1);
    const x = s.x;
    s.update(5000, 0);
    expect(s.x).toBe(x);
  });

  it('returns to centre with the spring: full spring in about half a second', () => {
    const s = new MouseStick();
    s.move(STICK_RANGE_PX, 0, 1);
    s.update(250, 1);
    expect(s.x).toBeCloseTo(Math.exp(-1), 6);
    s.update(10_000, 1);
    expect(s.x).toBe(0);
    expect(Object.is(s.x, -0)).toBe(false);
  });

  it('returns slower with a weaker spring', () => {
    const a = new MouseStick();
    const b = new MouseStick();
    a.move(STICK_RANGE_PX, 0, 1);
    b.move(STICK_RANGE_PX, 0, 1);
    a.update(250, 1);
    b.update(250, 0.25);
    expect(b.x).toBeGreaterThan(a.x);
  });

  it('resets', () => {
    const s = new MouseStick();
    s.move(100, 100, 1);
    s.reset();
    expect([s.x, s.y]).toEqual([0, 0]);
  });
});
