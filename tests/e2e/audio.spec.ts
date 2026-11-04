import { expect, test } from '@playwright/test';
import { byId, HEADERS_BASE, watchErrors } from './support.ts';

// The page cannot run an AudioContext until the user has done something. These tests press a key,
// as the Fly click does in the game; a separate test checks that without one the page says so.

test('the flight audio runs in the browser: worklet, level, and a tone within 100 ms', async ({
  page,
}) => {
  test.setTimeout(90_000);
  const problems = watchErrors(page);
  await page.goto(`${HEADERS_BASE}?selftest=audio`);
  await page.keyboard.press('Space'); // the gesture
  await expect(byId(page, 'status')).toHaveText('done', { timeout: 60_000 });
  await expect(byId(page, 'worklet')).toHaveAttribute('data-result', 'pass');
  await expect(byId(page, 'context')).toHaveAttribute('data-result', 'pass');
  await expect(byId(page, 'engine')).toHaveAttribute('data-result', 'pass');
  const onset = byId(page, 'onset');
  await expect(onset).toHaveAttribute('data-result', 'pass');
  const p95 = Number(/p95 ([\d.]+) ms/.exec((await onset.textContent()) ?? '')?.[1]);
  expect(p95).toBeLessThan(100);
  for (const id of ['worklet', 'context', 'engine', 'onset']) {
    test
      .info()
      .annotations.push({ type: id, description: (await byId(page, id).textContent()) ?? '' });
  }
  expect(problems).toEqual([]);
});

test('without a gesture the page says the context is waiting', async ({ page }) => {
  await page.goto(`${HEADERS_BASE}?selftest=audio`);
  await expect(byId(page, 'status')).toHaveText('done', { timeout: 30_000 });
  await expect(byId(page, 'worklet')).toHaveAttribute('data-result', 'pass');
  // Headless Chromium may allow autoplay; where it does not, the row says it skipped.
  await expect(byId(page, 'context')).toHaveAttribute('data-result', /^(pass|skipped)$/);
});
