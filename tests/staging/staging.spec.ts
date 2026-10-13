import { readFileSync } from 'node:fs';
import { expect, test } from '@playwright/test';

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
