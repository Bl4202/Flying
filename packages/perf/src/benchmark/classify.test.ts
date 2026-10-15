import { describe, expect, it } from 'vitest';
import {
  classify,
  THRESHOLDS,
  tierHigherIsBetter,
  tierLowerIsBetter,
  type ClassifyInput,
} from './classify.ts';
import type { CpuResult } from './types.ts';

const cpu = (ticks: number): CpuResult => ({
  singleTicksPerSecond: ticks,
  workers: 8,
  multiTicksPerSecond: ticks * 6,
  speedup: 6,
});

/** CPU rates placed against the shipped thresholds, so re-basing them does not break the tests. */
const CPU = {
  high: THRESHOLDS.cpu.high * 1.3,
  recommended: (THRESHOLDS.cpu.recommended + THRESHOLDS.cpu.high) / 2,
  minimum: (THRESHOLDS.cpu.minimum + THRESHOLDS.cpu.recommended) / 2,
  belowMinimum: THRESHOLDS.cpu.minimum / 2,
};

/** A machine comfortably in the top tier, which each test then weakens in one place. */
function strong(over: Partial<ClassifyInput> = {}): ClassifyInput {
  return {
    cpu: cpu(CPU.high),
    gpuHeavyMs: 0.8,
    gpuBackend: 'webgpu',
    cores: 10,
    memory: { deviceMemoryGb: 8 },
    ...over,
  };
}

describe('tier boundaries', () => {
  const t = { minimum: 10, recommended: 20, high: 30 };
  it('higher is better: each value lands in the highest tier it reaches', () => {
    expect(tierHigherIsBetter(9.99, t)).toBe('below-minimum');
    expect(tierHigherIsBetter(10, t)).toBe('minimum');
    expect(tierHigherIsBetter(19.99, t)).toBe('minimum');
    expect(tierHigherIsBetter(20, t)).toBe('recommended');
    expect(tierHigherIsBetter(30, t)).toBe('high');
    expect(tierHigherIsBetter(1e9, t)).toBe('high');
  });

  it('lower is better: the same, for milliseconds', () => {
    const ms = { minimum: 16, recommended: 4, high: 1.5 };
    expect(tierLowerIsBetter(0.1, ms)).toBe('high');
    expect(tierLowerIsBetter(1.5, ms)).toBe('high');
    expect(tierLowerIsBetter(1.51, ms)).toBe('recommended');
    expect(tierLowerIsBetter(4, ms)).toBe('recommended');
    expect(tierLowerIsBetter(16, ms)).toBe('minimum');
    expect(tierLowerIsBetter(16.01, ms)).toBe('below-minimum');
  });

  it('the shipped thresholds are ordered the right way round', () => {
    const { cpu: c, cores, gpuHeavyMs: g, gpuPresetMs: p } = THRESHOLDS;
    expect(c.minimum).toBeLessThan(c.recommended);
    expect(c.recommended).toBeLessThan(c.high);
    expect(cores.minimum).toBeLessThan(cores.recommended);
    expect(cores.recommended).toBeLessThan(cores.high);
    expect(g.high).toBeLessThan(g.recommended);
    expect(g.recommended).toBeLessThan(g.minimum);
    expect(p.ultra).toBeLessThan(p.high);
    expect(p.high).toBeLessThan(p.medium);
  });
});

describe('classify', () => {
  it('a strong machine is High with the Ultra preset and nothing holding it back', () => {
    const c = classify(strong());
    expect(c).toMatchObject({
      profile: 'high',
      preset: 'ultra',
      limitedBy: 'none',
      complete: true,
    });
  });

  it('is as good as its weakest part, and says which part that is', () => {
    expect(classify(strong({ cpu: cpu(CPU.minimum) }))).toMatchObject({
      profile: 'minimum',
      limitedBy: 'cpu',
    });
    expect(classify(strong({ gpuHeavyMs: 3 }))).toMatchObject({
      profile: 'recommended',
      limitedBy: 'gpu',
    });
    expect(classify(strong({ cores: 4 }))).toMatchObject({
      profile: 'minimum',
      limitedBy: 'cores',
    });
    expect(classify(strong({ gpuHeavyMs: 67 }))).toMatchObject({
      profile: 'below-minimum',
      limitedBy: 'gpu',
    });
  });

  it('names the first of equally weak parts', () => {
    const c = classify(strong({ cpu: cpu(CPU.minimum), cores: 4 }));
    expect(c.parts.cpu).toBe('minimum');
    expect(c.parts.cores).toBe('minimum');
    expect(c.limitedBy).toBe('cpu');
  });

  it('memory can fail a machine but never rank it', () => {
    expect(classify(strong({ memory: { deviceMemoryGb: 4 } }))).toMatchObject({
      profile: 'below-minimum',
      limitedBy: 'memory',
    });
    expect(classify(strong({ memory: { deviceMemoryGb: 8 } })).parts.memory).toBe('high');
    expect(classify(strong({ memory: { deviceMemoryGb: null } })).parts.memory).toBeNull();
  });

  it('a part that was not measured does not vote, and the verdict is marked incomplete', () => {
    const noGpu = classify(strong({ gpuHeavyMs: null, gpuBackend: '' }));
    expect(noGpu.parts.gpu).toBeNull();
    expect(noGpu.complete).toBe(false);
    expect(noGpu.profile).toBe('high');
    const noCores = classify(strong({ cores: null }));
    expect(noCores.complete).toBe(false);
    expect(classify(strong({ cores: 0 })).parts.cores).toBeNull();
  });

  it('with nothing measured the profile is below Minimum and limited by nothing', () => {
    const c = classify({
      cpu: null,
      gpuHeavyMs: null,
      gpuBackend: '',
      cores: null,
      memory: { deviceMemoryGb: null },
    });
    expect(c).toMatchObject({
      profile: 'below-minimum',
      preset: 'low',
      limitedBy: 'none',
      complete: false,
    });
  });

  it('picks the preset from the GPU, held down by the profile', () => {
    const preset = (ms: number, over: Partial<ClassifyInput> = {}) =>
      classify(strong({ gpuHeavyMs: ms, ...over })).preset;
    expect(preset(1.4)).toBe('ultra');
    expect(preset(1.6)).toBe('high');
    expect(preset(4)).toBe('high');
    expect(preset(5)).toBe('medium');
    expect(preset(8)).toBe('medium');
    expect(preset(9)).toBe('low');
    // A fast GPU on a weak CPU: the profile caps the preset.
    expect(preset(0.5, { cpu: cpu(CPU.minimum) })).toBe('medium');
    expect(preset(0.5, { cpu: cpu(CPU.belowMinimum) })).toBe('low');
    expect(preset(0.5, { cpu: cpu(CPU.recommended) })).toBe('high');
  });

  it('carries the figures the explanation needs', () => {
    const c = classify(strong());
    expect(c.facts).toEqual({
      cpuTicksPerSecond: CPU.high,
      speedup: 6,
      workers: 8,
      gpuHeavyMs: 0.8,
      gpuBackend: 'webgpu',
      cores: 10,
      memoryGb: 8,
    });
  });
});
