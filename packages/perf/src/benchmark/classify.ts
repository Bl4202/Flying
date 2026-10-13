/**
 * Turns benchmark measurements into a hardware profile and a quality preset. The profile is the
 * lowest tier any one part reaches, because a machine is as good as its weakest part for a game
 * that needs all of them; the preset comes from what the GPU can draw, held down by the profile.
 *
 * Pure: the thresholds come in as data (`thresholds.json`), so this is tested without a browser
 * and tuned without touching code.
 */
import thresholdsJson from './thresholds.json';
import type { Classification, CpuResult, Limit, Preset, Tier } from './types.ts';
import type { MemoryHints } from './memory.ts';

export type Thresholds = typeof thresholdsJson;
export const THRESHOLDS: Thresholds = thresholdsJson;

const TIER_ORDER: readonly Tier[] = ['below-minimum', 'minimum', 'recommended', 'high'];
const PRESET_ORDER: readonly Preset[] = ['low', 'medium', 'high', 'ultra'];

/** Which of `minimum`, `recommended` and `high` a value reaches when higher is better. */
export function tierHigherIsBetter(
  value: number,
  t: { minimum: number; recommended: number; high: number },
): Tier {
  if (value >= t.high) return 'high';
  if (value >= t.recommended) return 'recommended';
  if (value >= t.minimum) return 'minimum';
  return 'below-minimum';
}

/** The same when lower is better (milliseconds). */
export function tierLowerIsBetter(
  value: number,
  t: { minimum: number; recommended: number; high: number },
): Tier {
  if (value <= t.high) return 'high';
  if (value <= t.recommended) return 'recommended';
  if (value <= t.minimum) return 'minimum';
  return 'below-minimum';
}

function lowest(...tiers: (Tier | null)[]): Tier | null {
  let best: Tier | null = null;
  for (const t of tiers) {
    if (t !== null && (best === null || TIER_ORDER.indexOf(t) < TIER_ORDER.indexOf(best))) {
      best = t;
    }
  }
  return best;
}

export interface ClassifyInput {
  cpu: CpuResult | null;
  /** Milliseconds per frame at the heaviest GPU level, or null if the GPU was not measured. */
  gpuHeavyMs: number | null;
  gpuBackend: string;
  /** Logical processors; 0 or null when the browser does not say. */
  cores: number | null;
  memory: Pick<MemoryHints, 'deviceMemoryGb'>;
}

const PROFILE_PRESET_CAP: Record<Tier, Preset> = {
  'below-minimum': 'low',
  minimum: 'medium',
  recommended: 'high',
  high: 'ultra',
};

function gpuPreset(ms: number, t: Thresholds['gpuPresetMs']): Preset {
  if (ms <= t.ultra) return 'ultra';
  if (ms <= t.high) return 'high';
  if (ms <= t.medium) return 'medium';
  return 'low';
}

export function classify(
  input: ClassifyInput,
  thresholds: Thresholds = THRESHOLDS,
): Classification {
  const cpu =
    input.cpu === null ? null : tierHigherIsBetter(input.cpu.singleTicksPerSecond, thresholds.cpu);
  const gpu =
    input.gpuHeavyMs === null ? null : tierLowerIsBetter(input.gpuHeavyMs, thresholds.gpuHeavyMs);
  const cores =
    input.cores === null || input.cores <= 0
      ? null
      : tierHigherIsBetter(input.cores, thresholds.cores);
  // Chrome stops deviceMemory at 8 GB, so memory can fail a machine but never rank it.
  const memory: Tier | null =
    input.memory.deviceMemoryGb === null
      ? null
      : input.memory.deviceMemoryGb >= thresholds.memoryGb.minimum
        ? 'high'
        : 'below-minimum';

  const parts = { cpu, gpu, cores, memory };
  const profile = lowest(cpu, gpu, cores, memory) ?? 'below-minimum';
  const order: Limit[] = ['cpu', 'gpu', 'cores', 'memory'];
  const measured = order.filter((k) => parts[k as keyof typeof parts] !== null);
  // A machine at the top tier is not held back by anything.
  const limitedBy: Limit =
    profile === 'high'
      ? 'none'
      : (measured.find((k) => parts[k as keyof typeof parts] === profile) ?? 'none');

  const fromGpu =
    input.gpuHeavyMs === null ? 'ultra' : gpuPreset(input.gpuHeavyMs, thresholds.gpuPresetMs);
  const cap = PROFILE_PRESET_CAP[profile];
  const preset =
    PRESET_ORDER[Math.min(PRESET_ORDER.indexOf(fromGpu), PRESET_ORDER.indexOf(cap))] ?? 'low';

  return {
    profile,
    preset,
    parts,
    limitedBy,
    facts: {
      cpuTicksPerSecond: input.cpu?.singleTicksPerSecond ?? null,
      speedup: input.cpu?.speedup ?? null,
      workers: input.cpu?.workers ?? null,
      gpuHeavyMs: input.gpuHeavyMs,
      gpuBackend: input.gpuBackend,
      cores: input.cores,
      memoryGb: input.memory.deviceMemoryGb,
    },
    complete: cpu !== null && gpu !== null && cores !== null,
  };
}
