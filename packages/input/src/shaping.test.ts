import { describe, expect, it } from 'vitest';
import {
  AxisShaper,
  calibrate,
  deadzone,
  expoCurve,
  IDENTITY_SHAPE,
  LINEAR_SHAPE,
  powerCurve,
  rateLimit,
  saturate,
  shapeStatic,
  smooth,
  STICK_SHAPE,
  type AxisShape,
} from './shaping.ts';

describe('calibrate', () => {
  it('is the identity for a standard axis', () => {
    for (const x of [-1, -0.3, 0, 0.7, 1]) expect(calibrate(x)).toBe(x);
  });
  it('stretches each side of an off-centre axis to the full range', () => {
    const c = { min: 0.1, centre: 0.4, max: 0.5 };
    expect(calibrate(0.4, c)).toBe(0);
    expect(calibrate(0.5, c)).toBe(1);
    expect(calibrate(0.1, c)).toBe(-1);
    expect(calibrate(0.25, c)).toBeCloseTo(-0.5, 12);
    expect(calibrate(0.45, c)).toBeCloseTo(0.5, 12);
  });
  it('clamps beyond the recorded range and distrusts garbage', () => {
    expect(calibrate(2)).toBe(1);
    expect(calibrate(-9)).toBe(-1);
    expect(calibrate(Number.NaN)).toBe(0);
    expect(calibrate(0.5, { min: 0, centre: 0, max: 0 })).toBe(0);
  });
});

describe('stage 1: deadzone (hand-computed)', () => {
  const dz = (x: number): number => deadzone(x, 0.03, 0.01);
  it('reads zero inside the centre band and full at the outer edge', () => {
    expect(dz(0)).toBe(0);
    expect(dz(0.03)).toBe(0);
    expect(Object.is(dz(-0.03), 0)).toBe(true); // positive zero, not -0
    expect(dz(0.99)).toBeCloseTo(1, 12);
    expect(dz(1)).toBe(1); // clamped
    expect(dz(-1)).toBe(-1);
  });
  it('rescales the live range linearly', () => {
    // (0.5 - 0.03) / 0.96
    expect(dz(0.5)).toBeCloseTo(0.489_583_333_333, 9);
    expect(dz(-0.5)).toBeCloseTo(-0.489_583_333_333, 9);
    // (0.04 - 0.03) / 0.96
    expect(dz(0.04)).toBeCloseTo(0.010_416_666_667, 9);
  });
  it('a dead band that leaves no travel reads zero rather than dividing by it', () => {
    expect(deadzone(0.7, 0.5, 0.5)).toBe(0);
    expect(deadzone(0.7, 0.6, 0.6)).toBe(0);
  });
});

describe('stage 2: curves (hand-computed)', () => {
  it('expo: (1 - k)|x| + k|x|^3', () => {
    expect(expoCurve(0.5, 0.3)).toBeCloseTo(0.7 * 0.5 + 0.3 * 0.125, 12); // 0.3875
    expect(expoCurve(0.5, 0.3)).toBeCloseTo(0.3875, 12);
    expect(expoCurve(1, 0.3)).toBe(1);
    expect(expoCurve(0.5, 0)).toBe(0.5);
    expect(expoCurve(0.5, 1)).toBeCloseTo(0.125, 12);
    expect(expoCurve(-0.5, 0.3)).toBeCloseTo(-0.3875, 12);
  });
  it('power: |x|^gamma', () => {
    expect(powerCurve(0.5, 2)).toBe(0.25);
    expect(powerCurve(-0.5, 2)).toBe(-0.25);
    expect(powerCurve(0.25, 0.5)).toBe(0.5);
    expect(powerCurve(0.8, 1)).toBeCloseTo(0.8, 12);
    expect(powerCurve(0, 0.5)).toBe(0);
  });
});

describe('stage 3: saturation', () => {
  it('multiplies and clamps', () => {
    expect(saturate(0.4, 2)).toBeCloseTo(0.8, 12);
    expect(saturate(0.6, 2)).toBe(1);
    expect(saturate(-0.6, 2)).toBe(-1);
    expect(saturate(0.6, 0.5)).toBeCloseTo(0.3, 12);
  });
});

describe('stage 4: smoothing', () => {
  it('passes straight through with no time constant', () => {
    expect(smooth(0, 1, 16, 0)).toBe(1);
  });
  it('closes 1 - 1/e of the gap in one time constant (AC-58.2: 63 % at tau +/- 5 %)', () => {
    for (const tau of [10, 20, 60, 120]) {
      for (const dt of [1, 4, 8, 16.667]) {
        let x = 0;
        let t = 0;
        while (x < 1 - 1 / Math.E) {
          x = smooth(x, 1, dt, tau);
          t += dt;
          if (t > 10 * tau) break;
        }
        // Discrete steps reach it at the first sample at or after tau.
        expect(t, `tau ${String(tau)} ms, dt ${String(dt)} ms`).toBeGreaterThanOrEqual(tau * 0.95);
        expect(t).toBeLessThanOrEqual(tau * 1.05 + dt);
      }
    }
  });
  it('is independent of how the same time is divided into steps', () => {
    let a = 0;
    for (let i = 0; i < 100; i++) a = smooth(a, 1, 1, 50);
    const b = smooth(0, 1, 100, 50);
    expect(a).toBeCloseTo(b, 12);
  });
});

describe('stage 5: rate limit', () => {
  it('moves at most maxRate * dt towards the target', () => {
    expect(rateLimit(0, 1, 0.1, 2)).toBeCloseTo(0.2, 12);
    expect(rateLimit(0, -1, 0.1, 2)).toBeCloseTo(-0.2, 12);
    expect(rateLimit(0.9, 1, 0.1, 2)).toBe(1); // close enough to arrive
    expect(rateLimit(0.5, 0.5, 0.1, 2)).toBe(0.5);
  });
});

/** The pipeline written out from the formulas in one expression, as the independent reference. */
function reference(x0: number, s: AxisShape): number {
  const a = Math.abs(x0);
  let x1 = (Math.max(0, a - s.deadCentre) / (1 - s.deadCentre - s.deadOuter)) * Math.sign(x0);
  x1 = Math.max(-1, Math.min(1, x1));
  const m = Math.abs(x1);
  const x2 =
    s.curve.kind === 'expo'
      ? Math.sign(x1) * ((1 - s.curve.k) * m + s.curve.k * m ** 3)
      : Math.sign(x1) * m ** s.curve.gamma;
  return Math.max(-1, Math.min(1, x2 * s.gain));
}

describe('stages 1 to 3 against the reference formulas (AC-58.2: 200 cases to 1e-6)', () => {
  const shapes: [string, AxisShape][] = [
    ['stick', STICK_SHAPE],
    ['linear', LINEAR_SHAPE],
    ['power 2', { ...STICK_SHAPE, curve: { kind: 'power', gamma: 2 } }],
    ['power 0.5, gain 1.5', { ...STICK_SHAPE, curve: { kind: 'power', gamma: 0.5 }, gain: 1.5 }],
    [
      'wide dead band',
      { ...STICK_SHAPE, deadCentre: 0.15, deadOuter: 0.05, curve: { kind: 'expo', k: 1 } },
    ],
    ['gain 2', { ...STICK_SHAPE, gain: 2 }],
  ];
  const grid = Array.from({ length: 201 }, (_, i) => -1 + (2 * i) / 200); // 201 points, -1 .. 1

  for (const [name, shape] of shapes) {
    it(`matches for ${name}`, () => {
      for (const x of grid) {
        expect(
          Math.abs(shapeStatic(x, shape) - reference(x, shape)),
          `x = ${String(x)}`,
        ).toBeLessThan(1e-6);
      }
    });
    it(`is odd, bounded and monotonic for ${name}`, () => {
      let last = -Infinity;
      for (const x of grid) {
        const y = shapeStatic(x, shape);
        expect(y).toBeGreaterThanOrEqual(-1);
        expect(y).toBeLessThanOrEqual(1);
        expect(shapeStatic(-x, shape)).toBeCloseTo(-y, 12);
        expect(y).toBeGreaterThanOrEqual(last - 1e-12);
        last = y;
      }
      expect(shapeStatic(0, shape)).toBe(0);
    });
  }

  it('has a few anchor values worked out by hand for the default stick', () => {
    // x0 = 0.5: dz = 0.47 / 0.96 = 0.4895833..., curve = 0.7 * dz + 0.3 * dz^3
    const dz = 0.47 / 0.96;
    expect(shapeStatic(0.5, STICK_SHAPE)).toBeCloseTo(0.7 * dz + 0.3 * dz ** 3, 12);
    expect(shapeStatic(0.5, STICK_SHAPE)).toBeCloseTo(0.377_9, 3);
    expect(shapeStatic(0.02, STICK_SHAPE)).toBe(0); // inside the dead band
    expect(shapeStatic(1, STICK_SHAPE)).toBe(1);
    expect(shapeStatic(-1, STICK_SHAPE)).toBe(-1);
  });
});

describe('AxisShaper (the whole pipeline)', () => {
  it('starts from the first sample instead of easing in from zero', () => {
    const s = new AxisShaper({ ...LINEAR_SHAPE, deadCentre: 0, deadOuter: 0, tauMs: 100 });
    expect(s.step(0.8, 16)).toBeCloseTo(0.8, 12);
  });

  it('smooths a step with the time constant it was given', () => {
    const tau = 60;
    const s = new AxisShaper({ ...IDENTITY_SHAPE, tauMs: tau });
    s.step(0, 1);
    let t = 0;
    let y = 0;
    while (y < 1 - 1 / Math.E) {
      y = s.step(1, 1);
      t += 1;
    }
    expect(t).toBeGreaterThanOrEqual(tau * 0.95);
    expect(t).toBeLessThanOrEqual(tau * 1.05);
  });

  it('limits the rate of change after smoothing', () => {
    const s = new AxisShaper({ ...IDENTITY_SHAPE, maxRatePerSecond: 2 });
    s.step(0, 10);
    let last = 0;
    for (let i = 0; i < 100; i++) {
      const y = s.step(1, 10);
      expect(y - last).toBeLessThanOrEqual(0.02 + 1e-12); // 2 per second * 10 ms
      last = y;
    }
    expect(last).toBe(1);
  });

  it('inverts and trims last, and stays within -1..1', () => {
    const s = new AxisShaper({ ...IDENTITY_SHAPE, invert: true, offset: 0.1 });
    expect(s.step(0.5, 8)).toBeCloseTo(-0.4, 12);
    const t = new AxisShaper({ ...IDENTITY_SHAPE, offset: 0.5 });
    expect(t.step(0.9, 8)).toBe(1);
    expect(new AxisShaper({ ...IDENTITY_SHAPE, offset: -0.5 }).step(-0.9, 8)).toBe(-1);
  });

  it('forgets its history on reset', () => {
    const s = new AxisShaper({ ...IDENTITY_SHAPE, tauMs: 1000 });
    s.step(1, 8);
    s.step(1, 8);
    s.reset();
    expect(s.step(-0.5, 8)).toBeCloseTo(-0.5, 12);
  });
});
