import { expect, test, type Page } from '@playwright/test';
import { byId, HEADERS_BASE } from './support.ts';

const SCREENS = `${HEADERS_BASE}?selftest=screens`;

const S1 = ['s1-100', 's1-1000', 's1-5000', 's1-10000', 's1-15000'];
const S2 = ['s2-curve-10000', 's2-curve-15000', 's2-roll-10000', 's2-fov-10000'];

/** Runs the screenshot gates in this browser and returns each row's result and numbers. */
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
  return rows;
}

test.describe('the horizon in screenshots (S1 to S3)', () => {
  test.setTimeout(180_000);

  test('S1 and S2: the dip is right at five heights, and the horizon is the image of the horizon circle', async ({
    page,
  }) => {
    const rows = await measure(page, `${SCREENS}&case=horizon&renderer=webgl2`);
    for (const id of [...S1, ...S2]) {
      test.info().annotations.push({ type: id, description: rows[id]?.text ?? 'missing' });
      expect(rows[id]?.result, `${id}: ${rows[id]?.text ?? 'missing'}`).toBe('pass');
    }
    expect(rows['error']).toBeUndefined();
  });

  test('S3: a seeded flat-Earth fault makes the gates fail, so they would catch the real defect', async ({
    page,
  }) => {
    const rows = await measure(
      page,
      `${SCREENS}&case=horizon&renderer=webgl2&debug=1&fault=flat-earth`,
    );
    // The dip is wrong where the Earth's curve matters: from 5 km up it is more than a degree out.
    for (const id of ['s1-5000', 's1-10000', 's1-15000']) {
      test.info().annotations.push({ type: id, description: rows[id]?.text ?? 'missing' });
      expect(rows[id]?.result, `${id}: ${rows[id]?.text ?? 'missing'}`).toBe('fail');
    }
    for (const id of S2) {
      expect(rows[id]?.result, `${id}: ${rows[id]?.text ?? 'missing'}`).toBe('fail');
    }
  });
});
