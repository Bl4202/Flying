import { expect, test } from '@playwright/test';
import {
  byId,
  HEADERS_BASE,
  openGame,
  overlayNumber,
  PAGES_BASE,
  sampleAfter,
  simRate,
} from './support.ts';

async function windowOfPlay(page: Parameters<typeof openGame>[0]) {
  await page.waitForTimeout(1000);
  const a = await sampleAfter(page, 300);
  const b = await sampleAfter(page, 2500);
  return { a, b };
}

test.describe('no isolation: no headers and no Service Worker', () => {
  test.use({ serviceWorkers: 'block' });

  test('runs the single build over copied messages, and the loop and input still work', async ({
    page,
  }) => {
    await openGame(page, `${PAGES_BASE}?debug=1`);
    // Transfer mode: the single WASM build, snapshots and inputs by message, timer-driven ticks.
    await expect(byId(page, 'ov-mode')).toHaveAttribute('data-value', 'single · message · timer');

    const { a, b } = await windowOfPlay(page);
    // The fixed-step clock corrects the coarse timers: the rate holds, the jitter is what suffers.
    const rate = simRate(a, b);
    expect(rate).toBeGreaterThan(58);
    expect(rate).toBeLessThan(62);
    expect(b.render.frames - a.render.frames).toBeGreaterThan(60);

    // Input still reaches the core, through the 30 Hz message path: the spawn is near level, and
    // pushing the stick lowers the nose.
    const pitch0 = await overlayNumber(page, 'ov-pitch');
    expect(Math.abs(pitch0)).toBeLessThan(5);
    await page.keyboard.down('KeyW');
    await expect
      .poll(() => overlayNumber(page, 'ov-pitch'), { timeout: 10_000 })
      .toBeLessThan(pitch0 - 3);
    await page.keyboard.up('KeyW');
  });

  test('the compat screen says what is reduced and why', async ({ page }) => {
    await page.goto(`${PAGES_BASE}?compat=1`);
    // With the Service Worker blocked the page first waits out its attempt to isolate itself.
    await expect(byId(page, 'compat-screen')).toBeVisible({ timeout: 30_000 });
    await expect(byId(page, 'compat-state-isolation')).toContainText('Reduced');
    await expect(byId(page, 'compat-state-threads')).toContainText('Reduced');
    await expect(byId(page, 'compat-remedy-isolation')).toBeVisible();
    await expect(byId(page, 'compat-modes')).toContainText('single-threaded WebAssembly');
    await expect(byId(page, 'compat-modes')).toContainText('copied messages');
    await expect(byId(page, 'compat-modes')).toContainText('browser timers');
  });
});

test.describe('isolated, with the degraded paths forced on', () => {
  test('copied messages and timer ticks work with the threads build too', async ({ page }) => {
    await openGame(page, `${HEADERS_BASE}?debug=1&flags=transfer,timer`);
    await expect(byId(page, 'ov-mode')).toHaveAttribute('data-value', 'threads · message · timer');
    const { a, b } = await windowOfPlay(page);
    const rate = simRate(a, b);
    expect(rate).toBeGreaterThan(58);
    expect(rate).toBeLessThan(62);
    expect(b.render.frames - a.render.frames).toBeGreaterThan(60);
  });

  test('the main-thread renderer draws, and the sim keeps its rate', async ({ page }) => {
    await openGame(page, `${HEADERS_BASE}?debug=1&flags=mainrender`);
    expect(await page.evaluate(() => window.__flying?.host())).toBe('main');
    const { a, b } = await windowOfPlay(page);
    const rate = simRate(a, b);
    expect(rate).toBeGreaterThan(59);
    expect(rate).toBeLessThan(61);
    expect(b.render.frames - a.render.frames).toBeGreaterThan(60);
    const pixels = await page.evaluate(() => window.__flying?.readPixels());
    expect(pixels?.distinctColours ?? 0).toBeGreaterThanOrEqual(8);
  });
});
