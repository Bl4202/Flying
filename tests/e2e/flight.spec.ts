/**
 * The real aircraft in the browser (v0.1a): the Kestrel spawns trimmed, flies on its own, answers
 * the throttle and the trim, and crashes and respawns. Numbers come from the debug overlay.
 */
import { expect, test } from '@playwright/test';
import { byId, HEADERS_BASE, openGame, overlayNumber, overlaySurfaces } from './support.ts';

const value = overlayNumber;
const status = (page: Parameters<typeof openGame>[0]) =>
  byId(page, 'ov-status').getAttribute('data-value');

test('the aircraft spawns trimmed in level flight', async ({ page }) => {
  await openGame(page, `${HEADERS_BASE}?debug=1`);
  const mach = await value(page, 'ov-mach');
  expect(mach).toBeGreaterThan(0.55);
  expect(mach).toBeLessThan(0.65);
  const alpha = await value(page, 'ov-alpha');
  expect(alpha).toBeGreaterThan(1);
  expect(alpha).toBeLessThan(6);
  expect(await status(page)).toBe('ok');
});

test('hands off, it holds its height for ten seconds', async ({ page }) => {
  await openGame(page, `${HEADERS_BASE}?debug=1`);
  const alt0 = await value(page, 'ov-alt');
  await page.waitForTimeout(10_000);
  expect(Math.abs((await value(page, 'ov-alt')) - alt0)).toBeLessThan(300);
  expect(await status(page)).toBe('ok');
});

test('full throttle lights all four afterburner stages', async ({ page }) => {
  await openGame(page, `${HEADERS_BASE}?debug=1`);
  await page.keyboard.down('ShiftLeft');
  await expect(byId(page, 'ov-throttle')).toHaveAttribute('data-value', /AB 4$/, {
    timeout: 5_000,
  });
  await expect.poll(() => value(page, 'ov-thrust'), { timeout: 3_000 }).toBeGreaterThan(90);
  await page.keyboard.up('ShiftLeft');
});

test('the trim keys move the elevator and it stays', async ({ page }) => {
  await openGame(page, `${HEADERS_BASE}?debug=1`);
  const [before] = await overlaySurfaces(page);
  await page.keyboard.down('BracketRight');
  await page.waitForTimeout(2000);
  await page.keyboard.up('BracketRight');
  await page.waitForTimeout(500);
  // Two degrees per second of nose-up trim: the trailing edge comes up about four degrees.
  const [after] = await overlaySurfaces(page);
  expect(before - after).toBeGreaterThan(3);
  expect(before - after).toBeLessThan(5);
});

test('flying into the ground crashes, and the aircraft respawns two seconds later', async ({
  page,
}) => {
  await openGame(page, `${HEADERS_BASE}?debug=1`);
  await page.keyboard.down('KeyW');
  await page.keyboard.down('ShiftLeft');
  await expect.poll(() => status(page), { timeout: 60_000 }).toContain('crashed');
  await page.keyboard.up('KeyW');
  await page.keyboard.up('ShiftLeft');
  await expect.poll(() => status(page), { timeout: 5_000 }).toBe('ok');
  expect(Math.abs((await value(page, 'ov-tas')) - 200)).toBeLessThan(5);
});
