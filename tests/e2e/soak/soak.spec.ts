/**
 * The soak (ADR-45 section 4.7, L1 to L7): the bot playtester flies its duel's sortie against a
 * Regular, from the runway to the stop, again and again through the debrief's Fly again, while the
 * harness samples memory, the DOM, the GPU and audio resources and the workers.
 *
 * Environment: `SOAK_SECONDS` (the measured window, default 1 800), `SOAK_WARMUP_SECONDS` (default
 * 120), `SOAK_GATES=quick` (L3, L4, L6 and L7 only: CI's harness check), `SOAK_BASE_URL` (default
 * the local headers preview; staging in WP10), `SOAK_REPORT_DIR` (default `test-results/soak`),
 * `SOAK_FAULT` (`leak-js` or `leak-gpu`: a seeded leak the verdict must catch), `CI`.
 */
import { mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { expect, test, type CDPSession, type Page } from '@playwright/test';
import {
  soakVerdict,
  type SoakReport,
  type SoakSample,
  type SortieStart,
} from '../../../packages/perf/src/soak.ts';
import { HEADERS_BASE, openGame, watchErrors } from '../support.ts';

const SECONDS = Number(process.env['SOAK_SECONDS'] ?? '1800');
const WARMUP_SECONDS = Number(process.env['SOAK_WARMUP_SECONDS'] ?? '120');
const GATES = process.env['SOAK_GATES'] === 'quick' ? 'quick' : 'all';
const BASE = process.env['SOAK_BASE_URL'] ?? HEADERS_BASE;
const REPORT_DIR = process.env['SOAK_REPORT_DIR'] ?? 'test-results/soak';
const FAULT = process.env['SOAK_FAULT'];
const SAMPLE_EVERY_S = 60;
/** A sample waits this long after Fly again: the sim's rate report must cover running time only. */
const SETTLE_AFTER_FLY_MS = 2000;
/** `CORE_STATE.RUNNING` (`sim-bridge`). */
const CORE_RUNNING = 2;

const PAGE = `${BASE}?debug=1&fly=1&scenario=duel&skill=2&script=sortie&res=640x360&quality=low${
  FAULT === undefined ? '' : `&fault=${FAULT}`
}`;

/** One sample of the page's hook, after a garbage collection where the page can ask for one. */
function sample(page: Page) {
  return page.evaluate(async () => {
    (globalThis as { gc?: () => void }).gc?.();
    return window.__flying?.memory();
  });
}

/**
 * Whether the simulation is running: no debrief holds it. L7 judges the sim's rate while it runs;
 * the debrief's hold between sorties is deliberate (its rate reads 0) and is not a silence (L4
 * counts those).
 */
function simRunning(page: Page): Promise<boolean> {
  return page.evaluate((RUNNING) => {
    const f = window.__flying;
    return f !== undefined && f.debrief() === null && f.stats().sim?.coreState === RUNNING;
  }, CORE_RUNNING);
}

/** The DOM's counters after a full collection (CDP), and the GPU resources, at a sortie start. */
async function sortieStart(page: Page, cdp: CDPSession, tS: number): Promise<SortieStart> {
  await cdp.send('HeapProfiler.collectGarbage');
  const dom = (await cdp.send('Memory.getDOMCounters')) as {
    nodes: number;
    jsEventListeners: number;
  };
  const m = await sample(page);
  return {
    tS,
    nodes: dom.nodes,
    listeners: dom.jsEventListeners,
    liveBuffers: m?.liveBuffers ?? 0,
    liveTextures: m?.liveTextures ?? 0,
  };
}

test('the soak: sortie after sortie with no leak (L1 to L7)', async ({ page }) => {
  const consoleErrors = watchErrors(page);
  const cdp = await page.context().newCDPSession(page);
  await cdp.send('HeapProfiler.enable');
  await openGame(page, PAGE);

  const t0 = Date.now();
  const windowStart = t0 + WARMUP_SECONDS * 1000;
  const end = windowStart + SECONDS * 1000;
  const since = (): number => (Date.now() - windowStart) / 1000;
  const starts: SortieStart[] = [await sortieStart(page, cdp, since())];
  const samples: SoakSample[] = [];
  let nextSample = windowStart;
  let flewAgainAt = 0;
  /**
   * A sample taken while the sim runs, or `undefined` if it is held now or was held by the time
   * the sample came back (the browser's memory measurement can take a second or more, and a
   * sortie can end meanwhile): the caller tries again a second later.
   */
  const runningSample = async () => {
    if (Date.now() - flewAgainAt < SETTLE_AFTER_FLY_MS || !(await simRunning(page))) return;
    const m = await sample(page);
    return m !== undefined && (await simRunning(page)) ? m : undefined;
  };

  while (Date.now() < end) {
    // The debrief opens after each stop: fly again, and that is a sortie start.
    if ((await page.evaluate(() => window.__flying?.debrief() ?? null)) !== null) {
      await page.locator('[data-testid="debrief-fly-again"]').click();
      await expect
        .poll(() => page.evaluate(() => window.__flying?.debrief() ?? null), { timeout: 10_000 })
        .toBeNull();
      flewAgainAt = Date.now();
      starts.push(await sortieStart(page, cdp, since()));
    }
    if (Date.now() >= nextSample) {
      const m = await runningSample();
      if (m !== undefined) {
        samples.push({ tS: since(), ...m });
        nextSample += SAMPLE_EVERY_S * 1000;
      }
    }
    await page.waitForTimeout(1000);
  }
  // The window's last sample, at its end (a few tries, in case a debrief holds the sim).
  for (let i = 0; i < 15; i++) {
    const last = await runningSample();
    if (last !== undefined) {
      samples.push({ tS: since(), ...last });
      break;
    }
    if ((await page.evaluate(() => window.__flying?.debrief() ?? null)) !== null) {
      await page.locator('[data-testid="debrief-fly-again"]').click();
      flewAgainAt = Date.now();
    }
    await page.waitForTimeout(1000);
  }

  const build = await page.evaluate(
    () => document.querySelector('[data-testid="ov-build"]')?.getAttribute('data-value') ?? '',
  );
  const report: SoakReport = {
    schema: 'soak_report/1',
    build,
    url: PAGE,
    warmupS: WARMUP_SECONDS,
    seconds: SECONDS,
    samples,
    // Those before the window (the first take-off, in the warm-up) still count as sortie starts.
    sortieStarts: starts,
    consoleErrors,
  };
  const verdict = soakVerdict(report, { gates: GATES, ci: process.env['CI'] !== undefined });
  mkdirSync(REPORT_DIR, { recursive: true });
  writeFileSync(
    join(REPORT_DIR, 'soak-report.json'),
    `${JSON.stringify({ ...report, gates: GATES, fault: FAULT ?? null, verdict }, null, 2)}\n`,
  );
  for (const k of ['L1', 'L2', 'L3', 'L4', 'L5', 'L6', 'L7'] as const) {
    const g = verdict[k];
    test.info().annotations.push({
      type: k,
      description: g === null ? 'not judged' : `${g.pass ? 'pass' : 'FAIL'}: ${g.detail}`,
    });
  }
  const failed = (['L1', 'L2', 'L3', 'L4', 'L5', 'L6', 'L7'] as const).filter(
    (k) => verdict[k]?.pass === false,
  );
  expect(failed, failed.map((k) => `${k}: ${verdict[k]?.detail ?? ''}`).join('\n')).toEqual([]);
});
