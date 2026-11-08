import { expect, test } from '@playwright/test';
import { FLAG2, HEADERS_BASE, openGame, statusNow } from './support.ts';

/**
 * F7 (AC-53.1's precursor): with the build in the browser's cache, opening the game and starting the
 * circuit gets the aircraft off the ground in under a minute. The scripted pilot stands in for the
 * player at the menu: `?script=circuit` is a sortie, so it goes straight to the flight.
 */
test('F7: from a cached cold start the circuit pilot has the wheels up within 60 s', async ({
  page,
}) => {
  test.setTimeout(150_000);
  const url = `${HEADERS_BASE}?flags=stall&script=circuit`;
  // A first visit puts the build into the Service Worker's cache.
  await openGame(page, url);
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

  // The cold start: the page loads again from the cache, and the clock starts at the navigation.
  const started = Date.now();
  await page.goto(url);
  await expect
    .poll(async () => ((await statusNow(page))?.flags2 ?? FLAG2.WOW) & FLAG2.WOW, {
      timeout: 60_000,
      intervals: [250],
    })
    .toBe(0);
  const seconds = (Date.now() - started) / 1000;
  test.info().annotations.push({ type: 'wheels up after', description: `${seconds.toFixed(1)} s` });
  expect(seconds).toBeLessThan(60);
});
