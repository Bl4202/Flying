/**
 * The duel in the browser, from the merge (the bandit starts 3 km ahead on the same heading, a
 * Recruit that never fires): the HUD's target and gunsight follow the core's cues (HU5's live half;
 * the pixels are the renderer contract's), the bandit and the tracers are drawn, the weapon keys
 * fire (CT3), and the duel's one-shot sounds start on time (AU3, on the audio self-test page). A
 * missile in flight is drawn by the scene's tests: hands off the stick no lock comes at the merge
 * (the Recruit turns past the seeker's 35° aspect before it is inside its 2°).
 */
import { expect, test, type Page } from '@playwright/test';
import { byId, CHEAP, COUNTER, HEADERS_BASE, openGame, watchErrors } from './support.ts';

const MERGE = `${HEADERS_BASE}?debug=1&spawn=air&scenario=merge&skill=1&${CHEAP}`;

/** `COMBAT_FLAG` and `LAUNCH_INHIBIT` (`sim-bridge`). */
const FLAG = { ACTIVE: 1, TARGET: 16, PIPPER: 32 } as const;
const NO_LOCK = 3;
/** The gunsight ranges a target this close (`GUN_RANGING_M`, ADR-48). */
const RANGING_M = 2500;

/** The duel's status now, with its counters as a plain array. */
function duel(page: Page) {
  return page.evaluate(() => {
    const c = window.__flying?.combat();
    return c === undefined
      ? undefined
      : {
          flags: c.flags,
          seeker: c.seeker,
          inhibit: c.inhibit,
          rounds: c.rounds,
          missiles: c.missiles,
          range: c.tgtRange,
          life: c.banditLife,
          counters: Array.from(c.counters),
        };
  });
}

/** The frames drawn so far with the bandit, a missile in flight and a tracer. */
const drawn = (page: Page) =>
  page.evaluate(
    () => window.__flying?.stats().render?.drawnFrames ?? { bandit: 0, missiles: 0, tracers: 0 },
  );

/** Opens the merge and waits for the living bandit and a second of flight. */
async function merge(page: Page): Promise<void> {
  await openGame(page, MERGE);
  await expect.poll(async () => (await duel(page))?.life ?? 0, { timeout: 15_000 }).toBe(1);
  await page.waitForTimeout(1000);
}

test('the merge puts a living bandit ahead, targeted, and the gunsight ranges it inside 2.5 km (HU5)', async ({
  page,
}) => {
  test.setTimeout(90_000);
  const problems = watchErrors(page);
  await merge(page);
  const first = await duel(page);
  expect((first?.flags ?? 0) & FLAG.ACTIVE).toBe(FLAG.ACTIVE);
  expect((first?.flags ?? 0) & FLAG.TARGET).toBe(FLAG.TARGET);
  expect(first?.range ?? 0).toBeGreaterThan(RANGING_M);
  // The range closes, and inside the gunsight's ranging with the bandit on the nose it has a
  // solution: the reticle draws on that flag (its pixels are the renderer contract's HU5 case).
  // Hands off the stick, the bandit soon leaves the nose, so the flag is caught while it lasts.
  await expect
    .poll(
      async () => {
        const c = await duel(page);
        return (
          c !== undefined && c.range > 0 && c.range < RANGING_M && (c.flags & FLAG.PIPPER) !== 0
        );
      },
      { timeout: 45_000, intervals: [200] },
    )
    .toBe(true);
  // And the bandit is drawn (its second model; the render reports come twice a second).
  await expect.poll(async () => (await drawn(page)).bandit, { timeout: 5_000 }).toBeGreaterThan(0);
  expect(problems).toEqual([]);
});

test('Space fires the gun (the rounds fall, the tracers draw), and M reaches the launch (CT3)', async ({
  page,
}) => {
  test.setTimeout(90_000);
  const problems = watchErrors(page);
  await merge(page);
  const before = (await duel(page))?.rounds ?? 0;
  expect(before).toBeGreaterThan(0);
  await page.keyboard.down('Space');
  await expect
    .poll(async () => (await duel(page))?.rounds ?? before, { timeout: 5_000 })
    .toBeLessThan(before);
  // The burst's tracers are drawn while it flies.
  await expect.poll(async () => (await drawn(page)).tracers, { timeout: 5_000 }).toBeGreaterThan(0);
  await page.keyboard.up('Space');
  expect((await duel(page))?.counters[COUNTER.ROUNDS_FIRED] ?? 0).toBeGreaterThan(0);

  // M: a missile leaves the rail at a lock, and without one the core refuses it as NO LOCK and
  // says so for 1.5 s. Hands off the stick the seeker does not hold the Recruit, so it refuses; the
  // launch at a lock is the bot's (dogfight.spec.ts) and the core's own tests.
  const missiles = (await duel(page))?.missiles ?? 0;
  await page.keyboard.press('KeyM');
  await expect
    .poll(
      async () => {
        const c = await duel(page);
        if (c === undefined) return 'none';
        if ((c.counters[COUNTER.MISSILES_FIRED] ?? 0) > 0) return 'launched';
        return c.inhibit === NO_LOCK ? 'refused' : 'nothing yet';
      },
      { timeout: 5_000 },
    )
    .toMatch(/^(launched|refused)$/);
  const after = await duel(page);
  if ((after?.counters[COUNTER.MISSILES_FIRED] ?? 0) > 0)
    expect(after?.missiles).toBe(missiles - 1);
  else expect(after?.missiles).toBe(missiles);
  expect(problems).toEqual([]);
});

test("the duel's one-shot sounds start within 100 ms of the counter that rose (AU3)", async ({
  page,
}) => {
  test.setTimeout(150_000);
  await page.goto(`${HEADERS_BASE}?selftest=audio`);
  await page.keyboard.press('Space'); // the gesture that lets the audio context run
  await expect(byId(page, 'status')).toHaveText('done', { timeout: 120_000 });
  for (const id of ['onset-launch', 'onset-hit', 'onset-explosion']) {
    const row = byId(page, id);
    test.info().annotations.push({ type: id, description: (await row.textContent()) ?? '' });
    await expect(row, (await row.textContent()) ?? id).toHaveAttribute('data-result', 'pass');
  }
});
