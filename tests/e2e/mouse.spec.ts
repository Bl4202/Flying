/**
 * The mouse in the browser (GDD 58.4, ADR-37), with Pointer Lock stubbed: headless Chromium has no
 * pointer to capture, so the stub takes the lock on request and the specs send the motion as
 * pointer events with `movementX` and `movementY`, as a locked pointer does.
 */
import { expect, test, type Page } from '@playwright/test';
import { byId, FLAG2, flightStatus, HEADERS_BASE, openGame, statusNow } from './support.ts';

const AIR = `${HEADERS_BASE}?debug=1&spawn=air`;

async function stubPointerLock(page: Page): Promise<void> {
  await page.addInitScript(() => {
    let locked: Element | null = null;
    Object.defineProperty(document, 'pointerLockElement', {
      get: () => locked,
      configurable: true,
    });
    // The game captures the pointer on the stage, so that is what the stub locks.
    Element.prototype.requestPointerLock = () => {
      locked = document.querySelector('[data-testid="stage"]');
      document.dispatchEvent(new Event('pointerlockchange'));
      return Promise.resolve();
    };
    document.exitPointerLock = () => {
      locked = null;
      document.dispatchEvent(new Event('pointerlockchange'));
    };
  });
}

const isLocked = (page: Page): Promise<boolean> =>
  page.evaluate(() => document.pointerLockElement !== null);

/** Moves the locked mouse by `dx`, `dy` pixels a frame for `frames` frames. */
async function moveMouse(page: Page, dx: number, dy: number, frames: number): Promise<void> {
  for (let i = 0; i < frames; i++) {
    await page.evaluate(
      ([x, y]) => {
        const stage = document.querySelector('[data-testid="stage"]');
        stage?.dispatchEvent(
          new PointerEvent('pointermove', { movementX: x, movementY: y, bubbles: true }),
        );
      },
      [dx, dy] as const,
    );
    await page.waitForTimeout(16);
  }
}

const mouseMode = (page: Page): Promise<string | undefined> =>
  page.evaluate(() => window.__flying?.input()?.mouseMode);

/** Roll angle in degrees, from the overlay's `Pitch/roll` row (`3.0° / -12.0°`). */
async function rollDegrees(page: Page): Promise<number> {
  const text = (await byId(page, 'ov-pitch').getAttribute('data-value')) ?? '';
  return Number.parseFloat(/\/ (\S+)°$/.exec(text)?.[1] ?? 'NaN');
}

test('O turns the mouse to aim and takes the pointer; the nose follows where it is pointed', async ({
  page,
}) => {
  await stubPointerLock(page);
  await openGame(page, AIR);
  const start = await flightStatus(page);
  expect(await mouseMode(page)).toBe('off');
  expect(await isLocked(page)).toBe(false);
  await page.keyboard.press('KeyO');
  await expect.poll(() => mouseMode(page), { timeout: 2_000 }).toBe('aim');
  expect(await isLocked(page)).toBe(true);
  // Aim to the right, as far as the cone allows, and hold it there.
  await moveMouse(page, 25, 0, 60);
  await expect
    .poll(async () => ((await statusNow(page))?.flags2 ?? 0) & FLAG2.MOUSE_AIM, {
      timeout: 2_000,
    })
    .toBe(FLAG2.MOUSE_AIM);
  // The heading comes round to the right by more than ten degrees within five seconds.
  await expect
    .poll(
      async () => {
        const s = await statusNow(page);
        if (s === undefined) return 0;
        let turned = s.noseAz - start.noseAz;
        if (turned > Math.PI) turned -= 2 * Math.PI;
        if (turned < -Math.PI) turned += 2 * Math.PI;
        return (turned * 180) / Math.PI;
      },
      { timeout: 5_000 },
    )
    .toBeGreaterThan(10);
});

test('O again: the mouse is a stick, and moving it rolls the aircraft', async ({ page }) => {
  await stubPointerLock(page);
  await openGame(page, AIR);
  await flightStatus(page);
  await page.keyboard.press('KeyO');
  await page.keyboard.press('KeyO');
  await expect.poll(() => mouseMode(page), { timeout: 2_000 }).toBe('stick');
  expect(await isLocked(page)).toBe(true);
  expect(Math.abs(await rollDegrees(page))).toBeLessThan(5);
  await moveMouse(page, 20, 0, 20);
  await expect.poll(() => rollDegrees(page), { timeout: 5_000 }).toBeGreaterThan(10);
  // No aim marker in this mode.
  expect(((await flightStatus(page)).flags2 & FLAG2.MOUSE_AIM) !== 0).toBe(false);
});

test('O a third time: the mouse is off and the pointer is given back', async ({ page }) => {
  await stubPointerLock(page);
  await openGame(page, AIR);
  await flightStatus(page);
  for (let i = 0; i < 3; i++) await page.keyboard.press('KeyO');
  await expect.poll(() => mouseMode(page), { timeout: 2_000 }).toBe('off');
  await expect.poll(() => isLocked(page), { timeout: 2_000 }).toBe(false);
  // With the pointer given back the pause menu does not open.
  expect(await page.evaluate(() => window.__flying?.menu())).toBeNull();
});

test('losing the pointer opens the pause menu and holds the sim; Resume takes it again', async ({
  page,
}) => {
  await stubPointerLock(page);
  await openGame(page, AIR);
  await flightStatus(page);
  await page.keyboard.press('KeyO');
  await expect.poll(() => isLocked(page), { timeout: 2_000 }).toBe(true);
  // What the browser does when the player presses Esc under Pointer Lock.
  await page.evaluate(() => {
    document.exitPointerLock();
  });
  await expect(byId(page, 'pause-menu')).toBeVisible();
  await expect(byId(page, 'ov-core')).toHaveAttribute('data-value', 'paused');
  await byId(page, 'pause-resume').click();
  await expect(byId(page, 'pause-menu')).toHaveCount(0);
  await expect.poll(() => isLocked(page), { timeout: 2_000 }).toBe(true);
  await expect(byId(page, 'ov-core')).toHaveAttribute('data-value', 'running');
});

test('the mouse setting from the menu is kept for the next visit', async ({ page }) => {
  await stubPointerLock(page);
  await openGame(page, AIR);
  await flightStatus(page);
  await page.keyboard.press('Escape');
  await byId(page, 'pause-mouse-stick').click();
  await byId(page, 'pause-resume').click();
  await expect.poll(() => mouseMode(page), { timeout: 2_000 }).toBe('stick');
  await page.waitForTimeout(300);
  await openGame(page, AIR);
  await expect.poll(() => mouseMode(page), { timeout: 5_000 }).toBe('stick');
});
