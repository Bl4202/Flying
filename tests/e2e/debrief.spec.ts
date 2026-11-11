/**
 * The debrief (ADR-45 ST4, ST6): End sortie opens it, a keyboard works it, its numbers are the
 * snapshot counters' change since Fly, Rate this flight keeps the answers, and its DOM is gone
 * once it closes; and a duel's sortie that ends with a stop on the runway opens it on its own.
 */
import { expect, test, type Page } from '@playwright/test';
import { byId, CHEAP, COUNTER, HEADERS_BASE, openGame, watchErrors } from './support.ts';

const MERGE = `${HEADERS_BASE}?debug=1&spawn=air&scenario=merge&skill=1&${CHEAP}`;

/** The test id of the element with the focus. */
const focused = (page: Page): Promise<string | undefined> =>
  page.evaluate(() => (document.activeElement as HTMLElement | null)?.dataset['testid']);

/** Presses Tab until `id` has the focus (or fails after `limit` presses). */
async function tabTo(page: Page, id: string, limit = 60): Promise<void> {
  for (let i = 0; i < limit; i++) {
    if ((await focused(page)) === id) return;
    await page.keyboard.press('Tab');
  }
  throw new Error(`Tab never reached ${id} (focus on ${(await focused(page)) ?? 'nothing'})`);
}

test('End sortie opens the debrief, a keyboard works it and the questionnaire, and both are gone after (ST4, ST6)', async ({
  page,
}) => {
  test.setTimeout(120_000);
  const problems = watchErrors(page);
  await openGame(page, MERGE);
  await expect
    .poll(() => page.evaluate(() => window.__flying?.combat()?.banditLife ?? 0), {
      timeout: 15_000,
    })
    .toBe(1);
  // A burst of the gun, so the debrief has something to count (after a second of flight: a key
  // held from the very first frame is not taken as a press).
  await page.waitForTimeout(1000);
  await page.keyboard.down('Space');
  await page.waitForTimeout(800);
  await page.keyboard.up('Space');
  await page.waitForTimeout(500);

  // Esc, then Tab to End sortie and Enter: keyboard only.
  await page.keyboard.press('Escape');
  await expect(byId(page, 'pause-menu')).toBeVisible();
  await tabTo(page, 'pause-end-sortie');
  await page.keyboard.press('Enter');
  await expect(byId(page, 'debrief')).toBeVisible();
  expect(await page.evaluate(() => window.__flying?.menu())).toBe('debrief');
  expect(await focused(page)).toBe('debrief-fly-again');

  // Its numbers are the counters (from zero at Fly), which the held simulation no longer moves.
  const counters = await page.evaluate(() => {
    const c = window.__flying?.combat();
    return c === undefined ? [] : Array.from(c.counters);
  });
  const shown = await page.evaluate(() => window.__flying?.debrief());
  expect(shown).not.toBeNull();
  expect(shown?.gunRounds).toBe(counters[COUNTER.ROUNDS_FIRED]);
  expect(shown?.gunRounds ?? 0).toBeGreaterThan(0);
  expect(shown?.gunHits).toBe(counters[COUNTER.ROUND_HITS]);
  expect(shown?.kills).toBe(counters[COUNTER.KILLS]);
  expect(shown?.missilesFired).toBe(counters[COUNTER.MISSILES_FIRED]);
  expect(shown?.deaths).toBe(counters[COUNTER.DEATHS]);
  expect(shown?.hitsTaken).toBe(counters[COUNTER.HITS_TAKEN]);
  expect(shown?.outcome).toBe('ended');

  // Rate this flight: three answers by the arrow keys, a comment, Save.
  await tabTo(page, 'debrief-rate');
  await page.keyboard.press('Enter');
  await expect(byId(page, 'feel-survey')).toBeVisible();
  expect(await focused(page)).toBe('feel-believable-1');
  for (const [group, steps] of [
    ['believable', 3],
    ['satisfying', 4],
    ['easy', 1],
  ] as const) {
    await tabTo(page, `feel-${group}-1`);
    for (let i = 0; i < steps; i++) await page.keyboard.press('ArrowRight');
    expect(await focused(page)).toBe(`feel-${group}-${String(steps + 1)}`);
  }
  await tabTo(page, 'feel-comment');
  await page.keyboard.type('Quick check from the debrief spec.');
  await tabTo(page, 'feel-save');
  await page.keyboard.press('Enter');
  await expect(byId(page, 'feel-survey')).toHaveCount(0);
  await expect(byId(page, 'debrief')).toBeVisible();

  // Main menu: the debrief's DOM is gone, and the mission and the answers are kept.
  await tabTo(page, 'debrief-menu');
  await page.keyboard.press('Enter');
  await expect(byId(page, 'main-menu')).toBeVisible();
  await expect(byId(page, 'debrief')).toHaveCount(0);
  await expect(byId(page, 'feel-survey')).toHaveCount(0);
  expect(await page.evaluate(() => window.__flying?.debrief())).toBeNull();
  await byId(page, 'menu-statistics').click();
  await expect(byId(page, 'stats-total-missions').locator('dd')).toHaveText('1');
  await expect(byId(page, 'stats-feel-count')).toHaveText('Sets of answers: 1');
  await expect(byId(page, 'stats-feel-median')).toContainText(
    'believable 4, satisfying 5, easy to learn 2',
  );
  expect(problems).toEqual([]);
});

test("a duel's sortie that ends with a stop on the runway opens the debrief on its own (ST4)", async ({
  page,
}) => {
  // The circuit tester takes off, flies the circuit and lands, while the Recruit (which never
  // fires) chases it: the one long e2e test.
  test.setTimeout(8 * 60_000);
  const problems = watchErrors(page);
  await openGame(page, `${HEADERS_BASE}?debug=1&scenario=duel&skill=1&script=circuit&${CHEAP}`);
  await expect
    .poll(() => page.evaluate(() => window.__flying?.debrief() ?? null), {
      timeout: 7 * 60_000,
      intervals: [1000],
    })
    .not.toBeNull();
  await expect(byId(page, 'debrief')).toBeVisible();
  const shown = await page.evaluate(() => window.__flying?.debrief());
  expect(shown?.outcome).toBe('landed');
  expect(shown?.landings).toBeGreaterThanOrEqual(1);
  expect(shown?.airborneS ?? 0).toBeGreaterThan(60);
  expect(problems).toEqual([]);
});
