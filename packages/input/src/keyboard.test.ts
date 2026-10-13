import { describe, expect, it } from 'vitest';
import { AxisEmulator, DEFAULT_EMULATION, KeyboardState, ThrottleLever } from './keyboard.ts';

describe('KeyboardState', () => {
  it('tracks keys by physical code', () => {
    const k = new KeyboardState();
    expect(k.keyDown('KeyW', false)).toBe(true);
    expect(k.isDown('KeyW')).toBe(true);
    expect(k.isDown('KeyS')).toBe(false);
    expect(k.keyUp('KeyW')).toBe(true);
    expect(k.isDown('KeyW')).toBe(false);
  });

  it('ignores OS auto-repeat and presses of a key already down', () => {
    const k = new KeyboardState();
    expect(k.keyDown('KeyA', false)).toBe(true);
    expect(k.keyDown('KeyA', true)).toBe(false);
    expect(k.keyDown('KeyA', false)).toBe(false);
    expect(k.count).toBe(1);
    // A repeat for a key we never saw go down does not invent a press.
    expect(k.keyDown('KeyZ', true)).toBe(false);
    expect(k.isDown('KeyZ')).toBe(false);
  });

  it('reports releases of keys that were not down as no change', () => {
    expect(new KeyboardState().keyUp('KeyQ')).toBe(false);
  });

  it('answers whether any of several keys is down, and releases all at once', () => {
    const k = new KeyboardState();
    k.keyDown('ShiftRight', false);
    expect(k.anyDown(['ShiftLeft', 'ShiftRight'])).toBe(true);
    expect(k.anyDown(['ControlLeft'])).toBe(false);
    expect(k.anyDown([])).toBe(false);
    k.keyDown('KeyW', false);
    k.releaseAll();
    expect(k.count).toBe(0);
  });
});

/** Runs the emulator for `ms` in `step`-millisecond steps with the keys held as given. */
function run(e: AxisEmulator, ms: number, neg: boolean, pos: boolean, step = 8): number {
  let v = e.value;
  for (let t = 0; t < ms - 1e-9; t += step) v = e.update(Math.min(step, ms - t), neg, pos);
  return v;
}

describe('AxisEmulator (GDD 58.2: ramp 3.0 per second, return 4.0 per second)', () => {
  it('has the documented defaults', () => {
    expect(DEFAULT_EMULATION).toEqual({ rampUpPerSecond: 3, returnPerSecond: 4 });
  });

  it('ramps to full in a third of a second and holds', () => {
    const e = new AxisEmulator();
    expect(run(e, 100, false, true)).toBeCloseTo(0.3, 9);
    expect(run(e, 233.333_333, false, true)).toBeCloseTo(1, 4);
    expect(run(e, 2000, false, true)).toBe(1);
  });

  it('ramps negative with the negative key', () => {
    const e = new AxisEmulator();
    expect(run(e, 150, true, false)).toBeCloseTo(-0.45, 9);
  });

  it('returns to zero at 4 per second when released, and stops there', () => {
    const e = new AxisEmulator();
    run(e, 1000, false, true);
    expect(run(e, 100, false, false)).toBeCloseTo(0.6, 9);
    expect(run(e, 150, false, false)).toBe(0);
    expect(Object.is(run(e, 500, false, false), 0)).toBe(true); // positive zero
  });

  it('cancels when both keys are held', () => {
    const e = new AxisEmulator();
    run(e, 200, false, true);
    expect(run(e, 1000, true, true)).toBe(0);
  });

  it('reverses through zero at the return rate, then ramps the new direction', () => {
    const e = new AxisEmulator();
    run(e, 1000, false, true); // +1
    // 250 ms to reach zero at 4/s, then 3/s the other way.
    expect(run(e, 250, true, false)).toBeCloseTo(0, 9);
    expect(run(e, 100, true, false)).toBeCloseTo(-0.3, 9);
    const f = new AxisEmulator();
    run(f, 1000, false, true);
    expect(f.update(300, true, false)).toBeCloseTo(-0.15, 9); // one step crossing zero: 250 ms + 50 ms
  });

  it('does not depend on how time is divided', () => {
    const [a, b, c] = [new AxisEmulator(), new AxisEmulator(), new AxisEmulator()];
    expect(run(a, 200, false, true, 1)).toBeCloseTo(run(b, 200, false, true, 50), 9);
    expect(run(c, 200, false, true, 200)).toBeCloseTo(0.6, 9);
  });

  it('never leaves -1..1, even for a huge step', () => {
    const e = new AxisEmulator();
    expect(e.update(1e9, false, true)).toBe(1);
    expect(e.update(1e9, true, false)).toBe(-1);
    expect(e.update(-50, true, false)).toBe(-1); // time does not run backwards
  });

  it('can be reset and tuned', () => {
    const e = new AxisEmulator({ rampUpPerSecond: 10, returnPerSecond: 10 });
    expect(run(e, 100, false, true)).toBeCloseTo(1, 9);
    e.reset();
    expect(e.value).toBe(0);
  });
});

describe('ThrottleLever', () => {
  it('starts mid-travel and moves at the given rate, within 0..1', () => {
    const t = new ThrottleLever();
    expect(t.value).toBe(0.5);
    expect(t.update(1000, 0.25)).toBeCloseTo(0.75, 12);
    expect(t.update(10_000, 1)).toBe(1);
    expect(t.update(10_000, -1)).toBe(0);
    t.set(0.3);
    expect(t.value).toBe(0.3);
    t.set(7);
    expect(t.value).toBe(1);
    expect(new ThrottleLever(-3).value).toBe(0);
  });
});
