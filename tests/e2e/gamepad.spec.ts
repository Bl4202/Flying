/**
 * Gamepads and joysticks in the browser (GDD 58.6, AC-58.6), with a stubbed `navigator.getGamepads`
 * shaped as the Gamepad API gives it: the build had no hardware to test with, so these specs are
 * the only check of the pad path from the browser's side. A standard pad, a raw joystick with a
 * hat on an axis, hot-plugging and the profile following the devices.
 */
import { expect, test, type Page } from '@playwright/test';
import { flightStatus, HEADERS_BASE, openGame, statusNow } from './support.ts';

const AIR = `${HEADERS_BASE}?debug=1&spawn=air`;

const STANDARD = {
  id: 'Xbox Wireless Controller (STANDARD GAMEPAD Vendor: 045e Product: 0b13)',
  mapping: 'standard',
  axes: 4,
  buttons: 17,
};
const HOTAS = {
  id: 'T.16000M (Vendor: 044f Product: b10a)',
  mapping: '',
  axes: 10,
  buttons: 12,
};
/** What a raw joystick's hat axis reads when it is centred. */
const HAT_CENTRED = 1.2857142857142856;

interface PadState {
  axes?: Record<number, number>;
  buttons?: Record<number, number>;
}

declare global {
  interface Window {
    __setPad?: (index: number, spec: unknown) => void;
  }
}

/** Replaces `navigator.getGamepads` before the page loads, with a list the test controls. */
async function stubPads(page: Page): Promise<void> {
  await page.addInitScript(() => {
    const pads: unknown[] = [];
    Object.defineProperty(navigator, 'getGamepads', { value: () => pads, configurable: true });
    window.__setPad = (index, spec) => {
      while (pads.length <= index) pads.push(null);
      if (spec === null) {
        pads[index] = null;
        return;
      }
      const s = spec as {
        id: string;
        mapping: string;
        axes: number[];
        buttons: number[];
      };
      pads[index] = {
        connected: true,
        index,
        id: s.id,
        mapping: s.mapping,
        timestamp: performance.now(),
        axes: s.axes,
        buttons: s.buttons.map((value) => ({ pressed: value > 0.5, value })),
      };
    };
  });
}

type Kind = typeof STANDARD | typeof HOTAS;

/** Plugs in (or updates) a device with the given axes and buttons set and the rest at rest. */
async function setPad(page: Page, kind: Kind, state: PadState = {}, index = 0): Promise<void> {
  const axes = Array.from({ length: kind.axes }, (_, i) => state.axes?.[i] ?? 0);
  if (kind === HOTAS && state.axes?.[9] === undefined) axes[9] = HAT_CENTRED;
  const buttons = Array.from({ length: kind.buttons }, (_, i) => state.buttons?.[i] ?? 0);
  await page.evaluate(
    ([i, spec]) => {
      window.__setPad?.(i, spec);
    },
    [index, { id: kind.id, mapping: kind.mapping, axes, buttons }] as const,
  );
}

const unplug = (page: Page, index = 0): Promise<void> =>
  page.evaluate((i) => {
    window.__setPad?.(i, null);
  }, index);

const profile = (page: Page): Promise<string | undefined> =>
  page.evaluate(() => window.__flying?.input()?.profile);
const nzNow = async (page: Page): Promise<number> => (await statusNow(page))?.nz ?? Number.NaN;

test('a standard gamepad takes over: the profile follows it, the right stick flies, the d-pad drops flaps', async ({
  page,
}) => {
  await stubPads(page);
  await openGame(page, AIR);
  await flightStatus(page);
  expect(await profile(page)).toBe('default-km');
  await setPad(page, STANDARD, { axes: { 3: 0.9 } });
  await expect.poll(() => profile(page), { timeout: 2_000 }).toBe('gamepad-standard');
  // Stick back: pull.
  await expect.poll(() => nzNow(page), { timeout: 5_000 }).toBeGreaterThan(1.3);
  await setPad(page, STANDARD);
  await expect
    .poll(async () => Math.abs((await nzNow(page)) - 1), { timeout: 8_000 })
    .toBeLessThan(0.4);
  // The d-pad as buttons: right drops the flaps to take-off, left raises them.
  expect((await flightStatus(page)).flapDetent).toBe(0);
  await setPad(page, STANDARD, { buttons: { 15: 1 } });
  await expect.poll(async () => (await statusNow(page))?.flapDetent, { timeout: 3_000 }).toBe(1);
  await setPad(page, STANDARD, { buttons: { 14: 1 } });
  await expect.poll(async () => (await statusNow(page))?.flapDetent, { timeout: 3_000 }).toBe(0);
});

test('unplugging leaves no stuck input; plugging back in restores the pad within two seconds', async ({
  page,
}) => {
  await stubPads(page);
  await openGame(page, AIR);
  await flightStatus(page);
  await setPad(page, STANDARD, { axes: { 3: 0.9 } });
  await expect.poll(() => profile(page), { timeout: 2_000 }).toBe('gamepad-standard');
  await expect.poll(() => nzNow(page), { timeout: 5_000 }).toBeGreaterThan(1.3);

  await unplug(page);
  // The lost pad's stick is neutral at once, so the load factor comes back to one g...
  await expect
    .poll(async () => Math.abs((await nzNow(page)) - 1), { timeout: 8_000 })
    .toBeLessThan(0.4);
  // ...and the keyboard profile is back within two seconds (AC-58.6).
  await expect.poll(() => profile(page), { timeout: 2_000 }).toBe('default-km');

  await setPad(page, STANDARD, { axes: { 3: 0.9 } });
  await expect.poll(() => profile(page), { timeout: 2_000 }).toBe('gamepad-standard');
  await expect.poll(() => nzNow(page), { timeout: 5_000 }).toBeGreaterThan(1.3);
});

test('a raw joystick with a hat: its profile, its stick, its throttle lever and its hat', async ({
  page,
}) => {
  await stubPads(page);
  await openGame(page, AIR);
  await flightStatus(page);
  // The lever pushed forward (-1) is full throttle; the pad takes the lever over at once.
  await setPad(page, HOTAS, { axes: { 1: 0.9, 2: -1 } });
  await expect.poll(() => profile(page), { timeout: 2_000 }).toBe('generic-joystick');
  await expect.poll(() => nzNow(page), { timeout: 5_000 }).toBeGreaterThan(1.3);
  await expect
    .poll(async () => (await statusNow(page))?.throttle ?? 0, { timeout: 5_000 })
    .toBeGreaterThan(0.95);
  await setPad(page, HOTAS, { axes: { 2: 1 } });
  await expect
    .poll(async () => (await statusNow(page))?.throttle ?? 1, { timeout: 5_000 })
    .toBeLessThan(0.05);
  // The hat on axis 9: right is flaps down (take-off), left is flaps up.
  await setPad(page, HOTAS, { axes: { 2: 1, 9: -0.4285714285714286 } });
  await expect.poll(async () => (await statusNow(page))?.flapDetent, { timeout: 3_000 }).toBe(1);
  await setPad(page, HOTAS, { axes: { 2: 1 } });
  await setPad(page, HOTAS, { axes: { 2: 1, 9: 0.7142857142857142 } });
  await expect.poll(async () => (await statusNow(page))?.flapDetent, { timeout: 3_000 }).toBe(0);
});

test('a second device is kept apart: the stick of the first still flies', async ({ page }) => {
  await stubPads(page);
  await openGame(page, AIR);
  await flightStatus(page);
  await setPad(page, HOTAS, { axes: { 1: 0.9, 2: 1 } }, 0);
  await setPad(page, STANDARD, {}, 1);
  await expect.poll(() => profile(page), { timeout: 2_000 }).toBe('generic-joystick');
  await expect.poll(() => nzNow(page), { timeout: 5_000 }).toBeGreaterThan(1.3);
});
