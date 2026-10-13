import { expect, test } from '@playwright/test';
import { byId, HEADERS_BASE, openGame, passCompat, watchErrors } from './support.ts';

interface Bench {
  schema: string;
  mode: string;
  cpu: { singleTicksPerSecond: number; workers: number; speedup: number } | null;
  gpu: { levels: { patches: number; msPerFrame: number }[] } | null;
  storage: { available: boolean };
  classification: { profile: string; preset: string; parts: Record<string, string | null> };
  notes: string[];
}

test('the quick benchmark yields a profile with an explanation, and the overlay keeps it', async ({
  page,
}) => {
  test.setTimeout(120_000);
  const problems = watchErrors(page);
  await passCompat(page, `${HEADERS_BASE}?bench=quick`);
  await expect(byId(page, 'bench-status')).toHaveAttribute('data-state', 'done', {
    timeout: 90_000,
  });

  await expect(byId(page, 'bench-profile')).toHaveAttribute(
    'data-profile',
    /^(below-minimum|minimum|recommended|high)$/,
  );
  await expect(byId(page, 'bench-preset')).toHaveAttribute(
    'data-preset',
    /^(low|medium|high|ultra)$/,
  );
  // The explanation names the figures, in words.
  await expect(byId(page, 'bench-explanation')).toContainText('preset');
  await expect(byId(page, 'bench-explanation')).toContainText(/ms|thousand/);

  const report = JSON.parse((await byId(page, 'bench-report').textContent()) ?? '{}') as Bench;
  expect(report.schema).toBe('benchmark_report/1');
  expect(report.mode).toBe('quick');
  expect(report.cpu?.singleTicksPerSecond ?? 0).toBeGreaterThan(10_000);
  expect(report.cpu?.workers ?? 0).toBeGreaterThanOrEqual(1);
  expect(report.gpu?.levels.map((l) => l.patches)).toEqual([1, 16, 64]);
  for (const level of report.gpu?.levels ?? []) {
    expect(level.msPerFrame).toBeGreaterThan(0);
  }
  // Heavier loads cost at least as much.
  const ms = report.gpu?.levels.map((l) => l.msPerFrame) ?? [];
  expect(ms[2]).toBeGreaterThanOrEqual(ms[0] ?? 0);
  expect(report.storage.available).toBe(true);
  expect(report.notes).toEqual([]);

  await expect(byId(page, 'bench-copy')).toBeEnabled();
  await expect(byId(page, 'bench-again')).toBeEnabled();
  expect(problems).toEqual([]);

  // The result survives: after a reload into the game, the overlay still shows it.
  await openGame(page, `${HEADERS_BASE}?debug=1`);
  await expect(byId(page, 'ov-bench')).toHaveAttribute(
    'data-value',
    `${report.classification.profile} / ${report.classification.preset}`,
  );
});
