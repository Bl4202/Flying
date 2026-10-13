/**
 * Judging a self-test result against the committed golden. Kept apart from `selftest.ts` because
 * that file imports bundler assets, and this one is plain enough to unit-test.
 */
import golden from '../../../tests/golden/determinism.json';
import type { SelfTestResult } from './selftest.worker.ts';

/** Default master seed, equal to `sim_core::DEFAULT_SEED`. */
export const SEED = '1f1e5eed00000001';
export const SELFTEST_TICKS = 10_000;

export interface GoldenCheckpoint {
  combined: string;
  content: string;
}

/** The committed expectation for the self-test's tick, if the golden has one. */
export function goldenCheckpoint(): GoldenCheckpoint | undefined {
  const found = golden.checkpoints.find((c) => c.tick === SELFTEST_TICKS);
  return found === undefined ? undefined : { combined: found.combined, content: found.content };
}

/** Whether a self-test result matches the golden, with a line to show for it. */
export function judgeSelfTest(
  result: SelfTestResult,
  expected: GoldenCheckpoint | undefined,
): { ok: boolean; text: string } {
  if (!result.ok) {
    return { ok: false, text: result.error };
  }
  if (expected === undefined) {
    return { ok: false, text: `the golden has no checkpoint at tick ${String(SELFTEST_TICKS)}` };
  }
  const lines = [
    `${result.variant} build, ABI ${String(result.abi)}, ${result.tick} ticks in ${result.ms.toFixed(0)} ms`,
    `state   ${result.hash}${result.hash === expected.combined ? '' : `  expected ${expected.combined}`}`,
    `content ${result.content}${result.content === expected.content ? '' : `  expected ${expected.content}`}`,
  ];
  return {
    ok: result.hash === expected.combined && result.content === expected.content,
    text: lines.join('\n'),
  };
}
