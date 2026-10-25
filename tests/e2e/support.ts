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

/** The number at the start of an overlay row's value (`NaN` for a dash). */
export const overlayNumber = async (page: Page, id: string): Promise<number> =>
  Number.parseFloat((await byId(page, id).getAttribute('data-value')) ?? 'NaN');

/** Elevator, aileron and rudder in degrees, from the overlay's `Surfaces` row. */
export async function overlaySurfaces(page: Page): Promise<[number, number, number]> {
  const text = (await byId(page, 'ov-surfaces').getAttribute('data-value')) ?? '';
  const m = /^e (\S+) a (\S+) r (\S+)°$/.exec(text);
  return m ? [Number(m[1]), Number(m[2]), Number(m[3])] : [Number.NaN, Number.NaN, Number.NaN];
}

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
  // With the debug overlay open, wait for its first refresh (it shows dashes until then).
  if ((await byId(page, 'ov-tas').count()) > 0) {
    await expect
      .poll(async () => Number.isFinite(await overlayNumber(page, 'ov-tas')), { timeout: 10_000 })
      .toBe(true);
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

/** The ownship as the page's test hook reports it (`AircraftStatus` in `sim-bridge`). */
export interface FlyingStatus {
  spawnSeq: number;
  assist: number;
  fcsMode: number;
  flags: number;
  flags2: number;
  throttle: number;
  tas: number;
  cas: number;
  mach: number;
  alpha: number;
  nz: number;
  gearPos: number;
  flapDetent: number;
  airbrake: number;
  brakeL: number;
  brakeR: number;
  groundSpeed: number;
  vs: number;
  agl: number;
  gLagged: number;
  gStress: number;
  redout: number;
  nCmd: number;
  gearBits: number;
  crashCause: number;
  noseAz: number;
  noseEl: number;
}

/** `OWNSHIP_FLAG2` bits, as the status reports them. */
export const FLAG2 = {
  WOW: 1,
  PARKING_BRAKE: 2,
  ALPHA_LIMIT: 4,
  G_LIMIT: 8,
  OVERSPEED: 16,
  OVERSTRESS: 32,
  BLACKOUT: 64,
  GLOC: 128,
  REDOUT: 256,
  HARD_LANDING: 512,
  MOUSE_AIM: 16384,
  THROTTLE_HELD: 32768,
} as const;

export const ASSIST = { FULL: 1, STANDARD: 2, REDUCED: 3, MANUAL: 4 } as const;

export interface LatencyQuantiles {
  count: number;
  avg: number;
  p95: number;
  p99: number;
  max: number;
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
        meanLuma: number;
      }>;
      stats(): {
        sim:
          | {
              tick: number;
              atMs: number;
              hz: number;
              droppedMs: number;
              droppedEvents: number;
              stepRun: { count: number; max: number };
              aircraft: { quat: number[]; tas: number; abStage: number; flags: number };
            }
          | undefined;
        render:
          | {
              frames: number;
              atMs: number;
              fps: number;
              backend: string;
              frameMsRun: { count: number; max: number };
            }
          | undefined;
      };
      host(): 'worker' | 'main';
      backend(): string;
      status(): FlyingStatus | undefined;
      input():
        | {
            mouseMode: string;
            context: string;
            profile: string;
            throttle: number;
            assistRequest: number;
            stats: { sent: number; refused: number; heartbeats: number; changed: number };
          }
        | undefined;
      menu(): null | 'pause' | 'controls';
    };
  }
}

/** The aircraft's state from the page's test hook (waits until there is one). */
export async function flightStatus(page: Page): Promise<FlyingStatus> {
  let latest: FlyingStatus | undefined;
  await expect
    .poll(
      async () => {
        latest = await page.evaluate(() => window.__flying?.status());
        return latest !== undefined;
      },
      { timeout: 10_000 },
    )
    .toBe(true);
  if (latest === undefined) throw new Error('no aircraft status');
  return latest;
}

/** The newest status, without waiting (for polling inside `expect.poll`). */
export const statusNow = (page: Page): Promise<FlyingStatus | undefined> =>
  page.evaluate(() => window.__flying?.status());

/** The control latency since the last `resetRun`, from the sim's report (AC-58.3). */
export function controlLatencyRun(page: Page): Promise<LatencyQuantiles | undefined> {
  return page.evaluate(
    () =>
      (
        window.__flying?.stats().sim as unknown as
          { controlLatencyRun?: LatencyQuantiles } | undefined
      )?.controlLatencyRun,
  );
}

/** Both threads' newest reports, with the numbers the independence tests compare. */
export interface Sample {
  sim: {
    tick: number;
    atMs: number;
    hz: number;
    droppedMs: number;
    droppedEvents: number;
    stepRunMaxMs: number;
  };
  render: { frames: number; atMs: number; fps: number; frameRunMaxMs: number; backend: string };
}

export function sample(page: Page): Promise<Sample> {
  return page.evaluate(() => {
    const s = window.__flying?.stats();
    if (s?.sim === undefined || s.render === undefined) throw new Error('no reports yet');
    return {
      sim: {
        tick: s.sim.tick,
        atMs: s.sim.atMs,
        hz: s.sim.hz,
        droppedMs: s.sim.droppedMs,
        droppedEvents: s.sim.droppedEvents,
        stepRunMaxMs: s.sim.stepRun.max,
      },
      render: {
        frames: s.render.frames,
        atMs: s.render.atMs,
        fps: s.render.fps,
        frameRunMaxMs: s.render.frameMsRun.max,
        backend: s.render.backend,
      },
    };
  });
}

/** A sample whose reports were both made at least `ms` after now, on the clock the reports use. */
export async function sampleAfter(page: Page, ms: number): Promise<Sample> {
  const target = await page.evaluate(
    (delay) => performance.timeOrigin + performance.now() + delay,
    ms,
  );
  let latest: Sample | undefined;
  await expect
    .poll(
      async () => {
        latest = await sample(page);
        return Math.min(latest.sim.atMs, latest.render.atMs) >= target;
      },
      { timeout: ms + 10_000, intervals: [100] },
    )
    .toBe(true);
  if (latest === undefined) throw new Error('no sample');
  return latest;
}

/** Ticks per second between two samples, by the reports' own timestamps. */
export function simRate(a: Sample, b: Sample): number {
  return (b.sim.tick - a.sim.tick) / ((b.sim.atMs - a.sim.atMs) / 1000);
}
