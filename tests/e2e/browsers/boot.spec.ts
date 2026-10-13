import { expect, test } from '@playwright/test';
import { byId, HEADERS_BASE, watchErrors } from '../support.ts';

/**
 * Firefox and WebKit (best effort, GDD 0.6.3): the page must boot, the compat check must say what
 * this engine has, and where it can run the game it must start. What each engine lacks is
 * information, not failure, so these look for sensible answers rather than for "ok".
 */
test('the compat screen renders every row with a verdict', async ({ page }) => {
  const problems: string[] = [];
  page.on('pageerror', (e) => problems.push(e.message));
  await page.goto(`${HEADERS_BASE}?compat=1`);
  await expect(byId(page, 'compat-screen')).toBeVisible({ timeout: 30_000 });
  await expect(page.locator('[data-testid^="compat-row-"]')).toHaveCount(9);
  await expect(byId(page, 'compat-verdict')).toHaveAttribute(
    'data-verdict',
    /^(ok|degraded|fatal)$/,
  );
  for (const id of ['wasm', 'simd', 'renderer']) {
    await expect(byId(page, `compat-state-${id}`)).toContainText(/OK|Reduced|Missing/);
  }
  expect(problems).toEqual([]);
});

test('where the engine can run the game, the game starts', async ({ page }) => {
  const problems = watchErrors(page);
  await page.goto(`${HEADERS_BASE}?debug=1`);
  const screen = byId(page, 'compat-screen');
  await expect(screen).toBeVisible({ timeout: 30_000 });
  const verdict = await byId(page, 'compat-verdict').getAttribute('data-verdict');
  test.skip(verdict === 'fatal', `this engine cannot run the game (${verdict ?? 'unknown'})`);
  await byId(page, 'compat-continue').click();
  await expect(byId(page, 'stage')).toHaveAttribute('data-state', 'running', { timeout: 60_000 });
  await expect(byId(page, 'ov-tick')).not.toHaveAttribute('data-value', '–', { timeout: 30_000 });
  // Console errors are reported, not required to be absent: engines word CSP and worker
  // diagnostics differently.
  if (problems.length > 0) console.log(`console errors in this engine:\n${problems.join('\n')}`);
});
