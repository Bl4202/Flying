/**
 * The dogfight as a player starts it: chosen in the main menu (Mode, Bandit, Fly), the duel is on
 * and the bandit alive; and with the bot playtester in the seat, its sortie from the runway shoots
 * the Recruit down (the bot is built for that sortie, ADR-47: from the merge's tail chase it
 * circles the Recruit without a kill).
 */
import { expect, test } from '@playwright/test';
import { byId, CHEAP, COUNTER, HEADERS_BASE, openGame, watchErrors } from './support.ts';

/** `COMBAT_FLAG.ACTIVE` and `SCENARIO.DUEL` (`sim-bridge`). */
const ACTIVE = 1;
const DUEL = 1;

test('the main menu starts a dogfight against the chosen bandit', async ({ page }) => {
  test.setTimeout(90_000);
  const problems = watchErrors(page);
  await openGame(page, `${HEADERS_BASE}?debug=1&${CHEAP}`, { menu: true });
  const skip = byId(page, 'firstrun-skip');
  const menu = byId(page, 'main-menu');
  await expect(skip.or(menu)).toBeVisible({ timeout: 30_000 });
  if (await skip.isVisible()) await skip.click();
  await expect(byId(page, 'menu-bandit')).toBeHidden();
  await byId(page, `menu-mode-${String(DUEL)}`).click();
  await expect(byId(page, 'menu-bandit')).toBeVisible();
  await byId(page, 'menu-bandit-1').click();
  await expect(byId(page, 'menu-bandit-preview')).toHaveText(
    'Holds fire: a target that runs and chases.',
  );
  await byId(page, 'menu-fly').click();
  await expect(byId(page, 'main-menu')).toHaveCount(0);
  await expect
    .poll(
      () =>
        page.evaluate(() => {
          const c = window.__flying?.combat();
          return c === undefined ? 0 : (c.flags & 1) + (c.banditLife === 1 ? 2 : 0);
        }),
      { timeout: 20_000 },
    )
    .toBe(ACTIVE + 2);
  expect(await page.evaluate(() => window.__flying?.combat()?.banditSkill)).toBe(1);
  expect(problems).toEqual([]);
});

test("the bot playtester's sortie from the runway shoots the Recruit down", async ({ page }) => {
  test.setTimeout(6 * 60_000);
  const problems = watchErrors(page);
  await openGame(page, `${HEADERS_BASE}?debug=1&scenario=duel&skill=1&script=sortie&${CHEAP}`);
  await expect
    .poll(
      () =>
        page.evaluate(() => {
          const c = window.__flying?.combat();
          return c === undefined ? 0 : (Array.from(c.counters)[4] ?? 0);
        }),
      { timeout: 5 * 60_000, intervals: [1000] },
    )
    .toBeGreaterThanOrEqual(1);
  const counters = await page.evaluate(() => {
    const c = window.__flying?.combat();
    return c === undefined ? [] : Array.from(c.counters);
  });
  expect(counters[COUNTER.KILLS]).toBeGreaterThanOrEqual(1);
  expect(counters[COUNTER.DEATHS]).toBe(0);
  expect(problems).toEqual([]);
});
