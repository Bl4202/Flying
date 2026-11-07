import { expect, test, type Page } from '@playwright/test';
import { byId, continuePastCompat, HEADERS_BASE, openGame, waitForStored } from './support.ts';

const PAGE = `${HEADERS_BASE}?flags=stall`;

async function waitForShellCached(page: Page): Promise<void> {
  await expect
    .poll(
      () =>
        page.evaluate(async () => {
          if (navigator.serviceWorker.controller === null) return false;
          const names = (await caches.keys()).filter((n) => n.startsWith('fg-'));
          const first = names[0];
          return first !== undefined && (await (await caches.open(first)).keys()).length > 5;
        }),
      { timeout: 30_000 },
    )
    .toBe(true);
}

test('after one visit the game starts and flies with no network (F5)', async ({
  page,
  context,
}) => {
  test.setTimeout(90_000);
  // One online visit: the first run is skipped, so the next launch opens the menu.
  await openGame(page, PAGE, { menu: true });
  await byId(page, 'firstrun-skip').click();
  await expect(byId(page, 'main-menu')).toBeVisible();
  await waitForStored(page, (s) => s['setupDone'] === true);
  await waitForShellCached(page);

  await context.setOffline(true);
  await page.reload();
  await continuePastCompat(page);
  await expect(byId(page, 'main-menu')).toBeVisible({ timeout: 30_000 });

  // Fly: the simulation ticks and frames are drawn, all from the cache.
  await byId(page, 'menu-fly').click();
  await expect
    .poll(async () => (await page.evaluate(() => window.__flying?.status()))?.assist ?? 0)
    .toBeGreaterThan(0);
  const stats = async (): Promise<{ tick: number; frames: number }> =>
    page.evaluate(() => ({
      tick: window.__flying?.stats().sim?.tick ?? 0,
      frames: window.__flying?.stats().render?.frames ?? 0,
    }));
  const first = await stats();
  await expect.poll(async () => (await stats()).tick).toBeGreaterThan(first.tick + 30);
  expect((await stats()).frames).toBeGreaterThan(first.frames);

  // The audio worklet is part of the build and comes from the cache too.
  const worklet = await page.evaluate(async () => {
    const manifest = (await (await fetch('build-manifest.json')).json()) as { files: string[] };
    const file = manifest.files.find((f) => f.includes('flight-processor'));
    if (file === undefined) return { file: undefined, ok: false };
    const response = await fetch(file);
    return { file, ok: response.ok && (await response.text()).includes('registerProcessor') };
  });
  expect(worklet.file).toBeDefined();
  expect(worklet.ok).toBe(true);
});

test('the navigation is the cached shell, and the network is not asked for the page itself', async ({
  page,
  context,
}) => {
  await openGame(page, PAGE, { menu: true });
  await waitForShellCached(page);
  await context.setOffline(true);
  const response = await page.reload();
  expect(response?.ok()).toBe(true);
  expect(response?.fromServiceWorker()).toBe(true);
});
