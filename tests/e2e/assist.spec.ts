/**
 * The assist levels in the browser (GDD 11.11, ADR-34): chosen from the pause menu or the key,
 * switched in flight without a jump, kept between visits, and set by the URL for one visit.
 */
import { expect, test } from '@playwright/test';
import { ASSIST, byId, flightStatus, HEADERS_BASE, openGame, statusNow } from './support.ts';

const AIR = `${HEADERS_BASE}?debug=1&spawn=air`;
const assistNow = async (page: Parameters<typeof openGame>[0]): Promise<number | undefined> =>
  (await statusNow(page))?.assist;

test('Standard assist is the default, and the pause menu holds the sim', async ({ page }) => {
  await openGame(page, AIR);
  expect((await flightStatus(page)).assist).toBe(ASSIST.STANDARD);
  await page.keyboard.press('Escape');
  await expect(byId(page, 'pause-menu')).toBeVisible();
  expect(await page.evaluate(() => window.__flying?.menu())).toBe('pause');
  await expect(byId(page, `pause-assist-${String(ASSIST.STANDARD)}`)).toHaveAttribute(
    'aria-pressed',
    'true',
  );
  await expect(byId(page, 'ov-core')).toHaveAttribute('data-value', 'paused');
  const frozen = await byId(page, 'ov-tick').getAttribute('data-value');
  await page.waitForTimeout(800);
  expect(await byId(page, 'ov-tick').getAttribute('data-value')).toBe(frozen);
  // Esc again: the sim runs on.
  await page.keyboard.press('Escape');
  await expect(byId(page, 'pause-menu')).toHaveCount(0);
  await expect(byId(page, 'ov-core')).toHaveAttribute('data-value', 'running');
});

test('the chips switch the assist in flight, and the switch is bumpless', async ({ page }) => {
  test.setTimeout(90_000);
  await openGame(page, AIR);
  await flightStatus(page);
  for (const [level, name] of [
    [ASSIST.FULL, 'full'],
    [ASSIST.REDUCED, 'reduced'],
    [ASSIST.MANUAL, 'manual'],
    [ASSIST.STANDARD, 'standard'],
  ] as const) {
    const before = (await flightStatus(page)).nz;
    await page.keyboard.press('Escape');
    await byId(page, `pause-assist-${String(level)}`).click();
    await byId(page, 'pause-resume').click();
    await expect.poll(() => assistNow(page), { timeout: 5_000, message: name }).toBe(level);
    // Watch the load factor for a second and a half after the switch: no jump (a transient of
    // under 0.3 g is the gate, ADR-33).
    let worst = 0;
    for (let i = 0; i < 30; i++) {
      await page.waitForTimeout(50);
      const nz = (await statusNow(page))?.nz ?? before;
      worst = Math.max(worst, Math.abs(nz - before));
    }
    expect(worst, `n_z after switching to ${name}`).toBeLessThan(0.3);
  }
});

test('F8 cycles the assist level, and says so', async ({ page }) => {
  await openGame(page, AIR);
  expect((await flightStatus(page)).assist).toBe(ASSIST.STANDARD);
  await page.keyboard.press('F8');
  await expect.poll(() => assistNow(page), { timeout: 5_000 }).toBe(ASSIST.REDUCED);
  await expect(byId(page, 'toast').first()).toContainText('Reduced');
  await page.keyboard.press('F8');
  await expect.poll(() => assistNow(page), { timeout: 5_000 }).toBe(ASSIST.MANUAL);
});

test('?assist=manual starts under Manual, and the menu says so', async ({ page }) => {
  await openGame(page, `${AIR}&assist=manual`);
  expect((await flightStatus(page)).assist).toBe(ASSIST.MANUAL);
  await page.keyboard.press('Escape');
  await expect(byId(page, `pause-assist-${String(ASSIST.MANUAL)}`)).toHaveAttribute(
    'aria-pressed',
    'true',
  );
});

test('the choice is kept for the next visit, and the URL still wins for one', async ({ page }) => {
  await openGame(page, AIR);
  await flightStatus(page);
  await page.keyboard.press('Escape');
  await byId(page, `pause-assist-${String(ASSIST.REDUCED)}`).click();
  await byId(page, 'pause-resume').click();
  await expect.poll(() => assistNow(page), { timeout: 5_000 }).toBe(ASSIST.REDUCED);
  // Give the store a moment, then come back with nothing in the URL.
  await page.waitForTimeout(300);
  await openGame(page, AIR);
  expect((await flightStatus(page)).assist).toBe(ASSIST.REDUCED);
  await openGame(page, `${AIR}&assist=full`);
  expect((await flightStatus(page)).assist).toBe(ASSIST.FULL);
});

test('Respawn now from the menu puts the aircraft back at its start', async ({ page }) => {
  await openGame(page, `${HEADERS_BASE}?debug=1`);
  const first = await flightStatus(page);
  await page.keyboard.press('Escape');
  await byId(page, 'pause-respawn').click();
  await expect
    .poll(async () => (await statusNow(page))?.spawnSeq ?? first.spawnSeq, { timeout: 5_000 })
    .not.toBe(first.spawnSeq);
  await expect(byId(page, 'pause-menu')).toHaveCount(0);
});
