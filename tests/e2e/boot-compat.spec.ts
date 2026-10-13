import { expect, test, type Page } from '@playwright/test';
import { byId, HEADERS_BASE, openGame, watchErrors } from './support.ts';

const URL = `${HEADERS_BASE}?debug=1`;

/** Makes the page look like a browser with no WebGPU and no WebGL2, before any script runs. */
async function withoutGraphics(page: Page): Promise<void> {
  await page.addInitScript(() => {
    Object.defineProperty(navigator, 'gpu', { value: undefined, configurable: true });
    for (const proto of [HTMLCanvasElement.prototype, OffscreenCanvas.prototype]) {
      const original = Reflect.get(proto, 'getContext') as (
        this: unknown,
        type: string,
        ...rest: unknown[]
      ) => unknown;
      Object.defineProperty(proto, 'getContext', {
        configurable: true,
        value(this: unknown, type: string, ...rest: unknown[]) {
          return type === 'webgl2' || type === 'webgl' ? null : original.call(this, type, ...rest);
        },
      });
    }
  });
}

test('first run shows the compat screen, and Enter continues to the game', async ({ page }) => {
  const problems = watchErrors(page);
  await page.goto(URL);
  await expect(byId(page, 'compat-screen')).toBeVisible();
  // Everything this version needs is there, so the screen offers Continue and nothing is fatal.
  await expect(byId(page, 'compat-verdict')).not.toHaveAttribute('data-verdict', 'fatal');
  await expect(byId(page, 'compat-state-wasm')).toContainText('OK');
  await expect(byId(page, 'compat-state-simd')).toContainText('OK');
  for (const id of [
    'wasm',
    'simd',
    'renderer',
    'isolation',
    'threads',
    'offscreen',
    'sw',
    'opfs',
    'gamepad',
  ]) {
    await expect(byId(page, `compat-row-${id}`)).toBeVisible();
  }
  // Continue has focus, so the keyboard alone is enough.
  await expect(byId(page, 'compat-continue')).toBeFocused();
  await page.keyboard.press('Enter');
  await expect(byId(page, 'compat-screen')).toHaveCount(0);
  await expect(byId(page, 'render-canvas')).toBeVisible();
  expect(problems).toEqual([]);
});

test('the second run skips the screen; ?compat=1 brings it back', async ({ page }) => {
  await openGame(page, URL);
  await page.reload();
  await expect(byId(page, 'render-canvas')).toBeVisible({ timeout: 30_000 });
  await expect(byId(page, 'compat-screen')).toHaveCount(0);

  await page.goto(`${URL}&compat=1`);
  await expect(byId(page, 'compat-screen')).toBeVisible();
});

test('a changed feature signature brings the screen back', async ({ page }) => {
  await openGame(page, URL);
  await page.reload();
  await expect(byId(page, 'compat-screen')).toHaveCount(0);
  // The same browser, with Service Workers taken away: the acknowledged signature no longer fits.
  await page.addInitScript(() => {
    Object.defineProperty(navigator, 'serviceWorker', { value: undefined, configurable: true });
  });
  await page.reload();
  await expect(byId(page, 'compat-screen')).toBeVisible({ timeout: 30_000 });
  await expect(byId(page, 'compat-state-sw')).toContainText('Reduced');
});

test('no WebGPU and no WebGL2 is fatal: SYS-103, remedy text, no Continue', async ({ page }) => {
  await withoutGraphics(page);
  await page.goto(URL);
  await expect(byId(page, 'compat-screen')).toBeVisible();
  await expect(byId(page, 'compat-verdict')).toHaveAttribute('data-verdict', 'fatal');
  await expect(byId(page, 'compat-codes')).toHaveText('SYS-103');
  await expect(byId(page, 'compat-state-renderer')).toContainText('Missing');
  await expect(byId(page, 'compat-remedy-renderer')).toContainText('hardware acceleration');
  await expect(byId(page, 'compat-continue')).toHaveCount(0);
  await expect(byId(page, 'render-canvas')).toHaveCount(0);
});

test('asking for a renderer that is not there is fatal too', async ({ page }) => {
  await withoutGraphics(page);
  await page.goto(`${HEADERS_BASE}?renderer=webgpu`);
  await expect(byId(page, 'compat-verdict')).toHaveAttribute('data-verdict', 'fatal');
  await expect(byId(page, 'compat-codes')).toHaveText('SYS-103');
  await expect(byId(page, 'compat-row-renderer')).toContainText('renderer=webgpu');
});
