import { expect, test, type Page } from '@playwright/test';
import { byId, HEADERS_BASE, openGame } from './support.ts';

/** On a machine with a real GPU the WebGPU specs must run; on CI runners there is none. */
const REQUIRE_WEBGPU = process.env['PW_REQUIRE_WEBGPU'] === '1';

async function hasWebGpuAdapter(page: Page): Promise<boolean> {
  await page.goto(`${HEADERS_BASE}?selftest=1`);
  return page.evaluate(async () => {
    try {
      return (await navigator.gpu.requestAdapter()) !== null;
    } catch {
      return false;
    }
  });
}

async function picture(page: Page) {
  const pixels = await page.evaluate(() => window.__flying?.readPixels());
  expect(pixels?.distinctColours ?? 0).toBeGreaterThanOrEqual(8);
}

test('forced WebGL2 draws the world', async ({ page }) => {
  await openGame(page, `${HEADERS_BASE}?debug=1&renderer=webgl2`);
  expect(await page.evaluate(() => window.__flying?.backend())).toBe('webgl2');
  await expect(byId(page, 'ov-renderer')).toContainText('webgl2');
  await picture(page);
});

test('forced WebGPU draws the world where there is an adapter, and is required to on real GPUs', async ({
  page,
}) => {
  const available = await hasWebGpuAdapter(page);
  if (REQUIRE_WEBGPU) {
    expect(available, 'PW_REQUIRE_WEBGPU=1 but this browser has no WebGPU adapter').toBe(true);
  } else {
    test.skip(!available, 'no WebGPU adapter here (CI runners have no GPU)');
  }
  await openGame(page, `${HEADERS_BASE}?debug=1&renderer=webgpu`);
  expect(await page.evaluate(() => window.__flying?.backend())).toBe('webgpu');
  await picture(page);
});

test('automatic choice falls back to WebGL2 when WebGPU cannot start', async ({ page }) => {
  await page.addInitScript(() => {
    Object.defineProperty(navigator, 'gpu', { value: undefined, configurable: true });
  });
  await openGame(page, `${HEADERS_BASE}?debug=1&renderer=auto`);
  expect(await page.evaluate(() => window.__flying?.backend())).toBe('webgl2');
  await picture(page);
});

test('the null renderer lets everything else run without drawing', async ({ page }) => {
  await openGame(page, `${HEADERS_BASE}?debug=1&renderer=null`);
  expect(await page.evaluate(() => window.__flying?.backend())).toBe('null');
  await expect(byId(page, 'ov-tick')).not.toHaveAttribute('data-value', '–');
});

test('the main-thread host works with each backend that exists', async ({ page }) => {
  await openGame(page, `${HEADERS_BASE}?debug=1&renderer=webgl2&flags=mainrender`);
  expect(await page.evaluate(() => window.__flying?.host())).toBe('main');
  expect(await page.evaluate(() => window.__flying?.backend())).toBe('webgl2');
  await picture(page);
});

test('the renderer contract passes on WebGL2 in this browser, and on WebGPU where it exists', async ({
  page,
}) => {
  test.setTimeout(90_000);
  await page.goto(`${HEADERS_BASE}?selftest=renderer`);
  await expect(byId(page, 'status')).toHaveText('done', { timeout: 60_000 });
  await expect(byId(page, 'contract-webgl2')).toHaveAttribute('data-result', 'pass');
  await expect(byId(page, 'contract-webgl2')).toContainText(/^pass \(\d+ checks\)$/);

  const webgpu = byId(page, 'contract-webgpu');
  if (REQUIRE_WEBGPU) {
    await expect(webgpu).toHaveAttribute('data-result', 'pass');
  } else {
    await expect(webgpu).toHaveAttribute('data-result', /^(pass|skipped)$/);
  }
});
