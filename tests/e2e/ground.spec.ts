/**
 * The runway start (GDD 48.2, ADR-35): the aircraft sits lined up on the runway under the parking
 * brake, takes off under power and the pilot's pull, and retracts its gear. Numbers come from the
 * test hook's aircraft status.
 */
import { expect, test } from '@playwright/test';
import { FLAG2, HEADERS_BASE, openGame, flightStatus, statusNow } from './support.ts';

const wow = (flags2: number): boolean => (flags2 & FLAG2.WOW) !== 0;

test('the aircraft starts on the runway, on its wheels, held by the parking brake', async ({
  page,
}) => {
  await openGame(page, `${HEADERS_BASE}?debug=1`);
  const s = await flightStatus(page);
  expect(wow(s.flags2)).toBe(true);
  expect(s.flags2 & FLAG2.PARKING_BRAKE).toBe(FLAG2.PARKING_BRAKE);
  expect(s.groundSpeed).toBeLessThan(0.1);
  expect(s.gearPos).toBeCloseTo(1, 2);
  expect(s.flapDetent).toBe(1);
  expect(s.crashCause).toBe(0);
  // The engine idles, and the lever is where the sim put it: nothing jumps when the pilot touches it.
  expect(s.throttle).toBeLessThan(0.1);
  // Five seconds on, still not moving.
  await page.waitForTimeout(5000);
  const later = await flightStatus(page);
  expect(later.groundSpeed).toBeLessThan(0.1);
  expect(wow(later.flags2)).toBe(true);
});

test('releasing the brake and going to full power accelerates it down the runway', async ({
  page,
}) => {
  await openGame(page, `${HEADERS_BASE}?debug=1`);
  await flightStatus(page);
  await page.keyboard.press('KeyP');
  await expect
    .poll(async () => ((await statusNow(page))?.flags2 ?? 0) & FLAG2.PARKING_BRAKE, {
      timeout: 5_000,
    })
    .toBe(0);
  await page.keyboard.down('ShiftLeft');
  await expect
    .poll(async () => (await statusNow(page))?.groundSpeed ?? 0, { timeout: 15_000 })
    .toBeGreaterThan(25);
  await page.keyboard.up('ShiftLeft');
});

test('it takes off, and L raises the gear', async ({ page }) => {
  test.setTimeout(120_000);
  await openGame(page, `${HEADERS_BASE}?debug=1`);
  await flightStatus(page);
  await page.keyboard.press('KeyP');
  await page.keyboard.down('ShiftLeft');
  await expect
    .poll(async () => (await statusNow(page))?.groundSpeed ?? 0, { timeout: 40_000 })
    .toBeGreaterThan(70);
  // Pull: the nose comes up and the wheels leave the runway.
  await page.keyboard.down('KeyS');
  await expect
    .poll(async () => wow((await statusNow(page))?.flags2 ?? FLAG2.WOW), { timeout: 15_000 })
    .toBe(false);
  await page.keyboard.up('KeyS');
  const airborne = await flightStatus(page);
  expect(airborne.crashCause).toBe(0);
  await expect
    .poll(async () => (await statusNow(page))?.agl ?? 0, { timeout: 10_000 })
    .toBeGreaterThan(5);
  await page.keyboard.press('KeyL');
  await expect
    .poll(async () => (await statusNow(page))?.gearPos ?? 1, { timeout: 7_000 })
    .toBeLessThan(0.02);
  await page.keyboard.up('ShiftLeft');
  expect((await flightStatus(page)).crashCause).toBe(0);
});

test('the gear will not come up while it is on the ground', async ({ page }) => {
  await openGame(page, `${HEADERS_BASE}?debug=1`);
  await flightStatus(page);
  await page.keyboard.press('KeyL');
  await page.waitForTimeout(2000);
  const s = await flightStatus(page);
  expect(s.gearPos).toBeCloseTo(1, 2);
  expect(wow(s.flags2)).toBe(true);
});
