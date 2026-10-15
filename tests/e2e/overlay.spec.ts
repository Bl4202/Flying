import { readFileSync } from 'node:fs';
import { expect, test } from '@playwright/test';
import { byId, HEADERS_BASE, openGame } from './support.ts';

const STAGES = [
  'ingest',
  'environment',
  'relevance',
  'ai',
  'dynamics',
  'weapons',
  'sensing',
  'collision',
  'systems',
  'director',
  'publish',
];

test('Ctrl+Shift+D toggles the overlay', async ({ page }) => {
  await openGame(page, HEADERS_BASE);
  await expect(byId(page, 'overlay')).toHaveCount(0);
  await page.keyboard.press('Control+Shift+D');
  await expect(byId(page, 'overlay')).toBeVisible();
  await page.keyboard.press('Control+Shift+D');
  await expect(byId(page, 'overlay')).toBeHidden();
});

test('shows eleven numeric stage rows, and stays within the DOM budget', async ({ page }) => {
  await openGame(page, `${HEADERS_BASE}?debug=1`);
  for (const stage of STAGES) {
    await expect(byId(page, `ov-stage-${stage}`)).toHaveAttribute('data-value', /^\d+\.\d{3} ms$/, {
      timeout: 10_000,
    });
  }
  // AC-53.4: the page as a whole, text nodes included, stays at 150 nodes or fewer.
  const nodes = await page.evaluate(() => {
    const walker = document.createTreeWalker(document, NodeFilter.SHOW_ALL);
    let n = 0;
    while (walker.nextNode()) n += 1;
    return n;
  });
  expect(nodes).toBeLessThanOrEqual(150);
});

test('the Hash button prints sixteen-digit values for every column', async ({ page }) => {
  await openGame(page, `${HEADERS_BASE}?debug=1`);
  await byId(page, 'ov-hash').click();
  const out = byId(page, 'ov-hash-out');
  await expect(out).toContainText('combined');
  const text = await out.innerText();
  const columns = ['combined', 'tick', 'rng', 'bodies', 'ownship', 'input', 'content'];
  for (const name of columns) {
    expect(text).toMatch(new RegExp(`^${name}\\s+[0-9a-f]{16}$`, 'm'));
  }
});

test('the Self-test button reproduces the golden hash while the game runs', async ({ page }) => {
  await openGame(page, `${HEADERS_BASE}?debug=1`);
  await byId(page, 'ov-selftest').click();
  const out = byId(page, 'ov-selftest-out');
  await expect(out).toContainText('PASS', { timeout: 30_000 });
  const golden = JSON.parse(
    readFileSync(new URL('../golden/determinism.json', import.meta.url), 'utf8'),
  ) as { checkpoints: { tick: number; combined: string }[] };
  const expected = golden.checkpoints.find((c) => c.tick === 10_000)?.combined ?? 'missing';
  await expect(out).toContainText(expected);
});

test('the Pause button freezes the tick and Resume lets it run on', async ({ page }) => {
  await openGame(page, `${HEADERS_BASE}?debug=1`);
  await byId(page, 'ov-pause').click();
  await expect(byId(page, 'ov-pause')).toHaveText('Resume');
  await expect(byId(page, 'ov-core')).toHaveAttribute('data-value', 'paused');
  const frozen = await byId(page, 'ov-tick').getAttribute('data-value');
  await page.waitForTimeout(1000);
  expect(await byId(page, 'ov-tick').getAttribute('data-value')).toBe(frozen);
  await byId(page, 'ov-pause').click();
  await expect(byId(page, 'ov-core')).toHaveAttribute('data-value', 'running');
});
