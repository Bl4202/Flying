import { describe, expect, it } from 'vitest';
import { levelDb, peakFrequency } from '../test-support/analysis.ts';
import {
  Biquad,
  blockCoefficient,
  dbToLin,
  linToDb,
  noise,
  noiseState,
  onePoleCoefficient,
  rmsDb,
} from './primitives.ts';

const noiseOf = (seed: number, n: number): Float32Array => {
  const s = noiseState(seed);
  return Float32Array.from({ length: n }, () => noise(s));
};

describe('decibels', () => {
  it('convert both ways', () => {
    expect(dbToLin(0)).toBe(1);
    expect(dbToLin(-6.0206)).toBeCloseTo(0.5, 4);
    expect(dbToLin(20)).toBeCloseTo(10, 9);
    expect(linToDb(0.1)).toBeCloseTo(-20, 9);
    expect(linToDb(0)).toBe(-200);
  });
});

describe('noise', () => {
  it('is the same every run for a seed, and different for another', () => {
    expect([...noiseOf(1, 5)]).toEqual([...noiseOf(1, 5)]);
    expect([...noiseOf(1, 5)]).not.toEqual([...noiseOf(2, 5)]);
  });
  it('is white, in [-1, 1), with a mean near zero and a power near one third', () => {
    const x = noiseOf(7, 200_000);
    const mean = x.reduce((s, v) => s + v, 0) / x.length;
    expect(Math.abs(mean)).toBeLessThan(0.01);
    let lo = Infinity;
    let hi = -Infinity;
    for (const v of x) {
      lo = Math.min(lo, v);
      hi = Math.max(hi, v);
    }
    expect(lo).toBeGreaterThanOrEqual(-1);
    expect(hi).toBeLessThan(1);
    expect(levelDb(x)).toBeCloseTo(10 * Math.log10(1 / 3), 1);
  });
  it('survives a zero seed', () => {
    expect(Math.abs(noise(noiseState(0)))).toBeGreaterThan(0);
  });
});

describe('Biquad', () => {
  const sine = (hz: number): Float32Array =>
    Float32Array.from({ length: 48_000 }, (_, i) => Math.sin((2 * Math.PI * hz * i) / 48_000));
  const gainDb = (f: Biquad, hz: number): number => {
    f.reset();
    const x = sine(hz);
    const y = Float32Array.from(x, (v) => f.process(v));
    return levelDb(y, 24_000) - levelDb(x, 24_000);
  };

  it('low-passes: flat below its corner, -3 dB at it, falling above', () => {
    const f = new Biquad('lowpass', 48_000, 1000, Math.SQRT1_2);
    expect(gainDb(f, 100)).toBeCloseTo(0, 1);
    expect(gainDb(f, 1000)).toBeCloseTo(-3, 0);
    expect(gainDb(f, 8000)).toBeLessThan(-30);
  });

  it('high-passes the other way', () => {
    const f = new Biquad('highpass', 48_000, 1000, Math.SQRT1_2);
    expect(gainDb(f, 8000)).toBeCloseTo(0, 1);
    expect(gainDb(f, 100)).toBeLessThan(-30);
  });

  it('band-passes with unity gain at the centre', () => {
    const f = new Biquad('bandpass', 48_000, 1000, 2);
    expect(gainDb(f, 1000)).toBeCloseTo(0, 1);
    expect(gainDb(f, 100)).toBeLessThan(-15);
    expect(gainDb(f, 10_000)).toBeLessThan(-15);
  });

  it('peaks by the gain asked for at its frequency', () => {
    const f = new Biquad('peaking', 48_000, 1000, 1.5, 4);
    expect(gainDb(f, 1000)).toBeCloseTo(4, 1);
    expect(gainDb(f, 100)).toBeCloseTo(0, 0);
  });

  it('keeps a bad frequency or Q from breaking it', () => {
    const f = new Biquad('lowpass', 48_000, 1e9, -5);
    const y = f.process(1);
    expect(Number.isFinite(y)).toBe(true);
  });

  it('can change its coefficients and clear its state', () => {
    const f = new Biquad('lowpass', 48_000, 1000, 0.7);
    f.process(1);
    f.process(0.5);
    expect(f.z[0]).not.toBe(0);
    f.reset();
    expect([f.z[0], f.z[1]]).toEqual([0, 0]);
    f.set('highpass', 48_000, 2000, 0.7);
    expect(f.c[0]).toBeGreaterThan(0.5);
  });
});

describe('smoothing coefficients', () => {
  it('move 63 per cent in the time asked', () => {
    // After `seconds` of samples at the per-sample coefficient the remainder is 1/e.
    const k = onePoleCoefficient(0.1, 48_000);
    expect((1 - k) ** 4800).toBeCloseTo(Math.exp(-1), 6);
    expect(blockCoefficient(0.1, 48_000, 128)).toBeCloseTo(1 - Math.exp(-128 / 4800), 12);
    expect(onePoleCoefficient(0, 48_000)).toBe(1);
  });
});

describe('rmsDb', () => {
  it('is the power of a range of samples', () => {
    expect(rmsDb([1, -1, 1, -1])).toBeCloseTo(0, 9);
    expect(rmsDb([0.5, 0.5, 0, 0], 0, 2)).toBeCloseTo(-6.02, 1);
  });
});

describe('a narrow tone through a band-pass', () => {
  it('keeps its frequency', () => {
    const f = new Biquad('bandpass', 48_000, 3200, 8);
    const x = Float32Array.from({ length: 16_384 }, (_, i) =>
      Math.sin((2 * Math.PI * 3200 * i) / 48_000),
    );
    const y = Float32Array.from(x, (v) => f.process(v));
    expect(peakFrequency(y, 48_000, 100, 12_000)).toBeCloseTo(3200, 0);
  });
});
