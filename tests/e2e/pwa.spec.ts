import { expect, test, type Page } from '@playwright/test';
import {
  byId,
  continuePastCompat,
  HEADERS_BASE,
  openGame,
  waitForStored,
  watchErrors,
} from './support.ts';

const PAGE = `${HEADERS_BASE}?flags=stall`;

interface Manifest {
  id: string;
  name: string;
  short_name: string;
  start_url: string;
  scope: string;
  display: string;
  theme_color: string;
  background_color: string;
  icons: { src: string; sizes: string; type: string; purpose: string }[];
}

/** Width and height from a PNG's header. */
function pngSize(bytes: Buffer): [number, number] {
  expect(bytes.subarray(0, 8).toString('hex')).toBe('89504e470d0a1a0a');
  return [bytes.readUInt32BE(16), bytes.readUInt32BE(20)];
}

/** Waits until a Service Worker controls the page and has filled its cache. */
async function waitForShellCached(page: Page): Promise<void> {
  await expect
    .poll(
      () =>
        page.evaluate(async () => {
          if (navigator.serviceWorker.controller === null) return false;
          const names = (await caches.keys()).filter((n) => n.startsWith('fg-'));
          const first = names[0];
          if (first === undefined) return false;
          return (await (await caches.open(first)).keys()).length > 5;
        }),
      { timeout: 30_000 },
    )
    .toBe(true);
}

test('the page links a manifest that names the app and three icons, each fetchable at its declared size', async ({
  page,
}) => {
  await openGame(page, PAGE, { menu: true });
  const href = await page.evaluate(
    () => document.querySelector<HTMLLinkElement>('link[rel="manifest"]')?.href ?? '',
  );
  expect(href).toBe(`${HEADERS_BASE}manifest.webmanifest`);
  const response = await page.request.get(href);
  expect(response.status()).toBe(200);
  const manifest = (await response.json()) as Manifest;
  expect(manifest).toMatchObject({
    id: '/Flying/',
    name: 'Flying',
    start_url: '/Flying/',
    scope: '/Flying/',
    display: 'standalone',
  });
  expect(manifest.theme_color).toMatch(/^#[0-9a-f]{6}$/);
  expect(manifest.icons.map((i) => `${i.sizes} ${i.purpose}`)).toEqual([
    '192x192 any',
    '512x512 any',
    '512x512 maskable',
  ]);
  for (const icon of manifest.icons) {
    const r = await page.request.get(new URL(icon.src, href).href);
    expect(r.status(), icon.src).toBe(200);
    expect(icon.type).toBe('image/png');
    const [w, h] = pngSize(await r.body());
    expect(`${String(w)}x${String(h)}`).toBe(icon.sizes);
    // Content-addressed, so a changed icon is a new file.
    expect(icon.src).toMatch(/-[A-Za-z0-9_-]{8,}\.png$/);
  }
  // The head is written for the DOM budget: the tags are there and no whitespace sits between them.
  const head = await page.evaluate(() => ({
    theme: document.querySelector('meta[name="theme-color"]')?.getAttribute('content'),
    icon: document.querySelector('link[rel="icon"]')?.getAttribute('type'),
    texts: [...document.head.childNodes].filter((n) => n.nodeType === Node.TEXT_NODE).length,
  }));
  expect(head).toEqual({ theme: manifest.theme_color, icon: 'image/svg+xml', texts: 0 });
});

test('a Service Worker with a fetch handler controls the page', async ({ page }) => {
  const problems = watchErrors(page);
  await openGame(page, PAGE, { menu: true });
  await waitForShellCached(page);
  const worker = await page.evaluate(async () => {
    const reg = await navigator.serviceWorker.getRegistration();
    const res = await fetch(reg?.active?.scriptURL ?? '');
    const source = await res.text();
    return {
      controlled: navigator.serviceWorker.controller !== null,
      scope: reg?.scope,
      handlesFetch: source.includes('"fetch"') || source.includes("'fetch'"),
    };
  });
  expect(worker).toEqual({ controlled: true, scope: HEADERS_BASE, handlesFetch: true });
  expect(problems).toEqual([]);
});

test("a page of the build in control is never given another build's shell, and no cache is written to on a visit", async ({
  page,
}) => {
  await openGame(page, PAGE, { menu: true });
  await byId(page, 'firstrun-skip').click();
  await waitForStored(page, (s) => s['setupDone'] === true);
  await waitForShellCached(page);
  const before = await page.evaluate(async () => {
    const out: Record<string, number> = {};
    for (const n of await caches.keys()) out[n] = (await (await caches.open(n)).keys()).length;
    return out;
  });
  await page.reload();
  await continuePastCompat(page);
  await expect(byId(page, 'main-menu')).toBeVisible();
  const after = await page.evaluate(async () => {
    const out: Record<string, number> = {};
    for (const n of await caches.keys()) out[n] = (await (await caches.open(n)).keys()).length;
    return out;
  });
  // Navigations are answered from the cache and never put into it (the v0.1b bug).
  expect(after).toEqual(before);
});

test("the browser's install offer reaches the main menu, and Install asks the browser", async ({
  page,
}) => {
  await openGame(page, PAGE, { menu: true });
  await page.keyboard.press('Tab'); // let the page settle before the synthetic event
  await expect(byId(page, 'first-run')).toBeVisible();
  await byId(page, 'firstrun-skip').click();
  await expect(byId(page, 'main-menu')).toBeVisible();
  await expect(byId(page, 'menu-install-row')).toBeHidden();

  // A synthetic `beforeinstallprompt`, as Chromium fires when the page qualifies.
  await page.evaluate(() => {
    const e = new Event('beforeinstallprompt', { cancelable: true });
    Object.assign(e, {
      prompt: () => {
        (window as unknown as { __prompted: number }).__prompted =
          ((window as unknown as { __prompted?: number }).__prompted ?? 0) + 1;
        return Promise.resolve();
      },
      userChoice: Promise.resolve({ outcome: 'accepted' }),
    });
    window.dispatchEvent(e);
  });
  await expect(byId(page, 'menu-install-row')).toBeVisible();
  await byId(page, 'menu-install').click();
  expect(await page.evaluate(() => (window as unknown as { __prompted?: number }).__prompted)).toBe(
    1,
  );
  // The offer is spent; the row goes.
  await expect(byId(page, 'menu-install-row')).toBeHidden();

  // Once installed, it never comes back.
  await page.evaluate(() => {
    const e = new Event('beforeinstallprompt', { cancelable: true });
    Object.assign(e, {
      prompt: () => Promise.resolve(),
      userChoice: Promise.resolve({ outcome: 'dismissed' }),
    });
    window.dispatchEvent(e);
    window.dispatchEvent(new Event('appinstalled'));
  });
  await expect(byId(page, 'menu-install-row')).toBeHidden();
});
