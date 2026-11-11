import { mkdirSync, writeFileSync } from 'node:fs';
import { expect, test } from '@playwright/test';
import { byId, CHEAP, HEADERS_BASE, passCompat, watchErrors } from '../support.ts';

const STRICT = process.env['PERF_STRICT'] === '1';
/** Length of the measured window; the reference machines ask for 120. */
const SECONDS = Number(process.env['PERF_SECONDS'] ?? '8');
/** Where to keep the report, if the caller wants it as a file. */
const REPORT_DIR = process.env['PERF_REPORT_DIR'];
/** The camera of the run (`PERF_CAMERA=chase` for the chase view). */
const CAMERA = process.env['PERF_CAMERA'] === 'chase' ? '&camera=chase' : '';

interface Report {
  schema: string;
  scenario: string;
  seconds: number;
  warmupS: number;
  verdict: 'pass' | 'fail';
  build: { abi: number };
  metrics: {
    simHz: number;
    renderFps: number;
    frames: number;
    longTasks: number | null;
    deviceLosses: number;
    frameMs: { count: number; avg: number; p99: number };
    renderCpuMs: { count: number; p99: number };
  };
  gates: { id: string; pass: boolean; informational?: boolean }[];
}

test('the dogfight run produces a valid report with the arena gates, and the sim holds its rate', async ({
  page,
}) => {
  test.setTimeout((SECONDS + 120) * 1000);
  const problems = watchErrors(page);
  // The bot's dogfight from the merge against a Regular (ADR-45): the guns, the missiles, their
  // sounds and the second aircraft on top of the arena run's load.
  // The reference machines measure the scenario as it is (1920 x 1080 at High); a CI runner has only
  // a software rasteriser, so it measures the harness with a small buffer at the lowest preset.
  const view = STRICT ? '' : `&${CHEAP}`;
  await passCompat(page, `${HEADERS_BASE}?perf=combat&seconds=${String(SECONDS)}${CAMERA}${view}`);
  await expect(byId(page, 'perf-status')).toHaveAttribute('data-state', 'done', {
    timeout: (SECONDS + 90) * 1000,
  });
  const report = JSON.parse((await byId(page, 'perf-report').textContent()) ?? '{}') as Report;

  expect(report.schema).toBe('perf_report/1');
  expect(report.scenario).toBe('COMBAT');
  expect(report.seconds).toBe(SECONDS);
  expect(report.warmupS).toBe(5);
  if (REPORT_DIR !== undefined) {
    mkdirSync(REPORT_DIR, { recursive: true });
    writeFileSync(`${REPORT_DIR}/perf-combat.json`, `${JSON.stringify(report, null, 2)}\n`);
  }
  expect(report.build.abi).toBe(5);
  expect(report.gates.map((g) => g.id)).toEqual([
    'sim-rate',
    'dropped',
    'step-p99',
    'step-avg',
    'render-cpu',
    'render-cpu-p99',
    'frame-avg',
    'frame-p99',
    'device-loss',
    'long-tasks',
    'fps-minimum',
  ]);
  expect(report.metrics.frames).toBeGreaterThan(SECONDS * 2);
  expect(report.metrics.frameMs.count).toBeGreaterThan(SECONDS * 2);
  expect(report.metrics.deviceLosses).toBe(0);
  // Chrome counts long tasks; where a browser cannot, the report says so with null.
  expect(['number', 'object']).toContain(typeof report.metrics.longTasks);

  if (STRICT) {
    // Reference machines: every gate that is one must hold (P1 and P2).
    expect(report.verdict, JSON.stringify(report.gates)).toBe('pass');
  } else {
    // Shared CI runners are too noisy for the frame gates; the sim must still keep roughly its rate.
    expect(report.metrics.simHz).toBeGreaterThanOrEqual(57);
  }
  expect(problems).toEqual([]);
});
