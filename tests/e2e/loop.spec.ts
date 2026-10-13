import { expect, test } from '@playwright/test';
import { byId, HEADERS_BASE, openGame, simSample, watchErrors } from './support.ts';

test('the loop runs: 60 Hz ticks, frames advance, and the picture has more than a clear colour', async ({
  page,
}) => {
  const problems = watchErrors(page);
  await openGame(page, `${HEADERS_BASE}?debug=1`);

  const before = await simSample(page);
  const framesBefore = await page.evaluate(() => window.__flying?.stats().render?.frames ?? 0);
  await page.waitForTimeout(2000);
  const after = await simSample(page);
  const framesAfter = await page.evaluate(() => window.__flying?.stats().render?.frames ?? 0);

  expect(before).toBeDefined();
  expect(after).toBeDefined();
  if (before === undefined || after === undefined) return;
  // Rates are taken between the reports' own timestamps, so when we happened to look does not matter.
  const seconds = (after.atMs - before.atMs) / 1000;
  expect(seconds).toBeGreaterThan(1.5);
  expect((after.tick - before.tick) / seconds).toBeGreaterThan(59);
  expect((after.tick - before.tick) / seconds).toBeLessThan(61);
  expect(framesAfter - framesBefore).toBeGreaterThan(30);

  const pixels = await page.evaluate(() => window.__flying?.readPixels());
  expect(pixels?.distinctColours ?? 0).toBeGreaterThanOrEqual(8);
  expect(problems).toEqual([]);
});

test('the overlay shows the content hash of the loaded pack', async ({ page }) => {
  await openGame(page, `${HEADERS_BASE}?debug=1`);
  await expect(byId(page, 'ov-content')).toHaveAttribute('data-value', /^[0-9a-f]{16}$/);
});

test('a hidden tab pauses the sim, and showing it again resumes', async ({ page }) => {
  await openGame(page, `${HEADERS_BASE}?debug=1`);
  const setVisibility = (state: 'hidden' | 'visible') =>
    page.evaluate((s) => {
      Object.defineProperty(document, 'visibilityState', { value: s, configurable: true });
      document.dispatchEvent(new Event('visibilitychange'));
    }, state);

  await setVisibility('hidden');
  await expect(byId(page, 'ov-core')).toHaveAttribute('data-value', 'paused');
  const frozen = await byId(page, 'ov-tick').getAttribute('data-value');
  await page.waitForTimeout(1000);
  expect(await byId(page, 'ov-tick').getAttribute('data-value')).toBe(frozen);

  await setVisibility('visible');
  await expect(byId(page, 'ov-core')).toHaveAttribute('data-value', 'running');
  await expect
    .poll(async () => Number(await byId(page, 'ov-tick').getAttribute('data-value')))
    .toBeGreaterThan(Number(frozen));
});

test('a person who paused stays paused through hiding and showing the tab', async ({ page }) => {
  await openGame(page, `${HEADERS_BASE}?debug=1`);
  await byId(page, 'ov-pause').click();
  await expect(byId(page, 'ov-core')).toHaveAttribute('data-value', 'paused');
  await page.evaluate(() => {
    Object.defineProperty(document, 'visibilityState', { value: 'hidden', configurable: true });
    document.dispatchEvent(new Event('visibilitychange'));
    Object.defineProperty(document, 'visibilityState', { value: 'visible', configurable: true });
    document.dispatchEvent(new Event('visibilitychange'));
  });
  await page.waitForTimeout(500);
  await expect(byId(page, 'ov-core')).toHaveAttribute('data-value', 'paused');
});
