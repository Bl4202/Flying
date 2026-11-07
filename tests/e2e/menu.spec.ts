import { expect, test, type Page } from '@playwright/test';
import {
  byId,
  continuePastCompat,
  HEADERS_BASE,
  openGame,
  waitForStored,
  watchErrors,
} from './support.ts';

const MENU_URL = `${HEADERS_BASE}?flags=stall`;

const flowName = (page: Page): Promise<string | undefined> =>
  page.evaluate(() => window.__flying?.flow());

/** Skips the first run if it shows, leaving the main menu. */
async function toMainMenu(page: Page): Promise<void> {
  const skip = byId(page, 'firstrun-skip');
  const menu = byId(page, 'main-menu');
  await expect(skip.or(menu)).toBeVisible({ timeout: 30_000 });
  if (await skip.isVisible()) await skip.click();
  await expect(menu).toBeVisible();
}

test('a first visit shows the setup and then the menu; the next visit goes straight to the menu', async ({
  page,
}) => {
  const problems = watchErrors(page);
  await openGame(page, MENU_URL, { menu: true });
  await expect(byId(page, 'first-run')).toBeVisible();
  expect(await flowName(page)).toBe('FIRST_RUN');
  await byId(page, 'firstrun-skip').click();
  await expect(byId(page, 'main-menu')).toBeVisible();
  expect(await flowName(page)).toBe('MAIN_MENU');
  await waitForStored(page, (s) => s['setupDone'] === true);

  await page.reload();
  await continuePastCompat(page);
  await expect(byId(page, 'main-menu')).toBeVisible();
  await expect(byId(page, 'first-run')).toHaveCount(0);
  expect(problems).toEqual([]);
});

test('Fly starts a sortie, Main menu ends it, and a second Fly starts a clean one', async ({
  page,
}) => {
  await openGame(page, MENU_URL, { menu: true });
  await toMainMenu(page);
  // Nothing flies behind the menu: the core is idle.
  expect((await page.evaluate(() => window.__flying?.status()))?.assist ?? 0).toBe(0);

  await byId(page, 'menu-fly').click();
  await expect(byId(page, 'main-menu')).toHaveCount(0);
  await expect.poll(() => flowName(page)).toBe('DEMO');
  await expect
    .poll(async () => (await page.evaluate(() => window.__flying?.status()))?.assist ?? 0)
    .toBeGreaterThan(0);
  // Push the throttle lever up so the first sortie leaves it high.
  await page.keyboard.down('Shift');
  await page.waitForTimeout(1500);
  await page.keyboard.up('Shift');
  const raised = (await page.evaluate(() => window.__flying?.status()))?.throttle ?? 0;
  expect(raised).toBeGreaterThan(0.3);

  // Main menu from the pause menu.
  await page.keyboard.press('Escape');
  await expect(byId(page, 'pause-menu')).toBeVisible();
  await byId(page, 'pause-main-menu').click();
  await expect(byId(page, 'main-menu')).toBeVisible();
  await expect(byId(page, 'pause-menu')).toHaveCount(0);
  expect(await flowName(page)).toBe('MAIN_MENU');
  // The core is idle again, and what it publishes says so: nothing of the last sortie lingers.
  await expect
    .poll(async () => (await page.evaluate(() => window.__flying?.status()))?.assist ?? -1)
    .toBe(0);

  // Fly again: a fresh core, a fresh spawn, and the lever follows the aircraft, not the last sortie.
  await byId(page, 'menu-fly').click();
  await expect.poll(() => flowName(page)).toBe('DEMO');
  await expect
    .poll(async () => (await page.evaluate(() => window.__flying?.status()))?.assist ?? 0)
    .toBeGreaterThan(0);
  await page.waitForTimeout(800);
  const state = await page.evaluate(() => ({
    aircraft: window.__flying?.status()?.throttle ?? -1,
    lever: window.__flying?.input()?.throttle ?? -1,
  }));
  expect(state.aircraft).toBeLessThan(0.2);
  expect(Math.abs(state.lever - state.aircraft)).toBeLessThan(0.05);
});

test('the arena and the assist level chosen in the menu reach the flight, and are remembered', async ({
  page,
}) => {
  await openGame(page, MENU_URL, { menu: true });
  await toMainMenu(page);
  await byId(page, 'menu-arena-1').click(); // the open plain
  await byId(page, 'menu-assist-1').click(); // Full
  await byId(page, 'menu-fly').click();
  await expect
    .poll(async () => (await page.evaluate(() => window.__flying?.status())) ?? null)
    .toMatchObject({ arena: 1, assist: 1 });
  await waitForStored(page, (s) => s['arena'] === 1 && s['assist'] === 1);

  await page.reload();
  await continuePastCompat(page);
  await expect(byId(page, 'main-menu')).toBeVisible();
  await expect(byId(page, 'menu-arena-1')).toHaveAttribute('aria-pressed', 'true');
  await expect(byId(page, 'menu-assist-1')).toHaveAttribute('aria-pressed', 'true');
});

test("a URL flag sets what the menu shows, and the player's own choice then wins at Fly", async ({
  page,
}) => {
  await openGame(page, `${MENU_URL}&arena=plain&assist=manual`, { menu: true });
  await toMainMenu(page);
  await expect(byId(page, 'menu-arena-1')).toHaveAttribute('aria-pressed', 'true');
  await expect(byId(page, 'menu-assist-4')).toHaveAttribute('aria-pressed', 'true');
  await byId(page, 'menu-arena-0').click();
  await byId(page, 'menu-fly').click();
  await expect
    .poll(async () => (await page.evaluate(() => window.__flying?.status())) ?? null)
    .toMatchObject({ arena: 0, assist: 4 });
});

test('the settings open from the menu, change a setting that persists, and Esc backs out', async ({
  page,
}) => {
  await openGame(page, MENU_URL, { menu: true });
  await toMainMenu(page);
  await byId(page, 'menu-settings').click();
  await expect(byId(page, 'settings-screen')).toBeVisible();
  await expect(byId(page, 'main-menu')).toHaveCount(0);
  await byId(page, 'settings-tab-video').click();
  await byId(page, 'settings-palette-amber').click();
  await page.keyboard.press('Escape');
  await expect(byId(page, 'main-menu')).toBeVisible();
  await waitForStored(page, (s) => (s['hud'] as { palette: string }).palette === 'amber');

  await page.reload();
  await continuePastCompat(page);
  await byId(page, 'menu-settings').click();
  await byId(page, 'settings-tab-video').click();
  await expect(byId(page, 'settings-palette-amber')).toHaveAttribute('aria-pressed', 'true');
});

test('settings export and import round-trip byte for byte (F2)', async ({ page }) => {
  await openGame(page, MENU_URL, { menu: true });
  await toMainMenu(page);
  await byId(page, 'menu-settings').click();
  await byId(page, 'settings-tab-audio').click();
  await byId(page, 'settings-mute-hidden').uncheck();
  await byId(page, 'settings-tab-data').click();

  const [download] = await Promise.all([
    page.waitForEvent('download'),
    byId(page, 'settings-export').click(),
  ]);
  expect(download.suggestedFilename()).toBe('flying-settings.json');
  const path = await download.path();
  const { readFileSync } = await import('node:fs');
  const exported = readFileSync(path, 'utf8');
  expect(
    (JSON.parse(exported) as { audio: { muteWhenHidden: boolean } }).audio.muteWhenHidden,
  ).toBe(false);

  // Change something else, then import the file: the exported state comes back.
  await byId(page, 'settings-tab-audio').click();
  await byId(page, 'settings-mute-hidden').check();
  await byId(page, 'settings-tab-data').click();
  await byId(page, 'settings-import').setInputFiles(path);
  await expect(byId(page, 'settings-status')).toHaveText('Settings imported.');
  await byId(page, 'settings-tab-audio').click();
  await expect(byId(page, 'settings-mute-hidden')).not.toBeChecked();

  const [again] = await Promise.all([
    page.waitForEvent('download'),
    (async () => {
      await byId(page, 'settings-tab-data').click();
      await byId(page, 'settings-export').click();
    })(),
  ]);
  expect(readFileSync(await again.path(), 'utf8')).toBe(exported);
});

test('a settings file with a bad field is refused with its pointer, and changes nothing', async ({
  page,
}, testInfo) => {
  await openGame(page, MENU_URL, { menu: true });
  await toMainMenu(page);
  await byId(page, 'menu-settings').click();
  await byId(page, 'settings-tab-data').click();
  const { writeFileSync } = await import('node:fs');
  const bad = testInfo.outputPath('bad-settings.json');
  writeFileSync(bad, JSON.stringify({ version: 2, camera: 'orbit' }));
  await byId(page, 'settings-import').setInputFiles(bad);
  await expect(byId(page, 'settings-problems')).toContainText('/camera');
  await byId(page, 'settings-tab-gameplay').click();
  await expect(byId(page, 'settings-camera-cockpit')).toHaveAttribute('aria-pressed', 'true');
});
