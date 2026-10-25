import { expect, test } from '@playwright/test';
import { byId, HEADERS_BASE, openGame, overlayNumber, overlaySurfaces } from './support.ts';

const value = overlayNumber;
/** These specs fly in the air, where the stick has something to push against. */
const AIR = `${HEADERS_BASE}?debug=1&spawn=air`;

test('holding S pulls the nose up and loads the airframe', async ({ page }) => {
  await openGame(page, AIR);
  const pitch0 = await value(page, 'ov-pitch');
  await page.keyboard.down('KeyS');
  await expect.poll(() => value(page, 'ov-nz'), { timeout: 5_000 }).toBeGreaterThan(1.3);
  await expect.poll(() => value(page, 'ov-pitch'), { timeout: 5_000 }).toBeGreaterThan(pitch0 + 3);
  await page.keyboard.up('KeyS');
});

test('R respawns the aircraft: level, heading north, at 200 m/s', async ({ page }) => {
  await openGame(page, AIR);
  const spawnPitch = await value(page, 'ov-pitch');
  await page.keyboard.down('KeyW');
  await expect.poll(() => value(page, 'ov-pitch'), { timeout: 10_000 }).toBeLessThan(-20);
  await page.keyboard.up('KeyW');
  // Let the emulated stick return to centre, so the reset leaves nothing behind.
  await page.waitForTimeout(1000);
  await page.keyboard.press('KeyR');
  await expect
    .poll(async () => Math.abs((await value(page, 'ov-tas')) - 200), { timeout: 10_000 })
    .toBeLessThan(3);
  expect(Math.abs((await value(page, 'ov-pitch')) - spawnPitch)).toBeLessThan(1);
  const heading = await value(page, 'ov-heading');
  expect(Math.min(heading, 360 - heading)).toBeLessThan(0.5);
});

test('losing focus sends a neutral frame: the stick centres while the throttle holds', async ({
  page,
}) => {
  await openGame(page, AIR);
  const [trim] = await overlaySurfaces(page);
  await page.keyboard.down('KeyW');
  // Stick forward: elevator trailing edge down, well away from the trim.
  await expect
    .poll(async () => (await overlaySurfaces(page))[0], { timeout: 10_000 })
    .toBeGreaterThan(trim + 5);
  const throttleBefore = await value(page, 'ov-throttle');
  // A blur with the key still held, as when the person switches windows.
  await page.evaluate(() => {
    window.dispatchEvent(new Event('blur'));
  });
  await expect
    .poll(async () => Math.abs((await overlaySurfaces(page))[0] - trim), { timeout: 5_000 })
    .toBeLessThan(0.5);
  expect(await value(page, 'ov-throttle')).toBeCloseTo(throttleBefore, 2);
  await page.keyboard.up('KeyW');
});

test('input latency is measured and reported', async ({ page }) => {
  await openGame(page, AIR);
  await page.keyboard.down('KeyW');
  await page.waitForTimeout(1000);
  await page.keyboard.up('KeyW');
  const latency = await page.evaluate(() => window.__flying?.stats().sim);
  expect(latency).toBeDefined();
  // The Control row: source event to the end of the tick that acted on it (AC-58.3, ADR-33).
  await expect(byId(page, 'ov-ctl')).toHaveAttribute('data-value', /^\d+\.\d{2} p95 · poll/);
  // Reported for the record, with a very loose bound: these runs share a CI machine with the browser.
  const p95 = await value(page, 'ov-ctl');
  console.log(`control latency p95: ${String(p95)} ms`);
  expect(p95).toBeLessThan(100);
});
