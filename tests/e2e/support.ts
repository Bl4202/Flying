import { expect, type Page } from '@playwright/test';

/** Preview with every security header (a host that can set them): the full, isolated mode. */
export const HEADERS_BASE = 'http://127.0.0.1:4173/Flying/';
/** Preview with no headers at all, as GitHub Pages serves: isolation comes from the Service Worker. */
export const PAGES_BASE = 'http://127.0.0.1:4174/Flying/';

/** Collects anything that would indicate a CSP violation or a script error. */
export function watchErrors(page: Page): string[] {
  const problems: string[] = [];
  page.on('console', (m) => {
    if (m.type() === 'error') {
      problems.push(`console: ${m.text()}`);
    }
  });
  page.on('pageerror', (e) => problems.push(`pageerror: ${e.message}`));
  return problems;
}

export const byId = (page: Page, id: string) => page.locator(`[data-testid="${id}"]`);

/**
 * Opens the game and gets past the compat screen if it shows (a first run always shows it, and in
 * CI so does every run, because there is no WebGPU adapter), leaving the loop running.
 */
export async function openGame(page: Page, url: string): Promise<void> {
  await page.goto(url);
  const screen = byId(page, 'compat-screen');
  const canvas = byId(page, 'render-canvas');
  await expect(screen.or(canvas)).toBeVisible({ timeout: 30_000 });
  if (await screen.isVisible()) {
    await byId(page, 'compat-continue').click();
  }
  await expect(canvas).toBeVisible({ timeout: 30_000 });
  // Set once everything is wired and the loop is going (the overlay hotkey exists from here on).
  await expect(byId(page, 'stage')).toHaveAttribute('data-state', 'running', { timeout: 30_000 });
  // The test hooks exist only with ?debug= or ?flags=stall.
  if (await page.evaluate(() => window.__flying !== undefined)) {
    await waitForStats(page);
  }
}

/** Gets past the compat screen if it shows (a first run always does); does not wait for the game. */
export async function passCompat(page: Page, url: string): Promise<void> {
  await page.goto(url);
  const screen = byId(page, 'compat-screen');
  const anything = byId(page, 'perf-panel')
    .or(byId(page, 'bench-panel'))
    .or(byId(page, 'render-canvas'));
  await expect(screen.or(anything)).toBeVisible({ timeout: 30_000 });
  if (await screen.isVisible()) {
    await byId(page, 'compat-continue').click();
  }
}

/** Waits until both threads have reported at least once. */
export async function waitForStats(page: Page): Promise<void> {
  await expect
    .poll(
      () =>
        page.evaluate(() => {
          const s = window.__flying?.stats();
          return s?.sim !== undefined && s.render !== undefined;
        }),
      { timeout: 30_000 },
    )
    .toBe(true);
}

/** The core's tick and the report's own timestamp, so rates do not depend on when we looked. */
export function simSample(page: Page) {
  return page.evaluate(() => {
    const sim = window.__flying?.stats().sim;
    return sim ? { tick: sim.tick, atMs: sim.atMs, hz: sim.hz } : undefined;
  });
}

/** What the page believes about the test hooks, for `page.evaluate` typing. */
declare global {
  interface Window {
    __flying?: {
      stallSim(ms: number): void;
      stallRender(ms: number): void;
      pause(): void;
      resume(): void;
      resetRun(): void;
      readPixels(): Promise<{
        width: number;
        height: number;
        distinctColours: number;
        skyFraction: number;
      }>;
      stats(): {
        sim: { tick: number; atMs: number; hz: number; observer: { quat: number[] } } | undefined;
        render: { frames: number; fps: number; backend: string } | undefined;
      };
      host(): 'worker' | 'main';
      backend(): string;
    };
  }
}
