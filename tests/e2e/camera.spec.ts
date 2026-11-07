import { expect, test, type Page } from '@playwright/test';
import { HEADERS_BASE, openGame, byId, continuePastCompat } from './support.ts';

const FLIGHT = `${HEADERS_BASE}?flags=stall`;
const view = (page: Page) => page.evaluate(() => window.__flying?.view());

test('C switches between the cockpit and the chase camera, and back', async ({ page }) => {
  await openGame(page, FLIGHT);
  expect((await view(page))?.camera).toBe('cockpit');
  await page.keyboard.press('KeyC');
  await expect.poll(async () => (await view(page))?.camera).toBe('chase');
  await page.keyboard.press('KeyC');
  await expect.poll(async () => (await view(page))?.camera).toBe('cockpit');
});

test('?camera= starts the sortie in that camera', async ({ page }) => {
  await openGame(page, `${FLIGHT}&camera=chase`);
  await expect.poll(async () => (await view(page))?.camera).toBe('chase');
});

test('the look keys turn the head while held and let go when released', async ({ page }) => {
  await openGame(page, FLIGHT);
  await page.keyboard.down('Period');
  await expect.poll(async () => (await view(page))?.lookYawRate).toBeGreaterThan(0);
  await page.keyboard.up('Period');
  await expect.poll(async () => (await view(page))?.lookYawRate).toBe(0);
  await page.keyboard.down('Comma');
  await expect.poll(async () => (await view(page))?.lookYawRate).toBeLessThan(0);
  await page.keyboard.up('Comma');
  await page.keyboard.down('ArrowUp');
  await expect.poll(async () => (await view(page))?.lookPitchRate).toBeGreaterThan(0);
  await page.keyboard.up('ArrowUp');
});

test('looking up shows more sky, and the recentre key brings the view back', async ({ page }) => {
  await openGame(page, `${FLIGHT}&spawn=air`);
  await page.waitForTimeout(500);
  const luma = async (): Promise<number> =>
    (await page.evaluate(() => window.__flying?.readPixels()))?.meanLuma ?? 0;
  const level = await luma();
  await page.keyboard.down('ArrowUp');
  await page.waitForTimeout(1500); // the head reaches its limit, 60 degrees up
  const up = await luma();
  await page.keyboard.up('ArrowUp');
  // The sky is brighter than the ground. The canopy bows stay in view, so the whole picture only
  // brightens by a few hundredths; the head's limits are checked exactly in the camera rig's tests.
  expect(up).toBeGreaterThan(level + 0.02);
  await page.keyboard.press('Slash');
  await page.waitForTimeout(800);
  expect(Math.abs((await luma()) - level)).toBeLessThan(0.015);
});

test('zoom is held with the right mouse button, free look with X', async ({ page }) => {
  await openGame(page, FLIGHT);
  await page.mouse.move(400, 300);
  await page.mouse.down({ button: 'right' });
  await expect.poll(async () => (await view(page))?.zoom).toBe(true);
  await page.mouse.up({ button: 'right' });
  await expect.poll(async () => (await view(page))?.zoom).toBe(false);
  await page.keyboard.down('KeyX');
  await expect.poll(async () => (await view(page))?.freeLook).toBe(true);
  await page.keyboard.up('KeyX');
  await expect.poll(async () => (await view(page))?.freeLook).toBe(false);
});

test('I steps the HUD through its three levels of detail, and - and = dim and brighten it', async ({
  page,
}) => {
  await openGame(page, FLIGHT);
  expect((await view(page))?.declutter).toBe(0);
  for (const level of [1, 2, 0]) {
    await page.keyboard.press('KeyI');
    await expect.poll(async () => (await view(page))?.declutter).toBe(level);
  }
  await page.keyboard.press('Minus');
  await expect.poll(async () => (await view(page))?.brightness).toBe(90);
  await page.keyboard.press('Equal');
  await expect.poll(async () => (await view(page))?.brightness).toBe(100);
});

test("the camera chosen in the settings is the one a sortie starts in, and the settings' view numbers reach the view", async ({
  page,
}) => {
  await openGame(page, FLIGHT, { menu: true });
  await byId(page, 'firstrun-skip').click();
  await byId(page, 'menu-settings').click();
  await byId(page, 'settings-camera-chase').click();
  const fov = byId(page, 'settings-fov');
  await fov.fill('100');
  await byId(page, 'settings-tab-video').click();
  await byId(page, 'settings-declutter-2').click();
  await page.keyboard.press('Escape');
  await byId(page, 'menu-fly').click();
  await expect.poll(async () => (await view(page))?.camera).toBe('chase');
  expect((await view(page))?.fovDeg).toBe(100);
  expect((await view(page))?.declutter).toBe(2);
  // A reload keeps them.
  await page.reload();
  await continuePastCompat(page);
  await byId(page, 'menu-fly').click();
  await expect.poll(async () => (await view(page))?.camera).toBe('chase');
});

test("the aircraft's presentation loaded: there is no error, and the HUD draws", async ({
  page,
}) => {
  await openGame(page, `${FLIGHT}&spawn=air`);
  expect(await page.evaluate(() => window.__flying?.presentationError())).toBeUndefined();
});
