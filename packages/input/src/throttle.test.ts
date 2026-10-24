import { describe, expect, it } from 'vitest';
import { DETENT_PAUSE_MS, MIL_DETENT, ThrottleController } from './throttle.ts';

/** Holds the lever up at `rate` for `ms` in 4 ms steps. */
function hold(t: ThrottleController, ms: number, rate: number): number {
  for (let elapsed = 0; elapsed < ms; elapsed += 4) t.step(4, rate);
  return t.value;
}

describe('ThrottleController: moving the lever', () => {
  it('starts where it is told and clamps to 0..1', () => {
    expect(new ThrottleController(0.25).value).toBe(0.25);
    expect(new ThrottleController(7).value).toBe(1);
    const t = new ThrottleController(0.5);
    t.set(-3);
    expect(t.value).toBe(0);
    t.set(0.3);
    expect(t.value).toBe(0.3);
  });

  it('lowers at the given rate with no hold-up', () => {
    const t = new ThrottleController(1);
    expect(hold(t, 1000, -0.5)).toBeCloseTo(0.5, 6);
    expect(hold(t, 5000, -0.5)).toBe(0);
  });

  it('raises at the given rate below the detent', () => {
    const t = new ThrottleController(0.2);
    expect(hold(t, 400, 0.5)).toBeCloseTo(0.4, 6);
  });
});

describe('ThrottleController: the military-power detent', () => {
  it('stops exactly on 0.8 on the way up and waits 300 ms before passing it', () => {
    const t = new ThrottleController(0.5);
    // 0.3 of travel at 0.5/s takes 0.6 s.
    expect(hold(t, 600, 0.5)).toBeCloseTo(MIL_DETENT, 6);
    expect(t.value).toBe(MIL_DETENT);
    // Held on: it waits, then passes.
    expect(hold(t, DETENT_PAUSE_MS - 40, 0.5)).toBe(MIL_DETENT);
    hold(t, 100, 0.5);
    expect(t.value).toBeGreaterThan(MIL_DETENT);
  });

  it('counts the part of the step that arrived at the detent towards the pause', () => {
    const t = new ThrottleController(0.79);
    t.step(100, 0.5); // 0.01 would arrive after 20 ms: 80 ms of the step is left over
    expect(t.value).toBe(MIL_DETENT);
    t.step(DETENT_PAUSE_MS - 80 - 4, 0.5);
    expect(t.value).toBe(MIL_DETENT);
    t.step(8, 0.5);
    expect(t.value).toBeGreaterThan(MIL_DETENT);
  });

  it('asks for the pause again after the key is let go and pressed again', () => {
    const t = new ThrottleController(0.5);
    hold(t, 1000, 0.5);
    expect(t.value).toBeGreaterThan(MIL_DETENT);
    // Back down through the detent without a stop, then up again: stops again.
    hold(t, 2000, -0.5);
    expect(t.value).toBeLessThan(MIL_DETENT);
    hold(t, 2000, 0.5);
    expect(t.value).toBeGreaterThanOrEqual(MIL_DETENT);
    t.set(MIL_DETENT);
    t.step(4, 0);
    hold(t, 100, 0.5); // pressed again on the detent: waits again
    expect(t.value).toBe(MIL_DETENT);
  });

  it('does not hold up a lever already above the detent', () => {
    const t = new ThrottleController(0.9);
    expect(hold(t, 100, 0.5)).toBeCloseTo(0.95, 6);
  });

  it('does not hold up lowering, even from the detent', () => {
    const t = new ThrottleController(MIL_DETENT);
    t.step(100, -0.5);
    expect(t.value).toBeCloseTo(0.75, 6);
  });
});

describe('ThrottleController: notches, afterburner, absolute', () => {
  it('lands on the detent with the first notch up and passes it with the next', () => {
    const t = new ThrottleController(0.78);
    t.nudge(0.05);
    expect(t.value).toBe(MIL_DETENT);
    t.nudge(0.05);
    expect(t.value).toBeCloseTo(0.85, 6);
    t.nudge(-0.1);
    expect(t.value).toBeCloseTo(0.75, 6);
  });

  it('clamps notches to the ends', () => {
    const t = new ThrottleController(0.99);
    t.nudge(0.5);
    expect(t.value).toBe(1);
    t.nudge(-5);
    expect(t.value).toBe(0);
  });

  it('toggles afterburner: full from below, military power from above', () => {
    const t = new ThrottleController(0.5);
    const toggled = (): number => {
      t.toggleAfterburner();
      return t.value;
    };
    expect(toggled()).toBe(1);
    expect(toggled()).toBe(MIL_DETENT);
    expect(toggled()).toBe(1);
    t.set(0.85);
    expect(toggled()).toBe(MIL_DETENT);
    t.set(MIL_DETENT);
    expect(toggled()).toBe(1);
  });

  it('holds afterburner while held and returns to military power on release', () => {
    const t = new ThrottleController(0.6);
    t.holdAfterburner(true);
    expect(t.value).toBe(1);
    t.holdAfterburner(false);
    expect(t.value).toBe(MIL_DETENT);
    // A lever already below military power is left alone on release.
    t.set(0.3);
    t.holdAfterburner(false);
    expect(t.value).toBe(0.3);
  });

  it('takes an absolute lever at once, through the detent', () => {
    const t = new ThrottleController(0.1);
    t.set(0.95);
    expect(t.value).toBe(0.95);
    hold(t, 100, -0.5);
    expect(t.value).toBeLessThan(0.95);
  });
});
