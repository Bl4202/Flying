/** The benchmark result, kept in the key-value store so it survives a reload. */
import type { Kv } from '@flying/platform-web/kv';
import { isBenchmarkReport, type BenchmarkReport } from './types.ts';

export const BENCHMARK_KEY = 'benchmark.report';

export async function saveBenchmark(kv: Kv, report: BenchmarkReport): Promise<void> {
  try {
    await kv.set(BENCHMARK_KEY, report);
  } catch {
    // losing the stored copy only means running it again
  }
}

/** The stored report, or undefined if there is none or it is not a report this version reads. */
export async function loadBenchmark(kv: Kv): Promise<BenchmarkReport | undefined> {
  try {
    const value = await kv.get<unknown>(BENCHMARK_KEY);
    return isBenchmarkReport(value) ? value : undefined;
  } catch {
    return undefined;
  }
}
