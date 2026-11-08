import { expect, test, type Page } from '@playwright/test';
import { byId, HEADERS_BASE } from './support.ts';

const SCREENS = `${HEADERS_BASE}?selftest=screens&case=hud&renderer=webgl2`;

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

const CASES = Object.freeze([
  's4-0-0',
  's4-140-0',
  's4-280-0',
  's4-0--3',
  's4-140--3',
  's4-280--3',
  's4-0--6',
  's4-140--6',
  's4-0-2',
  's4-280-2',
]);

test.describe('the HUD in screenshots (S4)', () => {
  test.setTimeout(180_000);

  test("S4: the HUD's zero line sits above the rendered horizon by the dip, at ten headings and pitches", async ({
    page,
  }) => {
    const rows = await measure(page, SCREENS);
    for (const id of CASES) {
      test.info().annotations.push({ type: id, description: rows[id]?.text ?? 'missing' });
      expect(rows[id]?.result, `${id}: ${rows[id]?.text ?? 'missing'}`).toBe('pass');
    }
    expect(rows['error']).toBeUndefined();
  });

  test('the HUD stays inside its field of view, steps its detail down, and is drawn over the chase view', async ({
    page,
  }) => {
    const rows = await measure(page, SCREENS);
    for (const id of ['s4-bounds', 's4-declutter', 's4-overlay']) {
      test.info().annotations.push({ type: id, description: rows[id]?.text ?? 'missing' });
      expect(rows[id]?.result, `${id}: ${rows[id]?.text ?? 'missing'}`).toBe('pass');
    }
  });

  test('a HUD ladder built in body axes, not the local level, fails the gate whenever the nose is not level', async ({
    page,
  }) => {
    const rows = await measure(page, `${SCREENS}&debug=1&fault=hud-body-axes`);
    // Every pitched case fails: the ladder stays at the boresight while the horizon moves.
    for (const id of CASES.filter((c) => !c.endsWith('-0'))) {
      expect(rows[id]?.result, `${id}: ${rows[id]?.text ?? 'missing'}`).toBe('fail');
    }
    expect(CASES.filter((c) => !c.endsWith('-0'))).toHaveLength(7);
  });
});
