/**
 * The first-run benchmark (GDD 60.8, prototype): CPU speed on one thread and across cores, GPU
 * frame time at three loads, memory hints and storage throughput, mapped to a hardware profile
 * and preset. Every part is optional: one that fails or is unavailable is noted and left out of
 * the verdict instead of stopping the run, so a browser without OPFS still gets a result.
 */
import { measureGpu, type GpuBenchResult } from '@flying/render/gpu-bench';
import type { Renderer } from '@flying/render/renderer';
import type { Variant } from '@flying/platform-web/wasm-loader';
import type { BuildInfo, MachineInfo } from '../report.ts';
import { classify, THRESHOLDS, type Thresholds } from './classify.ts';
import { runKernel, type KernelWorkerLike } from './kernel.ts';
import type { MemoryHints } from './memory.ts';
import type { StorageResult } from './storage.ts';
import {
  BENCHMARK_SCHEMA,
  type BenchmarkMode,
  type BenchmarkReport,
  type CpuResult,
} from './types.ts';

/** How long each part takes in each mode. About 25 s for `full`, a third of that for `quick`. */
export const BENCHMARK_PLAN = {
  quick: { kernelMs: 1000, gpuBudgetMs: 400, storageMb: 8 },
  full: { kernelMs: 4000, gpuBudgetMs: 2500, storageMb: 64 },
} as const;

/** The GPU load levels: copies of the terrain patch at 1920 x 1080. */
export const GPU_LEVELS = [1, 16, 64] as const;
export const GPU_WIDTH = 1920;
export const GPU_HEIGHT = 1080;

export type BenchmarkStep = 'cpu-single' | 'cpu-multi' | 'gpu' | 'memory' | 'storage' | 'done';

export interface BenchmarkDeps {
  mode: BenchmarkMode;
  build: BuildInfo;
  machine: MachineInfo;
  /** Logical processors, for choosing how many workers to scale across. */
  cores: number;
  /** The page is cross-origin isolated, so workers can start on a shared flag. */
  isolated: boolean;
  nowMs(): number;
  now(): Date;
  kernel: {
    variant: Variant;
    wasmUrl: string;
    threadsInitialPages: number;
    /** The content pack the kernel loads, so it times a tick with the aircraft flying. */
    contentUrl: string;
    spawn(): KernelWorkerLike;
  };
  /** Starts a renderer on a throwaway surface of 1920 x 1080; rejects if none can start. */
  makeRenderer(): Promise<Renderer>;
  memory(): Promise<MemoryHints>;
  storage(megabytes: number): Promise<StorageResult>;
  thresholds?: Thresholds;
  onProgress?: (step: BenchmarkStep, detail: string) => void;
}

const MAX_WORKERS = 8;

function message(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}

export async function runBenchmark(deps: BenchmarkDeps): Promise<BenchmarkReport> {
  const plan = BENCHMARK_PLAN[deps.mode];
  const started = deps.nowMs();
  const startedAt = deps.now().toISOString();
  const notes: string[] = [];
  const progress = (step: BenchmarkStep, detail = ''): void => {
    deps.onProgress?.(step, detail);
  };

  // CPU, one worker and then several.
  let cpu: CpuResult | null = null;
  try {
    progress('cpu-single');
    const k = deps.kernel;
    const base = {
      ms: plan.kernelMs,
      variant: k.variant,
      wasmUrl: k.wasmUrl,
      threadsInitialPages: k.threadsInitialPages,
      contentUrl: k.contentUrl,
      spawn: () => k.spawn(),
      sharedStart: deps.isolated,
    };
    const single = await runKernel({ ...base, workers: 1 });
    progress('cpu-multi');
    const workers = Math.min(MAX_WORKERS, Math.max(1, deps.cores));
    const multi = workers === 1 ? single : await runKernel({ ...base, workers });
    cpu = {
      singleTicksPerSecond: single.ticksPerSecond,
      workers,
      multiTicksPerSecond: multi.ticksPerSecond,
      speedup: multi.ticksPerSecond / Math.max(single.ticksPerSecond, 1e-9),
    };
  } catch (e) {
    notes.push(`CPU kernel skipped: ${message(e)}`);
  }

  // GPU, on a renderer of its own.
  let gpu: GpuBenchResult | null = null;
  try {
    progress('gpu');
    const renderer = await deps.makeRenderer();
    try {
      gpu = await measureGpu({
        renderer,
        width: GPU_WIDTH,
        height: GPU_HEIGHT,
        levels: GPU_LEVELS,
        budgetMs: plan.gpuBudgetMs,
        nowMs: () => deps.nowMs(),
        onLevel: (l) => {
          progress('gpu', `${String(l.patches)} patches: ${l.msPerFrame.toFixed(2)} ms`);
        },
      });
    } finally {
      renderer.dispose();
    }
  } catch (e) {
    notes.push(`GPU measurement skipped: ${message(e)}`);
  }

  progress('memory');
  let memory: MemoryHints = {
    deviceMemoryGb: null,
    jsHeapLimitMb: null,
    jsHeapUsedMb: null,
    measuredMb: null,
  };
  try {
    memory = await deps.memory();
  } catch (e) {
    notes.push(`memory hints skipped: ${message(e)}`);
  }

  progress('storage');
  let storage: StorageResult = {
    available: false,
    writeMBps: null,
    readMBps: null,
    note: 'not measured',
  };
  try {
    storage = await deps.storage(plan.storageMb);
    if (storage.note !== '') notes.push(`storage: ${storage.note}`);
  } catch (e) {
    notes.push(`storage measurement skipped: ${message(e)}`);
  }

  const heavy = gpu?.levels.at(-1);
  const classification = classify(
    {
      cpu,
      gpuHeavyMs: heavy?.msPerFrame ?? null,
      gpuBackend: gpu?.backend ?? '',
      cores: deps.cores > 0 ? deps.cores : null,
      memory,
    },
    deps.thresholds ?? THRESHOLDS,
  );
  progress('done');
  return {
    schema: BENCHMARK_SCHEMA,
    mode: deps.mode,
    startedAt,
    durationS: (deps.nowMs() - started) / 1000,
    build: deps.build,
    machine: deps.machine,
    cpu,
    gpu,
    memory,
    storage,
    classification,
    notes,
  };
}
