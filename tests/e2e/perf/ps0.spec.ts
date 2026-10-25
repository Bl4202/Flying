import { mkdirSync, writeFileSync } from 'node:fs';
import { expect, test } from '@playwright/test';
import { byId, HEADERS_BASE, passCompat, watchErrors } from '../support.ts';

const STRICT = process.env['PERF_STRICT'] === '1';
/** Length of the measured window; the nightly run and the reference machines ask for 120. */
const SECONDS = Number(process.env['PERF_SECONDS'] ?? '8');
/** Where to keep the report, if the caller wants it as a file (the nightly job uploads it). */
const REPORT_DIR = process.env['PERF_REPORT_DIR'];

interface Report {
  schema: string;
  scenario: string;
  seconds: number;
  verdict: 'pass' | 'fail';
  modes: { variant: string; renderer: string };
  build: { id: string; abi: number; contentHash: string };
  machine: { hardwareConcurrency: number; renderer: string };
  metrics: {
    simHz: number;
    droppedMs: number;
    renderFps: number;
    step: { count: number; avg: number; p99: number };
    renderCpuMs: { count: number };
  };
  gates: { id: string; pass: boolean }[];
}

test('PS-0 produces a valid report, and the sim holds its rate', async ({ page }) => {
  test.setTimeout((SECONDS + 90) * 1000);
  const problems = watchErrors(page);
  await passCompat(page, `${HEADERS_BASE}?perf=ps0&seconds=${String(SECONDS)}`);

  const out = byId(page, 'perf-report');
  await expect(byId(page, 'perf-status')).toHaveAttribute('data-state', 'done', {
    timeout: 90_000,
  });
  const report = JSON.parse((await out.textContent()) ?? '{}') as Report;

  expect(report.schema).toBe('perf_report/1');
  expect(report.scenario).toBe('PS-0');
  expect(report.seconds).toBe(SECONDS);
  if (REPORT_DIR !== undefined) {
    mkdirSync(REPORT_DIR, { recursive: true });
    writeFileSync(`${REPORT_DIR}/perf-ps0.json`, `${JSON.stringify(report, null, 2)}\n`);
  }
  expect(report.build.abi).toBe(3);
  expect(report.build.contentHash).toMatch(/^[0-9a-f]{16}$/);
  expect(report.modes.variant).toBe('threads');
  expect(report.machine.hardwareConcurrency).toBeGreaterThan(0);
  expect(report.gates.map((g) => g.id)).toEqual([
    'sim-rate',
    'dropped',
    'step-p99',
    'step-avg',
    'render-cpu',
  ]);
  expect(report.metrics.step.count).toBeGreaterThan(SECONDS * 40);
  expect(report.metrics.renderCpuMs.count).toBeGreaterThan(SECONDS * 4);

  if (STRICT) {
    // Reference machines: every gate must hold.
    expect(report.verdict, JSON.stringify(report.gates)).toBe('pass');
  } else {
    // Shared CI runners are too noisy for 5 % gates; they must still keep roughly the rate.
    expect(report.metrics.simHz).toBeGreaterThanOrEqual(57);
  }
  expect(problems).toEqual([]);
});

test('the report can be copied and downloaded once the run is done', async ({ page }) => {
  test.setTimeout(120_000);
  await passCompat(page, `${HEADERS_BASE}?perf=ps0&seconds=3`);
  await expect(byId(page, 'perf-copy')).toBeDisabled();
  await expect(byId(page, 'perf-status')).toHaveAttribute('data-state', 'done', {
    timeout: 60_000,
  });
  await expect(byId(page, 'perf-copy')).toBeEnabled();
  const download = page.waitForEvent('download');
  await byId(page, 'perf-download').click();
  expect((await download).suggestedFilename()).toMatch(/^perf-ps-0-.*\.json$/);
});

test('an unknown scenario is an error, not a hang', async ({ page }) => {
  await passCompat(page, `${HEADERS_BASE}?perf=ps9`);
  // Flags parsing rejects it (only ps0 exists), so the game just runs; no perf panel appears.
  await expect(byId(page, 'perf-panel')).toHaveCount(0);
});
