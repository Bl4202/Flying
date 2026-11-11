/**
 * The statistics screen (ADR-45 ST5): it shows the totals; an export imported into a cleared store
 * gives the same file back, byte for byte; Clear empties them.
 */
import { readFileSync } from 'node:fs';
import { expect, test, type Page } from '@playwright/test';
import { byId, CHEAP, HEADERS_BASE, openGame, watchErrors } from './support.ts';

const MERGE = `${HEADERS_BASE}?debug=1&spawn=air&scenario=merge&skill=1&${CHEAP}`;

/** Exports the statistics and returns the file's text. */
async function exported(page: Page): Promise<string> {
  const download = page.waitForEvent('download');
  await byId(page, 'stats-export').click();
  const file = await download;
  expect(file.suggestedFilename()).toBe('flying-stats.json');
  return readFileSync(await file.path(), 'utf8');
}

const total = (page: Page, key: string) => byId(page, `stats-total-${key}`).locator('dd');

test('Statistics shows the totals; export and import round-trip byte for byte; Clear empties them (ST5)', async ({
  page,
}) => {
  test.setTimeout(120_000);
  const problems = watchErrors(page);
  await openGame(page, MERGE);
  await expect
    .poll(() => page.evaluate(() => window.__flying?.combat()?.banditLife ?? 0), {
      timeout: 15_000,
    })
    .toBe(1);
  // A second of flight first: a key held from the very first frame is not taken as a press.
  await page.waitForTimeout(1000);
  await page.keyboard.down('Space');
  await page.waitForTimeout(600);
  await page.keyboard.up('Space');
  // End the sortie, and leave its debrief for the main menu.
  await page.keyboard.press('Escape');
  await byId(page, 'pause-end-sortie').click();
  await byId(page, 'debrief-menu').click();
  await byId(page, 'menu-statistics').click();

  await expect(total(page, 'missions')).toHaveText('1');
  const rounds = Number(await total(page, 'gun_rounds').textContent());
  expect(rounds).toBeGreaterThan(0);
  await expect(byId(page, 'stats-log-0')).toContainText('Dogfight');

  const text = await exported(page);
  expect(JSON.parse(text)).toMatchObject({ schema: 'stats_export/1' });

  // Clear asks twice, then empties everything.
  await byId(page, 'stats-clear').click();
  await expect(total(page, 'missions')).toHaveText('1');
  await byId(page, 'stats-clear').click();
  await expect(total(page, 'missions')).toHaveText('0');
  await expect(byId(page, 'stats-log-empty')).toBeVisible();

  // The file back in: the same totals, and the same file out again.
  await byId(page, 'stats-import').setInputFiles({
    name: 'flying-stats.json',
    mimeType: 'application/json',
    buffer: Buffer.from(text),
  });
  await expect(byId(page, 'stats-status')).toHaveText('Statistics imported and added to these.');
  await expect(total(page, 'missions')).toHaveText('1');
  await expect(total(page, 'gun_rounds')).toHaveText(String(rounds));
  expect(await exported(page)).toBe(text);

  // And they outlast a reload.
  await page.reload();
  await openGame(page, `${HEADERS_BASE}?debug=1&${CHEAP}`, { menu: true });
  const skip = byId(page, 'firstrun-skip');
  const menu = byId(page, 'main-menu');
  await expect(skip.or(menu)).toBeVisible({ timeout: 30_000 });
  if (await skip.isVisible()) await skip.click();
  await byId(page, 'menu-statistics').click();
  await expect(total(page, 'missions')).toHaveText('1');
  expect(problems).toEqual([]);
});
