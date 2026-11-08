import { expect, test, type Page } from '@playwright/test';
import { byId, HEADERS_BASE, openGame, checkScreensBackend, SCREENS_RENDERER } from './support.ts';

const SCREENS = `${HEADERS_BASE}?selftest=screens&case=terrain&renderer=${SCREENS_RENDERER}`;

async function measure(
  page: Page,
  url: string,
): Promise<Record<string, { result: string; text: string }>> {
  await page.goto(url);
  await expect(byId(page, 'status')).toHaveText('done', { timeout: 150_000 });
  const rows: Record<string, { result: string; text: string }> = {};
  for (const cell of await page.locator('[data-testid][data-result]').all()) {
    const id = (await cell.getAttribute('data-testid')) ?? '';
    rows[id] = {
      result: (await cell.getAttribute('data-result')) ?? '',
      text: (await cell.textContent()) ?? '',
    };
  }
  checkScreensBackend(rows, (backend) =>
    test.info().annotations.push({ type: 'backend', description: backend }),
  );
  return rows;
}

test.describe('the terrain in screenshots (S5)', () => {
  test.setTimeout(180_000);

  test("S5: no crack shows the globe in 24 views across the terrain levels' seams", async ({
    page,
  }) => {
    const rows = await measure(page, SCREENS);
    const views = Object.keys(rows).filter((id) => id.startsWith('s5-') && id !== 's5-total');
    expect(views).toHaveLength(24);
    for (const id of views) {
      expect(rows[id]?.result, `${id}: ${rows[id]?.text ?? ''}`).toBe('pass');
      // The seam is in the picture: ground of two levels (or three) either side of it.
      const levels = Number(/(\d) terrain levels/.exec(rows[id]?.text ?? '')?.[1]);
      expect(levels, id).toBeGreaterThanOrEqual(2);
    }
    expect(rows['s5-total']?.result).toBe('pass');
    expect(rows['error']).toBeUndefined();
  });

  test('a seeded fault that removes the skirts makes the gate fail, so it would catch a crack', async ({
    page,
  }) => {
    const rows = await measure(page, `${SCREENS}&debug=1&fault=no-skirts`);
    expect(rows['s5-total']?.result, rows['s5-total']?.text).toBe('fail');
    const cracked = Object.entries(rows).filter(
      ([id, r]) => id !== 's5-total' && id.startsWith('s5-') && r.result === 'fail',
    );
    test.info().annotations.push({ type: 'cracked views', description: String(cracked.length) });
    expect(cracked.length).toBeGreaterThanOrEqual(8);
  });
});

test('all four terrain levels reach the renderer once a sortie starts', async ({ page }) => {
  await openGame(page, `${HEADERS_BASE}?flags=stall&renderer=${SCREENS_RENDERER}`);
  await expect
    .poll(async () => page.evaluate(() => window.__flying?.stats().render?.terrainLevels ?? 0), {
      timeout: 15_000,
    })
    .toBe(0b1111);
});
