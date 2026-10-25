/**
 * The virtual tester in the browser (ADR-33): `?script=circuit` has the scripted pilot fly the
 * take-off and circuit through the same input, flight control and gear path the player uses. The
 * exit gate flies the whole circuit natively under every assist (`cargo xtask circuit`); this
 * checks that the browser build flies the start of it and draws it.
 */
import { expect, test } from '@playwright/test';
import { ASSIST, byId, FLAG2, flightStatus, HEADERS_BASE, openGame, statusNow } from './support.ts';

test('the scripted pilot takes off, raises the gear and flies on, without a crash', async ({
  page,
}) => {
  test.setTimeout(180_000);
  await openGame(page, `${HEADERS_BASE}?debug=1&script=circuit`);
  const start = await flightStatus(page);
  expect(start.assist).toBe(ASSIST.STANDARD);
  expect((start.flags2 & FLAG2.WOW) !== 0).toBe(true);

  // Airborne, on its own, within half a minute.
  await expect
    .poll(async () => ((await statusNow(page))?.flags2 ?? FLAG2.WOW) & FLAG2.WOW, {
      timeout: 40_000,
    })
    .toBe(0);
  // Gear up within twenty seconds of lift-off.
  await expect
    .poll(async () => (await statusNow(page))?.gearPos ?? 1, { timeout: 20_000 })
    .toBeLessThan(0.02);

  // Ninety seconds into the run it is still flying: in the climb or the pattern, no crash, no
  // stall warning, the gear up.
  await expect
    .poll(async () => await page.evaluate(() => window.__flying?.stats().sim?.tick ?? 0), {
      timeout: 120_000,
      intervals: [500],
    })
    .toBeGreaterThanOrEqual(90 * 60);
  const later = await flightStatus(page);
  expect(later.crashCause).toBe(0);
  expect((later.flags2 & FLAG2.WOW) !== 0).toBe(false);
  expect(later.agl).toBeGreaterThan(100);
  expect(later.gearPos).toBeLessThan(0.02);
  expect(later.alpha).toBeLessThan(0.45);
  expect((await byId(page, 'ov-status').getAttribute('data-value')) ?? '').not.toContain('crashed');

  // And the picture is being drawn: the frame counter moves, and the view is not blank.
  const frames = await page.evaluate(() => window.__flying?.stats().render?.frames ?? 0);
  await page.waitForTimeout(500);
  expect(await page.evaluate(() => window.__flying?.stats().render?.frames ?? 0)).toBeGreaterThan(
    frames,
  );
  const pixels = await page.evaluate(() => window.__flying?.readPixels());
  expect(pixels?.distinctColours ?? 0).toBeGreaterThanOrEqual(8);
});
