/**
 * The real aircraft in the browser (v0.1a): the Kestrel spawns trimmed, flies on its own, answers
 * the throttle and the trim, and crashes and respawns. Numbers come from the debug overlay. These
 * specs start in the air; `ground.spec.ts` starts on the runway.
 */
import { expect, test } from '@playwright/test';
import { byId, HEADERS_BASE, openGame, overlayNumber, overlaySurfaces } from './support.ts';

const value = overlayNumber;
const AIR = `${HEADERS_BASE}?debug=1&spawn=air`;
const status = (page: Parameters<typeof openGame>[0]) =>
  byId(page, 'ov-status').getAttribute('data-value');

test('the aircraft spawns trimmed in level flight', async ({ page }) => {
  await openGame(page, AIR);
  // The TAS row reads `200.1 m/s / 389 kn · M 0.600`.
  const mach = Number.parseFloat(
    /M (\S+)$/.exec((await byId(page, 'ov-tas').getAttribute('data-value')) ?? '')?.[1] ?? 'NaN',
  );
  expect(mach).toBeGreaterThan(0.55);
  expect(mach).toBeLessThan(0.65);
  const alpha = await value(page, 'ov-alpha');
  expect(alpha).toBeGreaterThan(1);
  expect(alpha).toBeLessThan(6);
  expect(await status(page)).toBe('ok');
});

test('hands off, it holds its height for ten seconds', async ({ page }) => {
  await openGame(page, AIR);
  const alt0 = await value(page, 'ov-alt');
  await page.waitForTimeout(10_000);
  expect(Math.abs((await value(page, 'ov-alt')) - alt0)).toBeLessThan(300);
  expect(await status(page)).toBe('ok');
});

test('full throttle lights all four afterburner stages', async ({ page }) => {
  await openGame(page, AIR);
  await page.keyboard.down('ShiftLeft');
  // The row reads `1.00 AB 4 · 101.5 kN`: the throttle, the stage, then the thrust.
  await expect(byId(page, 'ov-throttle')).toHaveAttribute('data-value', /^1\.00 AB 4 · /, {
    timeout: 5_000,
  });
  const thrust = async (): Promise<number> =>
    Number.parseFloat(
      /· (\S+) kN$/.exec((await byId(page, 'ov-throttle').getAttribute('data-value')) ?? '')?.[1] ??
        'NaN',
    );
  await expect.poll(thrust, { timeout: 3_000 }).toBeGreaterThan(90);
  await page.keyboard.up('ShiftLeft');
});

test('the trim keys move the elevator and it stays', async ({ page }) => {
  // Manual assist: the trim is the pilot's. The other assists trim themselves.
  await openGame(page, `${AIR}&assist=manual`);
  const [before] = await overlaySurfaces(page);
  await page.keyboard.down('BracketRight');
  await page.waitForTimeout(2000);
  await page.keyboard.up('BracketRight');
  await page.waitForTimeout(500);
  // Two degrees per second of nose-up trim: the trailing edge comes up about four degrees.
  const [after] = await overlaySurfaces(page);
  expect(before - after).toBeGreaterThan(3);
  expect(before - after).toBeLessThan(5);
});

test('flying into the ground crashes, and the aircraft respawns two seconds later', async ({
  page,
}) => {
  // The push takes about half a minute to reach the ground from the spawn height.
  test.setTimeout(90_000);
  // Manual assist: the stick drives the surfaces, so a held push goes into the ground. (Under the
  // other assists the limiters hold the nose up; the circuit covers those.)
  await openGame(page, `${AIR}&assist=manual`);
  await page.keyboard.down('KeyW');
  await page.keyboard.down('ShiftLeft');
  await expect.poll(() => status(page), { timeout: 60_000 }).toContain('crashed');
  await page.keyboard.up('KeyW');
  await page.keyboard.up('ShiftLeft');
  await expect.poll(() => status(page), { timeout: 5_000 }).toBe('ok');
  expect(Math.abs((await value(page, 'ov-tas')) - 200)).toBeLessThan(5);
});
