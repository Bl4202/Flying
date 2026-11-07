import { expect, test, type Page } from '@playwright/test';
import { byId, HEADERS_BASE, openGame, waitForStored } from './support.ts';

const URL = `${HEADERS_BASE}?flags=stall`;

/** Gives the page a joystick, as the Gamepad API reports a raw device. */
async function plugJoystick(page: Page): Promise<void> {
  await page.addInitScript(() => {
    const stick = {
      id: 'Test Stick 3000 (Vendor: 1234 Product: abcd)',
      index: 0,
      connected: true,
      mapping: '',
      timestamp: 1,
      axes: [0, 0, 0, 0, 0],
      buttons: Array.from({ length: 8 }, () => ({ pressed: false, touched: false, value: 0 })),
      vibrationActuator: null,
    };
    navigator.getGamepads = () => [stick as unknown as Gamepad, null, null, null];
  });
}

test('every step can be reached and operated by the keyboard alone, with a visible focus', async ({
  page,
}) => {
  await openGame(page, URL, { menu: true });
  await expect(byId(page, 'first-run')).toBeVisible();
  // Focus starts on the step's heading, and Tab moves to a control whose focus ring shows.
  await expect(byId(page, 'firstrun-heading')).toBeFocused();
  const steps: string[] = [];
  for (let guard = 0; guard < 8; guard++) {
    const step = await byId(page, 'first-run').getAttribute('data-step');
    steps.push(step ?? '');
    if (step === 'done') break;
    await page.keyboard.press('Tab');
    const ring = await page.evaluate(() => {
      const el = document.activeElement;
      if (!el) return 0;
      const style = getComputedStyle(el);
      return style.outlineStyle === 'none' ? 0 : Number.parseFloat(style.outlineWidth);
    });
    expect(ring).toBeGreaterThan(0);
    // Move focus to Next with Tab, and press it with Enter.
    for (let i = 0; i < 40; i++) {
      if (await byId(page, 'firstrun-next').evaluate((el) => el === document.activeElement)) break;
      await page.keyboard.press('Tab');
    }
    await page.keyboard.press('Enter');
    await expect(byId(page, 'first-run')).not.toHaveAttribute('data-step', step ?? '');
  }
  expect(steps).toEqual(['device', 'assist', 'comfort', 'notice', 'done']);
  await byId(page, 'firstrun-next').focus();
  await page.keyboard.press('Enter');
  await expect(byId(page, 'main-menu')).toBeVisible();
});

test('a joystick adds the calibration step', async ({ page }) => {
  await plugJoystick(page);
  await openGame(page, URL, { menu: true });
  await expect(byId(page, 'firstrun-detected')).toContainText('Found: joy:1234:abcd');
  // The controls profile follows the device that is plugged in, so a stick means the longer setup.
  await expect(byId(page, 'firstrun-device-joystick')).toHaveAttribute('aria-pressed', 'true');
  await expect(byId(page, 'firstrun-step')).toHaveText('Step 1 of 6');
  await byId(page, 'firstrun-next').click();
  await expect(byId(page, 'first-run')).toHaveAttribute('data-step', 'calibration');
  await expect(byId(page, 'firstrun-calibrate')).toBeVisible();
  // Choosing the keyboard instead takes the calibration step away.
  await byId(page, 'firstrun-back').click();
  await byId(page, 'firstrun-device-km').click();
  await expect(byId(page, 'firstrun-step')).toHaveText('Step 1 of 5');
});

test('the assist choice applies to the first flight', async ({ page }) => {
  await openGame(page, URL, { menu: true });
  await byId(page, 'firstrun-next').click(); // device -> assist
  await byId(page, 'firstrun-assist-1').click(); // Full: "I'm new"
  await byId(page, 'firstrun-skip').click();
  await byId(page, 'menu-fly').click();
  await expect
    .poll(async () => (await page.evaluate(() => window.__flying?.status()))?.assist ?? 0)
    .toBe(1);
});

test('the setup shows once, and Run setup again brings it back', async ({ page }) => {
  await openGame(page, URL, { menu: true });
  await byId(page, 'firstrun-skip').click();
  await waitForStored(page, (s) => s['setupDone'] === true);
  await page.reload();
  await expect(byId(page, 'main-menu')).toBeVisible();
  await expect(byId(page, 'first-run')).toHaveCount(0);
  await byId(page, 'menu-settings').click();
  await byId(page, 'settings-tab-data').click();
  await byId(page, 'settings-setup').click();
  await expect(byId(page, 'first-run')).toHaveAttribute('data-step', 'device');
});
