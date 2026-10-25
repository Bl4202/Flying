/**
 * Control latency in the browser (AC-58.3, ADR-33): from a key event to the end of the sim tick
 * that acted on it, measured by the sim on frames that carry a change. This is the CI check with
 * the lenient bound (two ticks); the strict 25 ms gate is judged on the reference machine
 * (`LATENCY_STRICT=1`, headed Chrome, the staging build).
 */
import { expect, test } from '@playwright/test';
import {
  controlLatencyRun,
  flightStatus,
  HEADERS_BASE,
  type LatencyQuantiles,
  openGame,
} from './support.ts';

const BOUND_MS = process.env['LATENCY_STRICT'] === '1' ? 25 : 33.3;
const MIN_SAMPLES = process.env['LATENCY_STRICT'] === '1' ? 500 : 200;

test(`at least ${String(MIN_SAMPLES)} control changes are measured, with a 95th percentile within ${String(BOUND_MS)} ms`, async ({
  page,
}) => {
  test.setTimeout(180_000);
  await openGame(page, `${HEADERS_BASE}?debug=1&spawn=air`);
  await flightStatus(page);
  // Warm up, then count from here.
  await page.waitForTimeout(1000);
  await page.evaluate(() => {
    window.__flying?.resetRun();
  });
  const phase = (lo: number, hi: number): number => lo + Math.random() * (hi - lo);
  // Key presses and releases at random phases of the 60 Hz tick: each one is a change.
  const rounds = Math.ceil(MIN_SAMPLES / 2) + 20;
  for (let i = 0; i < rounds; i++) {
    await page.keyboard.down('KeyE');
    await page.waitForTimeout(phase(8, 40));
    await page.keyboard.up('KeyE');
    await page.waitForTimeout(phase(8, 40));
  }
  let latest: LatencyQuantiles | undefined;
  await expect
    .poll(
      async () => {
        latest = await controlLatencyRun(page);
        return latest?.count ?? 0;
      },
      { timeout: 10_000 },
    )
    .toBeGreaterThanOrEqual(MIN_SAMPLES);
  if (latest === undefined) throw new Error('no latency report');
  console.log(
    `control latency over ${String(latest.count)} changes: avg ${latest.avg.toFixed(2)} p95 ${latest.p95.toFixed(2)} p99 ${latest.p99.toFixed(2)} max ${latest.max.toFixed(2)} ms`,
  );
  expect(latest.p95).toBeLessThanOrEqual(BOUND_MS);
});
