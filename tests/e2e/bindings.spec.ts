/**
 * The binding editor in the browser (GDD 58.5, ADR-37): rebinding by pressing the control, the
 * conflict marks, export and import, profiles kept between visits, and restoring the defaults.
 */
import { readFileSync } from 'node:fs';
import { expect, test, type Page } from '@playwright/test';
import { byId, flightStatus, HEADERS_BASE, openGame, statusNow } from './support.ts';

const AIR = `${HEADERS_BASE}?debug=1&spawn=air`;

async function openEditor(page: Page): Promise<void> {
  await page.keyboard.press('Escape');
  await byId(page, 'pause-controls').click();
  await expect(byId(page, 'binding-editor')).toBeVisible();
}

/** Adds `code` as a nose-up key for the pitch axis by the editor's capture. */
async function bindPitchUp(page: Page, code: string): Promise<void> {
  await byId(page, 'be-add-positive-flight.pitch').click();
  await expect(byId(page, 'be-capture')).toBeVisible();
  await page.keyboard.press(code);
  await expect(byId(page, 'be-capture')).toBeHidden();
}

const pitchRow = (page: Page) => byId(page, 'be-row-flight.pitch');
/** The chip for I as a nose-up key (not the word "Invert" elsewhere in the row). */
const chipI = (page: Page) => pitchRow(page).locator('.chip', { hasText: /^I \+/ });

test('pressing a key for an action rebinds it, and the new key flies the aircraft', async ({
  page,
}) => {
  await openGame(page, AIR);
  await flightStatus(page);
  await openEditor(page);
  await expect(byId(page, 'be-builtin-note')).toBeVisible();
  await bindPitchUp(page, 'KeyI');
  await expect(chipI(page)).toHaveCount(1);
  // The first edit to a built-in profile made a copy of it and moved to the copy.
  await expect(byId(page, 'be-builtin-note')).toHaveCount(0);
  await byId(page, 'be-close').click();
  await byId(page, 'pause-resume').click();
  await page.keyboard.down('KeyI');
  await expect
    .poll(async () => (await statusNow(page))?.nz ?? 1, { timeout: 5_000 })
    .toBeGreaterThan(1.3);
  await page.keyboard.up('KeyI');
});

test('a control used by two actions is marked in words as well as a shape', async ({ page }) => {
  await openGame(page, AIR);
  await openEditor(page);
  await expect(byId(page, 'be-conflict')).toHaveCount(0);
  // L raises the gear; give it to the parking brake as well.
  await byId(page, 'be-add-flight.parking_brake').click();
  await page.keyboard.press('KeyL');
  await expect(byId(page, 'be-conflict')).toHaveCount(2);
  await expect(byId(page, 'be-conflict').first()).toContainText('Conflict');
  await expect(byId(page, 'be-conflicts')).toContainText('2 bindings');
  // Taking it off again clears the marks.
  await byId(page, 'be-restore-flight.parking_brake').click();
  await expect(byId(page, 'be-conflict')).toHaveCount(0);
});

test('Esc while waiting for a control cancels the wait, not the editor', async ({ page }) => {
  await openGame(page, AIR);
  await openEditor(page);
  await byId(page, 'be-add-positive-flight.pitch').click();
  await expect(byId(page, 'be-capture')).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(byId(page, 'be-capture')).toBeHidden();
  await expect(byId(page, 'binding-editor')).toBeVisible();
  expect(await page.evaluate(() => window.__flying?.menu())).toBe('controls');
  // Esc again steps back to the pause menu, and again resumes.
  await page.keyboard.press('Escape');
  await expect(byId(page, 'pause-menu')).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(byId(page, 'pause-menu')).toHaveCount(0);
  expect(await page.evaluate(() => window.__flying?.menu())).toBeNull();
});

test('the profile survives a reload, and restoring the defaults takes the key away', async ({
  page,
}) => {
  await openGame(page, AIR);
  await openEditor(page);
  await bindPitchUp(page, 'KeyI');
  const id = await byId(page, 'be-profile').inputValue();
  expect(id).not.toBe('default-km');
  // Let the store finish writing, then come back.
  await page.waitForTimeout(300);
  await openGame(page, AIR);
  expect(await page.evaluate(() => window.__flying?.input()?.profile)).toBe(id);
  await openEditor(page);
  await expect(byId(page, 'be-profile')).toHaveValue(id);
  await expect(chipI(page)).toHaveCount(1);
  await byId(page, 'be-restore-all').click();
  await expect(chipI(page)).toHaveCount(0);
  await expect(pitchRow(page).locator('.chip', { hasText: /^W −/ })).toHaveCount(1);
  await expect(pitchRow(page).locator('.chip', { hasText: /^S \+/ })).toHaveCount(1);
});

test('export and import give the same profile back, byte for byte', async ({ page }) => {
  await openGame(page, AIR);
  await openEditor(page);
  await bindPitchUp(page, 'KeyI');
  const id = await byId(page, 'be-profile').inputValue();

  const exported = async (): Promise<string> => {
    const download = page.waitForEvent('download');
    await byId(page, 'be-export').click();
    const path = await (await download).path();
    return readFileSync(path, 'utf8');
  };
  const first = await exported();
  expect(JSON.parse(first)).toMatchObject({ id });

  await byId(page, 'be-delete').click();
  await expect(byId(page, 'be-profile')).not.toHaveValue(id);
  await byId(page, 'be-import').setInputFiles({
    name: 'profile.json',
    mimeType: 'application/json',
    buffer: Buffer.from(first),
  });
  await expect(byId(page, 'be-profile')).toHaveValue(id);
  await expect(chipI(page)).toHaveCount(1);
  expect(await exported()).toBe(first);
});

test('a file that is not a profile is refused, with each problem named by its place', async ({
  page,
}) => {
  await openGame(page, AIR);
  await openEditor(page);
  const before = await byId(page, 'be-profile').inputValue();
  await byId(page, 'be-import').setInputFiles({
    name: 'bad.json',
    mimeType: 'application/json',
    buffer: Buffer.from('{"a":1}'),
  });
  await expect(byId(page, 'be-import-error').first()).toBeVisible();
  await expect(byId(page, 'be-import-error').first()).toContainText(/^\/\S*: /);
  await expect(byId(page, 'be-profile')).toHaveValue(before);
});
