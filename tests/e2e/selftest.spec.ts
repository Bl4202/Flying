import { readFileSync } from 'node:fs';
import { expect, test, type Page } from '@playwright/test';
import { ABI } from '../../packages/sim-bridge/src/layout.gen.ts';

/** `?selftest=1` shows the status table: what this load got and the hash after 10 000 ticks. */
const HEADERS_URL = 'http://127.0.0.1:4173/Flying/?selftest=1';
const PAGES_URL = 'http://127.0.0.1:4174/Flying/?selftest=1';

interface Golden {
  checkpoints: { tick: number; combined: string }[];
}
const golden = JSON.parse(
  readFileSync(new URL('../golden/determinism.json', import.meta.url), 'utf8'),
) as Golden;
const GOLDEN_10K = golden.checkpoints.find((c) => c.tick === 10_000)?.combined;

/** Collects anything that would indicate a CSP violation or a script error. */
function watchErrors(page: Page): string[] {
  const problems: string[] = [];
  page.on('console', (m) => {
    if (m.type() === 'error') {
      problems.push(`console: ${m.text()}`);
    }
  });
  page.on('pageerror', (e) => problems.push(`pageerror: ${e.message}`));
  return problems;
}

const cell = (page: Page, id: string) => page.locator(`[data-testid="${id}"]`);

async function expectDone(page: Page): Promise<void> {
  await expect(cell(page, 'status')).toHaveText('done', { timeout: 30_000 });
}

test('golden hash for tick 10000 exists', () => {
  expect(GOLDEN_10K).toMatch(/^[0-9a-f]{16}$/);
});

test('host with headers: isolated on first load, threads build, golden hash', async ({ page }) => {
  const problems = watchErrors(page);
  const response = await page.goto(HEADERS_URL);
  expect(response?.headers()['cross-origin-opener-policy']).toBe('same-origin');
  expect(response?.headers()['cross-origin-embedder-policy']).toBe('require-corp');
  expect(response?.headers()['content-security-policy']).toContain("default-src 'self'");

  await expectDone(page);
  await expect(cell(page, 'isolated')).toHaveText('true');
  await expect(cell(page, 'sab')).toHaveText('true');
  await expect(cell(page, 'mode')).toHaveText('full');
  await expect(cell(page, 'variant')).toHaveText('threads');
  await expect(cell(page, 'abi')).toHaveText(String(ABI.VERSION));
  await expect(cell(page, 'ticks')).toHaveText('10000');
  await expect(cell(page, 'hash')).toHaveText(GOLDEN_10K ?? 'missing');
  await expect(cell(page, 'build-id')).not.toHaveText('');
  expect(problems).toEqual([]);
});

test('host without headers (GitHub Pages): Service Worker isolates after one reload', async ({
  page,
}) => {
  const problems = watchErrors(page);
  let mainFrameNavigations = 0;
  page.on('framenavigated', (frame) => {
    if (frame === page.mainFrame()) {
      mainFrameNavigations += 1;
    }
  });

  await page.goto(PAGES_URL);
  await expectDone(page);
  await expect(cell(page, 'isolated')).toHaveText('true');
  await expect(cell(page, 'mode')).toHaveText('full');
  await expect(cell(page, 'sw')).toHaveText('active');
  await expect(cell(page, 'variant')).toHaveText('threads');
  await expect(cell(page, 'hash')).toHaveText(GOLDEN_10K ?? 'missing');
  expect(mainFrameNavigations, 'initial load plus exactly one reload').toBe(2);
  expect(problems).toEqual([]);
});

test('Service Worker shell works offline', async ({ page, context }) => {
  const problems = watchErrors(page);
  await page.goto(PAGES_URL);
  await expectDone(page);

  await context.setOffline(true);
  await page.reload();
  await expectDone(page);
  await expect(cell(page, 'isolated')).toHaveText('true');
  await expect(cell(page, 'variant')).toHaveText('threads');
  await expect(cell(page, 'hash')).toHaveText(GOLDEN_10K ?? 'missing');
  expect(problems).toEqual([]);
});

test.describe('service workers blocked', () => {
  test.use({ serviceWorkers: 'block' });

  test('no headers and no Service Worker: degraded mode, single build, same hash', async ({
    page,
  }) => {
    await page.goto(PAGES_URL);
    await expectDone(page);
    await expect(cell(page, 'isolated')).toHaveText('false');
    await expect(cell(page, 'mode')).toHaveText('degraded');
    await expect(cell(page, 'variant')).toHaveText('single');
    await expect(cell(page, 'hash')).toHaveText(GOLDEN_10K ?? 'missing');
  });
});
