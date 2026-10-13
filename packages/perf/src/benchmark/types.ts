/** `benchmark_report/1`: one run of the first-run benchmark (GDD 60.8). Plain JSON. */
import type { GpuBenchResult } from '@flying/render/gpu-bench';
import type { BuildInfo, MachineInfo } from '../report.ts';
import type { MemoryHints } from './memory.ts';
import type { StorageResult } from './storage.ts';

export const BENCHMARK_SCHEMA = 'benchmark_report/1';

export type BenchmarkMode = 'quick' | 'full';
export type Tier = 'below-minimum' | 'minimum' | 'recommended' | 'high';
export type Preset = 'low' | 'medium' | 'high' | 'ultra';
/** The part of the machine that set the profile. */
export type Limit = 'cpu' | 'gpu' | 'cores' | 'memory' | 'none';

export interface CpuResult {
  /** Ticks per second of the simulation core on one worker. */
  singleTicksPerSecond: number;
  workers: number;
  /** Ticks per second summed over `workers` workers running at once. */
  multiTicksPerSecond: number;
  /** `multi / single`: close to `workers` means the cores scale. */
  speedup: number;
}

export interface Classification {
  profile: Tier;
  preset: Preset;
  /** Each part's own tier, or null if it could not be measured. */
  parts: { cpu: Tier | null; gpu: Tier | null; cores: Tier | null; memory: Tier | null };
  limitedBy: Limit;
  /** The figures the verdict rests on, for the explanation line. */
  facts: {
    cpuTicksPerSecond: number | null;
    speedup: number | null;
    workers: number | null;
    gpuHeavyMs: number | null;
    gpuBackend: string;
    cores: number | null;
    memoryGb: number | null;
  };
  /** False when a part could not be measured, so the verdict may be too generous. */
  complete: boolean;
}

export interface BenchmarkReport {
  schema: typeof BENCHMARK_SCHEMA;
  mode: BenchmarkMode;
  startedAt: string;
  /** Wall time the whole benchmark took. */
  durationS: number;
  build: BuildInfo;
  machine: MachineInfo;
  cpu: CpuResult | null;
  gpu: GpuBenchResult | null;
  memory: MemoryHints;
  storage: StorageResult;
  classification: Classification;
  /** Parts that were skipped or failed, and why. */
  notes: string[];
}

export function isBenchmarkReport(v: unknown): v is BenchmarkReport {
  if (typeof v !== 'object' || v === null) return false;
  const r = v as Record<string, unknown>;
  const c = r['classification'] as Record<string, unknown> | undefined;
  return (
    r['schema'] === BENCHMARK_SCHEMA &&
    (r['mode'] === 'quick' || r['mode'] === 'full') &&
    typeof r['startedAt'] === 'string' &&
    typeof r['durationS'] === 'number' &&
    typeof r['machine'] === 'object' &&
    typeof r['memory'] === 'object' &&
    typeof r['storage'] === 'object' &&
    Array.isArray(r['notes']) &&
    c !== undefined &&
    typeof c['profile'] === 'string' &&
    typeof c['preset'] === 'string'
  );
}
