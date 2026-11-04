/** Helpers for tests. Not production code. */

/**
 * Bytes of heap a call makes on average, in the steady state: the median heap growth over a chunk
 * of calls, less the same for an empty call, per call. It warms up, gives the compiler time,
 * measures, and repeats; it returns the smallest round, which is the steady state if there was one
 * and the truth if there was none. (The same method as the input pump's garbage test.)
 */
export async function bytesPerCall(
  fn: (i: number) => void,
  options: { rounds?: number; chunks?: number; chunkSize?: number; warmup?: number } = {},
): Promise<number> {
  const { rounds = 6, chunks = 41, chunkSize = 2000, warmup = 100_000 } = options;
  const v8 = await import('node:v8');
  const vm = await import('node:vm');
  v8.setFlagsFromString('--expose_gc');
  const gc = vm.runInNewContext('gc') as () => void;
  const median = (xs: number[]): number => [...xs].sort((a, b) => a - b)[xs.length >> 1] ?? 0;
  const chunk = (body: (i: number) => void): number => {
    const before = process.memoryUsage().heapUsed;
    for (let i = 0; i < chunkSize; i++) body(i);
    return process.memoryUsage().heapUsed - before;
  };
  const noop = (): void => undefined;
  let best = Number.POSITIVE_INFINITY;
  for (let round = 0; round < rounds; round++) {
    for (let i = 0; i < warmup; i++) fn(i);
    await new Promise((resolve) => setTimeout(resolve, 30)); // let the compiler finish
    gc();
    const overhead = median(Array.from({ length: chunks }, () => chunk(noop)));
    const deltas = Array.from({ length: chunks }, () => chunk(fn));
    best = Math.min(best, Math.max(0, median(deltas) - overhead) / chunkSize);
    if (best < 0.5) break;
  }
  return best;
}
