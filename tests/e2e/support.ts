import { expect, type Page } from '@playwright/test';

/** Preview with every security header (a host that can set them): the full, isolated mode. */
export const HEADERS_BASE = 'http://127.0.0.1:4173/Flying/';
/** Preview with no headers at all, as GitHub Pages serves: isolation comes from the Service Worker. */
export const PAGES_BASE = 'http://127.0.0.1:4174/Flying/';

/**
 * The renderer the screenshot gates run on: WebGL2 in CI, and on a reference machine
 * `SCREENS_RENDERER=webgpu PW_CHANNEL=chrome` (the headless shell has no WebGPU adapter).
 */
export const SCREENS_RENDERER = process.env['SCREENS_RENDERER'] ?? 'webgl2';

/**
 * Fails when the screenshot gates ran on another renderer than the one asked for (`webgpu` quietly
 * falls back to WebGL2 where there is no adapter), and records the backend with the test.
 */
export function checkScreensBackend(
  rows: Record<string, { text: string } | undefined>,
  annotate: (backend: string) => void,
): void {
  const backend = rows['backend']?.text ?? 'missing';
  annotate(backend);
  expect(backend.startsWith(SCREENS_RENDERER), `the pictures were drawn by ${backend}`).toBe(true);
}

/**
 * A small drawing buffer at the lowest preset. The specs that check the threads (rates, stalls,
 * pausing) and not the picture add this, so that a software rasteriser, which is all CI has, keeps
 * up with the scene and a frame count measures the loop and not the fill rate.
 */
export const CHEAP = 'res=480x270&quality=low';

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
export async function openGame(
  page: Page,
  url: string,
  options: { menu?: boolean } = {},
): Promise<void> {
  // The game opens at the first run or the main menu; most specs want the flight, so `fly=1`.
  await page.goto(options.menu === true ? url : withFly(url));
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

/** `url` with `fly=1` added, unless it already says what to do. */
export function withFly(url: string): string {
  const u = new URL(url);
  if (!u.searchParams.has('fly')) u.searchParams.set('fly', '1');
  return u.href;
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
              /** `CORE_STATE` (`sim-bridge`): 2 running, 3 paused (a menu or the debrief holds it). */
              coreState: number;
              stepRun: { count: number; max: number };
              aircraft: { quat: number[]; tas: number; abStage: number; flags: number };
            }
          | undefined;
        render:
          | {
              frames: number;
              atMs: number;
              fps: number;
              terrainLevels?: number;
              backend: string;
              frameMsRun: { count: number; max: number };
              drawnFrames: { bandit: number; missiles: number; tracers: number };
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
      menu(): null | 'pause' | 'controls' | 'debrief' | 'feel';
      flow(): string;
      audio(): {
        kind: string;
        state: string;
        levelDb: number;
        paramWrites: number;
        workletError: string | undefined;
        onsets: number;
        lastOnset: { tone: number; time: number } | undefined;
        muted: boolean;
      };
      view(): {
        camera: string;
        declutter: number;
        brightness: number;
        fovDeg: number;
        lookYawRate: number;
        lookPitchRate: number;
        zoom: boolean;
        freeLook: boolean;
      };
      presentationError(): string | undefined;
      combat(): FlyingCombat | undefined;
      debrief(): FlyingDebrief | null;
      memory(): Promise<FlyingMemory>;
    };
  }
}

/** The duel's status from the page's test hook (`CombatStatus` in `sim-bridge`), as JSON. */
export interface FlyingCombat {
  flags: number;
  seeker: number;
  /** `LAUNCH_INHIBIT` of the last refused launch, and how long ago (s). */
  inhibit: number;
  inhibitAge: number;
  rounds: number;
  missiles: number;
  signal: number;
  tgtRange: number;
  health: number;
  /** `COMBAT_COUNTER` order (a typed array in the page: copy it with `Array.from` there). */
  counters: ArrayLike<number>;
  eventSeq: number;
  banditLife: number;
  banditAi: number;
  banditSkill: number;
  banditPosBf: ArrayLike<number>;
  banditRange: number;
}

/** `COMBAT_COUNTER` (`sim-bridge`), for reading `FlyingCombat.counters`. */
export const COUNTER = {
  ROUNDS_FIRED: 0,
  ROUND_HITS: 1,
  MISSILES_FIRED: 2,
  MISSILE_HITS: 3,
  KILLS: 4,
  GUN_KILLS: 5,
  MISSILE_KILLS: 6,
  DEATHS: 7,
  HITS_TAKEN: 8,
  BANDIT_ROUNDS: 9,
  EXPLOSIONS: 10,
  BANDIT_SPAWNS: 11,
} as const;

/** The open debrief's numbers (`DebriefNumbers` in `ui`). */
export interface FlyingDebrief {
  missionId: string;
  outcome: 'landed' | 'ended' | 'died';
  airborneS: number;
  kills: number;
  gunKills: number;
  missileKills: number;
  gunRounds: number;
  gunHits: number;
  missilesFired: number;
  missileHits: number;
  deaths: number;
  hitsTaken: number;
  landings: number;
  maxAltM: number;
  maxTasMps: number;
  maxMach: number;
  taskS: number | null;
}

/** One soak sample (`MemorySample` in `app-shell`). */
export interface FlyingMemory {
  jsBytes: number;
  jsSource: 'uasm' | 'heap';
  wasmBytes: number;
  liveBuffers: number;
  liveTextures: number;
  liveAudioNodes: number;
  silences: number;
  workerErrors: number;
  deviceLosses: number;
  workers: number;
  simHz: number;
  sorties: number;
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

/**
 * The least the picture must advance, frames per second, for "the frames go on". CI draws with a
 * software rasteriser whose speed depends on the load of a shared runner (it managed 7 to 20 here),
 * and these specs hold the sim to its 60 Hz, not the renderer to a speed: the renderer has to keep
 * drawing while the other threads stall, and a stall of the thread under test would leave a gap of a
 * second or a third of one, far longer than the intervals this allows.
 */
export const MIN_RENDER_FPS = 10;

/** The longest frame interval that is no stall of the renderer, ms: well under 300 and 1 000, the stalls the specs inject. */
export const MAX_FRAME_GAP_MS = 200;

/** Frames per second between two samples, by the reports' own timestamps. */
export function renderRate(a: Sample, b: Sample): number {
  return (b.render.frames - a.render.frames) / ((b.render.atMs - a.render.atMs) / 1000);
}

/** Ticks per second between two samples, by the reports' own timestamps. */
export function simRate(a: Sample, b: Sample): number {
  return (b.sim.tick - a.sim.tick) / ((b.sim.atMs - a.sim.atMs) / 1000);
}

/** Gets past the compat screen if it shows; does not wait for anything else. */
export async function continuePastCompat(page: Page): Promise<void> {
  const screen = byId(page, 'compat-screen');
  const canvas = byId(page, 'render-canvas');
  await expect(screen.or(canvas)).toBeVisible({ timeout: 30_000 });
  if (await screen.isVisible()) await byId(page, 'compat-continue').click();
  await expect(canvas).toBeVisible({ timeout: 30_000 });
}

/** The settings as stored in this browser (`settings/2`), or undefined if nothing is stored yet. */
export function storedSettings(page: Page): Promise<Record<string, unknown> | undefined> {
  return page.evaluate(
    () =>
      new Promise<Record<string, unknown> | undefined>((resolve) => {
        const open = indexedDB.open('flying');
        open.onerror = () => {
          resolve(undefined);
        };
        open.onsuccess = () => {
          const db = open.result;
          if (!db.objectStoreNames.contains('kv')) {
            db.close();
            resolve(undefined);
            return;
          }
          const request = db.transaction('kv', 'readonly').objectStore('kv').get('settings/2');
          request.onsuccess = () => {
            db.close();
            resolve(request.result as Record<string, unknown> | undefined);
          };
          request.onerror = () => {
            db.close();
            resolve(undefined);
          };
        };
      }),
  );
}

/** Waits until the stored settings satisfy `check`: a change is written asynchronously, and a reload would cut it short. */
export async function waitForStored(
  page: Page,
  check: (stored: Record<string, unknown>) => boolean,
): Promise<void> {
  await expect
    .poll(async () => {
      const stored = await storedSettings(page);
      return stored !== undefined && check(stored);
    })
    .toBe(true);
}
