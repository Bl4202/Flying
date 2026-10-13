import { describe, expect, it } from 'vitest';
import { FixedStepClock } from './clock.ts';
import { MAX_CATCH_UP_TICKS, SIM_DT_MS } from './constants.ts';

const make = (): FixedStepClock =>
  new FixedStepClock({ dtMs: SIM_DT_MS, maxTicksPerIteration: MAX_CATCH_UP_TICKS });

/** A small deterministic generator, so "random" wake-up jitter is repeatable. */
function lcg(seed: number): () => number {
  let s = seed >>> 0;
  return () => {
    s = (Math.imul(s, 1664525) + 1013904223) >>> 0;
    return s / 2 ** 32;
  };
}

describe('FixedStepClock', () => {
  it('runs exactly floor(elapsed / dt) ticks over ten simulated minutes of jittered wake-ups', () => {
    const clock = make();
    const rand = lcg(7);
    const t0 = 12_345.678;
    let now = t0;
    clock.start(now);
    let ticks = 0;
    let busiest = 0;
    const end = t0 + 10 * 60 * 1000;
    while (now < end) {
      // Wake somewhere between on time and 4 ms late, as a timer would, sometimes in a hurry.
      const due = clock.nextDue();
      now = Math.max(now, due + rand() * 4);
      const n = clock.update(now);
      ticks += n;
      busiest = Math.max(busiest, n);
      expect(ticks, `after ${((now - t0) / 1000).toFixed(3)} s`).toBe(
        Math.floor((now - t0) / SIM_DT_MS),
      );
    }
    expect(ticks).toBeGreaterThanOrEqual(35_990);
    expect(ticks).toBeLessThanOrEqual(36_010);
    expect(clock.totalTicks).toBe(ticks);
    expect(clock.droppedMs).toBe(0);
    expect(busiest).toBeLessThanOrEqual(2);
  });

  it('holds 60.00 Hz on average even when every wake-up is late', () => {
    const clock = make();
    let now = 0;
    clock.start(now);
    for (let i = 0; i < 6000; i++) {
      now = clock.nextDue() + 3.9; // always the worst timer
      clock.update(now);
    }
    expect(clock.totalTicks / (now / 1000)).toBeCloseTo(60, 1);
    expect(clock.droppedMs).toBe(0);
  });

  it('gives a stalled worker five ticks, drops the rest, and starts a new epoch', () => {
    const clock = make();
    clock.start(0);
    let now = 0;
    for (let i = 0; i < 60; i++) {
      now = clock.nextDue();
      clock.update(now);
    }
    expect(clock.totalTicks).toBe(60);

    now += 200; // a 200 ms stall: 12 ticks owed
    expect(clock.update(now)).toBe(5);
    expect(clock.dropEvents).toBe(1);
    // 200 ms owed, 5 ticks (83.3 ms) run, the other 116.7 ms is given up.
    expect(clock.droppedMs).toBeGreaterThan(116);
    expect(clock.droppedMs).toBeLessThan(134);
    // Back to normal from the present: nothing owed until a full tick later.
    expect(clock.update(now + 1)).toBe(0);
    expect(clock.nextDue()).toBeCloseTo(now + SIM_DT_MS, 9);
    expect(clock.update(now + SIM_DT_MS)).toBe(1);
  });

  it('never returns more than the catch-up limit, whatever the stall', () => {
    const clock = make();
    const rand = lcg(99);
    let now = 0;
    clock.start(now);
    for (let i = 0; i < 5000; i++) {
      now += rand() < 0.05 ? rand() * 5000 : rand() * 30;
      expect(clock.update(now)).toBeLessThanOrEqual(MAX_CATCH_UP_TICKS);
    }
  });

  it('accounts for every millisecond: ticks run plus time dropped equals time passed (within a tick)', () => {
    const clock = make();
    let now = 0;
    clock.start(now);
    for (const stall of [400, 17, 1000, 90, 5]) {
      now += stall;
      clock.update(now);
    }
    const accounted = clock.totalTicks * SIM_DT_MS + clock.droppedMs;
    expect(Math.abs(accounted - now)).toBeLessThan(SIM_DT_MS * 5);
  });

  it('pauses without a burst on resume', () => {
    const clock = make();
    clock.start(0);
    expect(clock.update(SIM_DT_MS * 3)).toBe(3);
    clock.pause();
    expect(clock.running).toBe(false);
    expect(clock.update(10_000)).toBe(0);
    expect(clock.update(60_000)).toBe(0);
    clock.resume(60_000);
    expect(clock.running).toBe(true);
    expect(clock.update(60_000)).toBe(0);
    expect(clock.update(60_000 + SIM_DT_MS - 0.01)).toBe(0);
    expect(clock.update(60_000 + SIM_DT_MS)).toBe(1);
    expect(clock.dropEvents).toBe(0);
    expect(clock.droppedMs).toBe(0);
  });

  it('hands out nothing before it is started, and nothing when time goes backwards', () => {
    const clock = make();
    expect(clock.update(1000)).toBe(0);
    clock.start(1000);
    expect(clock.update(500)).toBe(0);
    expect(clock.update(1000)).toBe(0);
  });

  it('says when the next tick is due', () => {
    const clock = make();
    clock.start(100);
    expect(clock.nextDue()).toBeCloseTo(100 + SIM_DT_MS, 9);
    clock.update(100 + SIM_DT_MS * 2.5);
    expect(clock.nextDue()).toBeCloseTo(100 + SIM_DT_MS * 3, 9);
  });

  it('rejects nonsense options', () => {
    expect(() => new FixedStepClock({ dtMs: 0, maxTicksPerIteration: 5 })).toThrow(RangeError);
    expect(() => new FixedStepClock({ dtMs: 16, maxTicksPerIteration: 0 })).toThrow(RangeError);
    expect(() => new FixedStepClock({ dtMs: 16, maxTicksPerIteration: 2.5 })).toThrow(RangeError);
  });
});
