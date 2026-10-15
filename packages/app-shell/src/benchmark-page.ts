/**
 * `?bench=quick|full`: the first-run benchmark on a page of its own, so the game is not running
 * while it measures. The result is stored (the overlay shows it afterwards) and shown with Copy
 * and Download.
 */
import {
  browserMachineEnv,
  BENCHMARK_PLAN,
  describeMachine,
  readMemoryHints,
  measureOpfs,
  runBenchmark,
  saveBenchmark,
  type BenchmarkMode,
  type BenchmarkReport,
  type KernelWorkerLike,
  type MemoryEnv,
} from '@flying/perf';
import kernelWorkerUrl from '@flying/perf/kernel.worker?worker&url';
import { createRenderer, type CanvasLike } from '@flying/render';
import { SHADERS } from '@flying/render/shader-sources';
import { scriptUrl, type DeepFeatures, type Flags, type Kv } from '@flying/platform-web';
import { EXPECTED_ABI_VERSION } from '@flying/sim-bridge/sim-handle';
import { mountBenchmarkPanel } from '@flying/ui';
import { coreContentUrl, wasmUrl } from './selftest.ts';

export interface BenchmarkPageArgs {
  root: HTMLElement;
  mode: BenchmarkMode;
  flags: Flags;
  features: DeepFeatures;
  kv: Kv;
  /** Leaves the benchmark for the game. */
  onBack: () => void;
}

function newSurface(): CanvasLike {
  return typeof OffscreenCanvas === 'undefined'
    ? document.createElement('canvas')
    : new OffscreenCanvas(1920, 1080);
}

export async function runBenchmarkPage(
  args: BenchmarkPageArgs,
): Promise<BenchmarkReport | undefined> {
  const { flags, features } = args;
  const plan = BENCHMARK_PLAN[args.mode];
  // Rough total: both kernel runs, three GPU levels with warm-up, and storage.
  const seconds = Math.round((2 * plan.kernelMs + 3 * (plan.gpuBudgetMs + 600)) / 1000 + 3);
  const panel = mountBenchmarkPanel(args.root, {
    seconds,
    onAgain: () => {
      location.reload();
    },
    onBack: args.onBack,
  });

  try {
    const report = await runBenchmark({
      mode: args.mode,
      build: { id: __BUILD_ID__, abi: EXPECTED_ABI_VERSION, contentHash: '' },
      machine: describeMachine(browserMachineEnv(), features.adapterInfo, ''),
      cores: navigator.hardwareConcurrency,
      isolated: features.crossOriginIsolated && features.sharedArrayBuffer,
      nowMs: () => performance.now(),
      now: () => new Date(),
      kernel: {
        variant: 'single',
        wasmUrl: wasmUrl('single'),
        threadsInitialPages: 0,
        contentUrl: coreContentUrl,
        spawn: () =>
          new Worker(scriptUrl(kernelWorkerUrl), { type: 'module' }) as unknown as KernelWorkerLike,
      },
      makeRenderer: () =>
        createRenderer(
          flags.renderer,
          { current: newSurface, fresh: () => Promise.resolve(newSurface()) },
          SHADERS,
        ),
      memory: () => readMemoryHints({ navigator, performance } as MemoryEnv),
      storage: (megabytes) =>
        measureOpfs(
          features.opfs ? () => navigator.storage.getDirectory() : undefined,
          megabytes,
          () => performance.now(),
        ),
      onProgress: (step, detail) => {
        panel.step(step, detail);
      },
    });
    const gpu = report.gpu;
    report.machine.renderer = gpu === null ? '' : `${gpu.backend} ${gpu.description}`;
    await saveBenchmark(args.kv, report);
    panel.done(report);
    return report;
  } catch (e) {
    panel.fail(e instanceof Error ? e.message : String(e));
    return undefined;
  }
}
