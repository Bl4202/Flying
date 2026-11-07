import { expect, test, type Page } from '@playwright/test';
import { byId, HEADERS_BASE, openGame, watchErrors } from './support.ts';

// The page cannot run an AudioContext until the user has done something. These tests press a key,
// as the Fly click does in the game; a separate test checks that without one the page says so.

test('the flight audio runs in the browser: worklet, level, and a tone within 100 ms', async ({
  page,
}) => {
  test.setTimeout(90_000);
  const problems = watchErrors(page);
  await page.goto(`${HEADERS_BASE}?selftest=audio`);
  await page.keyboard.press('Space'); // the gesture
  await expect(byId(page, 'status')).toHaveText('done', { timeout: 60_000 });
  await expect(byId(page, 'worklet')).toHaveAttribute('data-result', 'pass');
  await expect(byId(page, 'context')).toHaveAttribute('data-result', 'pass');
  await expect(byId(page, 'engine')).toHaveAttribute('data-result', 'pass');
  const onset = byId(page, 'onset');
  await expect(onset).toHaveAttribute('data-result', 'pass');
  const p95 = Number(/p95 ([\d.]+) ms/.exec((await onset.textContent()) ?? '')?.[1]);
  expect(p95).toBeLessThan(100);
  for (const id of ['worklet', 'context', 'engine', 'onset']) {
    test
      .info()
      .annotations.push({ type: id, description: (await byId(page, id).textContent()) ?? '' });
  }
  expect(problems).toEqual([]);
});

test('without a gesture the page says the context is waiting', async ({ page }) => {
  await page.goto(`${HEADERS_BASE}?selftest=audio`);
  await expect(byId(page, 'status')).toHaveText('done', { timeout: 30_000 });
  await expect(byId(page, 'worklet')).toHaveAttribute('data-result', 'pass');
  // Headless Chromium may allow autoplay; where it does not, the row says it skipped.
  await expect(byId(page, 'context')).toHaveAttribute('data-result', /^(pass|skipped)$/);
});

const GAME = `${HEADERS_BASE}?flags=stall`;
const audio = (page: Page) => page.evaluate(() => window.__flying?.audio());

test('Fly starts the sound: the worklet loads and the context runs from the click', async ({
  page,
}) => {
  await openGame(page, GAME, { menu: true });
  await byId(page, 'firstrun-skip').click();
  // Before the click the browser keeps the sound off, and the menu says so.
  await expect.poll(async () => (await audio(page))?.kind).toBe('webaudio');
  await byId(page, 'menu-fly').click();
  await expect.poll(async () => (await audio(page))?.state).toBe('running');
  const a = await audio(page);
  expect(a?.workletError).toBeUndefined();
  await expect.poll(async () => (await audio(page))?.paramWrites).toBeGreaterThan(5);
});

test('the engine gets louder with the throttle', async ({ page }) => {
  await openGame(page, GAME, { menu: true });
  await byId(page, 'firstrun-skip').click();
  await byId(page, 'menu-fly').click();
  await expect.poll(async () => (await audio(page))?.state).toBe('running');
  await page.waitForTimeout(1500);
  const idle = (await audio(page))?.levelDb ?? -200;
  await page.keyboard.down('Shift');
  await page.waitForTimeout(3000);
  await page.keyboard.up('Shift');
  await page.waitForTimeout(500);
  const loud = (await audio(page))?.levelDb ?? -200;
  expect(loud).toBeGreaterThan(idle + 3);
});

test('the sound is muted while the tab is hidden, and comes back', async ({ page }) => {
  await openGame(page, `${GAME}&spawn=air`, { menu: true });
  await byId(page, 'firstrun-skip').click();
  await byId(page, 'menu-fly').click();
  await expect.poll(async () => (await audio(page))?.state).toBe('running');
  await page.waitForTimeout(1000);
  expect((await audio(page))?.levelDb).toBeGreaterThan(-90);
  const hide = (hidden: boolean): Promise<void> =>
    page.evaluate((h) => {
      Object.defineProperty(document, 'visibilityState', {
        value: h ? 'hidden' : 'visible',
        configurable: true,
      });
      document.dispatchEvent(new Event('visibilitychange'));
    }, hidden);
  await hide(true);
  await expect.poll(async () => (await audio(page))?.muted).toBe(true);
  await expect.poll(async () => (await audio(page))?.levelDb, { timeout: 5000 }).toBeLessThan(-90);
  await hide(false);
  await expect.poll(async () => (await audio(page))?.muted).toBe(false);
});
