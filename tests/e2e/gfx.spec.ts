/**
 * G-physiology on the screen (GDD 11.14, 53.4, ADR-38), through the whole chain: the sim's
 * physiology, the snapshot, the screen pass. A push over gives red-out; a hard pull from the dive
 * greys the picture out, then blacks it out, then G-LOC takes the controls. The aircraft cannot
 * hold nine g from level flight (about seven is all it has at the spawn speed), so the pull
 * starts from a dive that has built up speed.
 */
import { expect, test, type Page } from '@playwright/test';
import { byId, FLAG2, flightStatus, HEADERS_BASE, openGame, statusNow } from './support.ts';

const AIR = `${HEADERS_BASE}?debug=1&spawn=air`;

const luma = async (page: Page): Promise<number> =>
  (await page.evaluate(() => window.__flying?.readPixels()))?.meanLuma ?? Number.NaN;
const flags2 = async (page: Page): Promise<number> => (await statusNow(page))?.flags2 ?? 0;

/**
 * Full power and the stick forward until the dive has gained speed (or the height is 1 300 m
 * above the ground), then the stick back. Leaves the keys held.
 */
async function pushThenPull(page: Page): Promise<void> {
  await page.keyboard.down('ShiftLeft');
  await page.keyboard.down('KeyW');
  await expect
    .poll(
      async () => {
        const s = await statusNow(page);
        return s !== undefined && (s.tas > 270 || s.agl < 1300);
      },
      { timeout: 30_000, intervals: [100] },
    )
    .toBe(true);
  await page.keyboard.up('KeyW');
  await page.keyboard.down('KeyS');
}

async function release(page: Page): Promise<void> {
  await page.keyboard.up('KeyS');
  await page.keyboard.up('ShiftLeft');
}

test('a hard pull from a dive greys the screen out, blacks it out, and G-LOC takes the controls', async ({
  page,
}) => {
  test.setTimeout(120_000);
  await openGame(page, AIR);
  await flightStatus(page);
  expect(await luma(page)).toBeGreaterThan(0.2);

  await pushThenPull(page);

  // Blackout: the sim flags it and the screen is nearly black.
  await expect
    .poll(async () => (await flags2(page)) & FLAG2.BLACKOUT, { timeout: 40_000 })
    .toBe(FLAG2.BLACKOUT);
  await expect.poll(() => luma(page), { timeout: 5_000 }).toBeLessThan(0.1);

  // G-LOC: the controls are taken. The stick is still back, but the load factor falls to one g.
  await expect
    .poll(async () => (await flags2(page)) & FLAG2.GLOC, { timeout: 30_000 })
    .toBe(FLAG2.GLOC);
  await expect
    .poll(async () => Math.abs((await statusNow(page))?.nz ?? 99), { timeout: 5_000 })
    .toBeLessThan(3);
  await release(page);
});

test('a push over turns the screen red and then dark', async ({ page }) => {
  await openGame(page, AIR);
  await flightStatus(page);
  await page.keyboard.down('ShiftLeft');
  await page.keyboard.down('KeyW');
  await expect
    .poll(async () => (await flags2(page)) & FLAG2.REDOUT, { timeout: 15_000 })
    .toBe(FLAG2.REDOUT);
  await expect
    .poll(async () => (await statusNow(page))?.redout ?? 0, { timeout: 10_000 })
    .toBeGreaterThan(0.9);
  await expect.poll(() => luma(page), { timeout: 5_000 }).toBeLessThan(0.1);
  await page.keyboard.up('KeyW');
  await page.keyboard.up('ShiftLeft');
});

/** Watches the flags and the picture for `ms`, and reports whether anything was ever dark. */
async function watch(page: Page, ms: number) {
  const seen = { effects: 0, darkest: 1, crash: 0 };
  const end = Date.now() + ms;
  while (Date.now() < end) {
    seen.effects |= (await flags2(page)) & (FLAG2.REDOUT | FLAG2.BLACKOUT | FLAG2.GLOC);
    seen.darkest = Math.min(seen.darkest, await luma(page));
    seen.crash = Math.max(seen.crash, (await statusNow(page))?.crashCause ?? 0);
    await page.waitForTimeout(250);
  }
  return seen;
}

test('with the g-effects dial off, the same pull does nothing to the pilot or the screen', async ({
  page,
}) => {
  test.setTimeout(120_000);
  await openGame(page, `${AIR}&g=off`);
  await flightStatus(page);
  await pushThenPull(page);
  // Long enough that the full-effects pull above is blacked out well before the end.
  const seen = await watch(page, 20_000);
  expect(seen.crash).toBe(0);
  expect(seen.effects).toBe(0);
  expect(seen.darkest).toBeGreaterThan(0.15);
  await release(page);
});

test('with the screen effect at 0 %, the pilot still blacks out but the picture stays', async ({
  page,
}) => {
  test.setTimeout(120_000);
  await openGame(page, AIR);
  await flightStatus(page);
  await page.keyboard.press('Escape');
  await byId(page, 'pause-strength').evaluate((el) => {
    const input = el as HTMLInputElement;
    input.value = '0';
    input.dispatchEvent(new Event('input', { bubbles: true }));
  });
  await byId(page, 'pause-resume').click();
  await pushThenPull(page);
  await expect
    .poll(async () => (await flags2(page)) & FLAG2.BLACKOUT, { timeout: 40_000 })
    .toBe(FLAG2.BLACKOUT);
  await page.waitForTimeout(500);
  expect(await luma(page)).toBeGreaterThan(0.15);
  await release(page);
});
