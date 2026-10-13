import { expect, test, type Page } from '@playwright/test';
import { byId, HEADERS_BASE, openGame } from './support.ts';

const value = async (page: Page, id: string): Promise<number> =>
  Number.parseFloat((await byId(page, id).getAttribute('data-value')) ?? 'NaN');

test('holding W pushes the nose down; releasing it stops the change', async ({ page }) => {
  await openGame(page, `${HEADERS_BASE}?debug=1`);
  await expect.poll(() => value(page, 'ov-pitch')).toBeCloseTo(0, 0);
  await page.keyboard.down('KeyW');
  await expect.poll(() => value(page, 'ov-pitch'), { timeout: 10_000 }).toBeLessThan(-5);
  await page.keyboard.up('KeyW');
  // The sim has the key's release within a tick or two; after that the pitch only drifts with the axis return.
  await page.waitForTimeout(1500);
  const settled = await value(page, 'ov-pitch');
  await page.waitForTimeout(1000);
  expect(await value(page, 'ov-pitch')).toBeCloseTo(settled, 1);
});

test('R puts the observer back at the start', async ({ page }) => {
  await openGame(page, `${HEADERS_BASE}?debug=1`);
  await page.keyboard.down('KeyW');
  await expect.poll(() => value(page, 'ov-pitch'), { timeout: 10_000 }).toBeLessThan(-20);
  await page.keyboard.up('KeyW');
  // Let the emulated stick return to centre, so the reset leaves nothing behind to integrate.
  await page.waitForTimeout(1000);
  await page.keyboard.press('KeyR');
  await expect
    .poll(async () => Math.abs(await value(page, 'ov-pitch')), { timeout: 10_000 })
    .toBeLessThan(1);
  await expect(byId(page, 'ov-heading')).toHaveAttribute('data-value', '0.0°');
});

test('losing focus sends a neutral frame: the sticks go to zero while the throttle holds', async ({
  page,
}) => {
  await openGame(page, `${HEADERS_BASE}?debug=1`);
  await page.keyboard.down('KeyW');
  await expect.poll(() => value(page, 'ov-pitch'), { timeout: 10_000 }).toBeLessThan(-5);
  const throttleBefore = await value(page, 'ov-throttle');
  // A blur with the key still held, as when the person switches windows.
  await page.evaluate(() => {
    window.dispatchEvent(new Event('blur'));
  });
  await page.waitForTimeout(1500);
  const settled = await value(page, 'ov-pitch');
  await page.waitForTimeout(1000);
  expect(await value(page, 'ov-pitch')).toBeCloseTo(settled, 1);
  expect(await value(page, 'ov-throttle')).toBeCloseTo(throttleBefore, 1);
  await page.keyboard.up('KeyW');
});

test('input latency is measured and reported', async ({ page }) => {
  await openGame(page, `${HEADERS_BASE}?debug=1`);
  await page.keyboard.down('KeyW');
  await page.waitForTimeout(1000);
  await page.keyboard.up('KeyW');
  const latency = await page.evaluate(() => window.__flying?.stats().sim);
  expect(latency).toBeDefined();
  await expect(byId(page, 'ov-latency')).toHaveAttribute('data-value', /^\d+\.\d{2} p95$/);
  // Reported for the record, with a very loose bound: these runs share a CI machine with the browser.
  const p95 = await value(page, 'ov-latency');
  console.log(`input latency p95: ${String(p95)} ms`);
  expect(p95).toBeLessThan(100);
});
