import { describe, expect, it } from 'vitest';
import { Counter, Gauge, Histogram } from './metrics.ts';

describe('Counter and Gauge', () => {
  it('count and hold', () => {
    const c = new Counter();
    c.inc();
    c.inc(4);
    expect(c.value).toBe(5);
    c.reset();
    expect(c.value).toBe(0);
    const g = new Gauge();
    g.set(2.5);
    expect(g.value).toBe(2.5);
  });
});

describe('Histogram', () => {
  it('is empty and says zero', () => {
    const h = new Histogram(10);
    expect([h.count, h.mean, h.min, h.peak, h.quantile(0.5), h.overflow]).toEqual([
      0, 0, 0, 0, 0, 0,
    ]);
  });

  it('puts quantiles on the upper edge of the right bin, never above the largest sample', () => {
    const h = new Histogram(100, 100); // 1-wide bins
    for (let v = 1; v <= 100; v++) h.record(v - 0.5); // 0.5, 1.5, ... 99.5
    expect(h.count).toBe(100);
    expect(h.quantile(0.5)).toBe(50);
    expect(h.quantile(0.95)).toBe(95);
    expect(h.quantile(0.99)).toBe(99);
    expect(h.quantile(1)).toBe(99.5); // clamped to the real peak
    expect(h.quantile(0)).toBe(1);
    expect(h.mean).toBeCloseTo(50, 9);
    expect(h.min).toBe(0.5);
    expect(h.peak).toBe(99.5);
  });

  it('keeps overflow, negative and non-finite samples from corrupting anything', () => {
    const h = new Histogram(10, 10);
    for (const v of [5, 50, -3, Number.NaN, Number.POSITIVE_INFINITY]) h.record(v);
    expect(h.count).toBe(5);
    expect(h.overflow).toBe(3); // 50, NaN, Infinity
    expect(h.quantile(1)).toBe(50);
    expect(Number.isFinite(h.mean)).toBe(true);
    h.reset();
    expect(h.count).toBe(0);
    expect(h.quantile(0.99)).toBe(0);
  });

  it('answers the p99 gate question', () => {
    const h = new Histogram(20, 200); // 0.1 ms bins
    for (let i = 0; i < 990; i++) h.record(1.2);
    for (let i = 0; i < 10; i++) h.record(6.9);
    expect(h.quantile(0.99)).toBeLessThanOrEqual(1.3);
    expect(h.quantile(0.995)).toBeGreaterThan(6.8);
    expect(h.quantile(0.995)).toBeLessThanOrEqual(6.9);
  });

  it('rejects nonsense shapes', () => {
    expect(() => new Histogram(0)).toThrow(RangeError);
    expect(() => new Histogram(10, 0)).toThrow(RangeError);
    expect(() => new Histogram(10, 1.5)).toThrow(RangeError);
  });
});
