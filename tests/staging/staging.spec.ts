import { readFileSync } from 'node:fs';
import { expect, test } from '@playwright/test';

interface Hooks {
  stats(): { sim: { tick: number } | undefined; render: { frames: number } | undefined };
  readPixels(): Promise<{ distinctColours: number }>;
}

const url = process.env['STAGING_URL'];
const expectedSha = process.env['EXPECTED_SHA'];

interface Golden {
  checkpoints: { tick: number; combined: string }[];
}
const golden = JSON.parse(
  readFileSync(new URL('../golden/determinism.json', import.meta.url), 'utf8'),
) as Golden;
const GOLDEN_10K = golden.checkpoints.find((c) => c.tick === 10_000)?.combined ?? 'missing';

test('live staging serves this commit, cross-origin isolated, with the golden hash', async ({
  page,
  request,
}) => {
  test.skip(url === undefined || expectedSha === undefined, 'STAGING_URL / EXPECTED_SHA not set');
  const base = (url ?? '').endsWith('/') ? (url ?? '') : `${url ?? ''}/`;

  // The CDN may serve the previous deploy for a short while after the workflow reports success.
  await expect
    .poll(
      async () => {
        const res = await request.get(`${base}build-manifest.json`, {
          headers: { 'Cache-Control': 'no-cache' },
        });
        return res.ok()
          ? ((await res.json()) as { buildId: string }).buildId
          : `HTTP ${String(res.status())}`;
      },
      { timeout: 180_000, intervals: [3_000] },
    )
    .toBe(expectedSha);

  const problems: string[] = [];
  page.on('console', (m) => {
    if (m.type() === 'error') {
      problems.push(m.text());
    }
  });
  page.on('pageerror', (e) => problems.push(e.message));

  // GitHub Pages sends no COOP/COEP: the Service Worker must install, reload once and isolate.
  await page.goto(`${base}?selftest=1`);
  const cell = (id: string) => page.locator(`[data-testid="${id}"]`);
  await expect(cell('status')).toHaveText('done', { timeout: 60_000 });
  await expect(cell('isolated')).toHaveText('true');
  await expect(cell('mode')).toHaveText('full');
  await expect(cell('sw')).toHaveText('active');
  await expect(cell('variant')).toHaveText('threads');
  await expect(cell('build-id')).toHaveText(expectedSha ?? '');
  await expect(cell('hash')).toHaveText(GOLDEN_10K);
  expect(problems).toEqual([]);
});

test('the live game runs: the sim ticks, frames are drawn, and the picture is not blank', async ({
  page,
}) => {
  test.skip(url === undefined || expectedSha === undefined, 'STAGING_URL / EXPECTED_SHA not set');
  const base = (url ?? '').endsWith('/') ? (url ?? '') : `${url ?? ''}/`;
  const problems: string[] = [];
  page.on('console', (m) => {
    if (m.type() === 'error') {
      problems.push(m.text());
    }
  });
  page.on('pageerror', (e) => problems.push(e.message));

  const byId = (id: string) => page.locator(`[data-testid="${id}"]`);
  await page.goto(`${base}?debug=1`);
  // First visit on a fresh browser: the compat screen comes first.
  const screen = byId('compat-screen');
  const stage = byId('stage');
  await expect(screen.or(stage.and(page.locator('[data-state="running"]')))).toBeVisible({
    timeout: 60_000,
  });
  if (await screen.isVisible()) {
    await byId('compat-continue').click();
  }
  await expect(stage).toHaveAttribute('data-state', 'running', { timeout: 60_000 });

  // The page's test hooks (present with ?debug=); typed here, since the staging suite stands alone.
  const readings = () =>
    page.evaluate(() => {
      const hooks = (window as unknown as { __flying?: Hooks }).__flying;
      const s = hooks?.stats();
      return { tick: s?.sim?.tick ?? 0, frames: s?.render?.frames ?? 0 };
    });
  await expect.poll(async () => (await readings()).tick, { timeout: 30_000 }).toBeGreaterThan(0);
  const a = await readings();
  await page.waitForTimeout(2000);
  const b = await readings();
  // About 120 ticks and 120 frames in two seconds; the bounds only ask that both are moving.
  expect(b.tick - a.tick).toBeGreaterThan(60);
  expect(b.frames - a.frames).toBeGreaterThan(30);

  const pixels = await page.evaluate(() =>
    (window as unknown as { __flying?: Hooks }).__flying?.readPixels(),
  );
  expect(pixels?.distinctColours ?? 0).toBeGreaterThanOrEqual(8);
  expect(problems).toEqual([]);
});
