/**
 * Scripted reference-machine run: opens the build in real Chrome, notes the compat verdict, flies
 * a scripted minute, runs the full benchmark and PS-0, and writes the reports and a summary
 * under `docs/exit/v0.0b/machines/<name>/`. Run it on the machine being measured:
 *
 *   pnpm perf:local --name m4-webgpu --class recommended
 *   pnpm perf:local --name m4-webgl2-throttled --class minimum --renderer webgl2 --throttle 4
 *
 * See docs/runbooks/reference-machines.md. Not part of CI: it needs a real GPU and a headed
 * browser to mean anything.
 */
import { mkdirSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { chromium, type Page } from '@playwright/test';
import {
  judgeRun,
  parseLocalArgs,
  renderSummary,
  type FlightSummary,
  type LocalOptions,
  type RunFacts,
} from './perf-local-lib.ts';

interface Hooks {
  stats(): {
    sim: { hz: number } | undefined;
    render:
      | {
          fps: number;
          backend: string;
          frameMsRun: { p99: number; max: number };
        }
      | undefined;
  };
  resetRun(): void;
}

const opts: LocalOptions = parseLocalArgs(process.argv.slice(2));
const outDir = resolve(opts.out, opts.name);
mkdirSync(outDir, { recursive: true });

const url = (query: string): string => {
  const renderer = opts.renderer === 'auto' ? '' : `&renderer=${opts.renderer}`;
  return `${opts.base}?${query}${renderer}`;
};
const byId = (page: Page, id: string) => page.locator(`[data-testid="${id}"]`);
const save = (name: string, content: string): void => {
  writeFileSync(join(outDir, name), content.endsWith('\n') ? content : `${content}\n`);
};

const browser = await chromium.launch({ channel: 'chrome', headless: opts.headless });
const context = await browser.newContext({ viewport: { width: 1600, height: 900 } });
const page = await context.newPage();
const consoleErrors: string[] = [];
page.on('console', (m) => {
  if (m.type() === 'error') consoleErrors.push(`console: ${m.text()}`);
});
page.on('pageerror', (e) => consoleErrors.push(`pageerror: ${e.message}`));

if (opts.throttle > 1) {
  const cdp = await context.newCDPSession(page);
  await cdp.send('Emulation.setCPUThrottlingRate', { rate: opts.throttle });
}

const facts: RunFacts = {
  compatVerdict: null,
  compatCodes: [],
  adapter: '',
  renderer: '',
  displayHz: 0,
  flight: null,
  perf: null,
  benchmark: null,
  consoleErrors,
};

/** Passes the compat screen if one shows, recording what it said. */
async function passCompat(): Promise<void> {
  const screen = byId(page, 'compat-screen');
  const alive = byId(page, 'render-canvas')
    .or(byId(page, 'bench-panel'))
    .or(byId(page, 'perf-panel'));
  await screen.or(alive).first().waitFor({ timeout: 60_000 });
  if (await screen.isVisible()) {
    const verdict = await byId(page, 'compat-verdict').getAttribute('data-verdict');
    if (verdict === 'ok' || verdict === 'degraded' || verdict === 'fatal') {
      facts.compatVerdict = verdict;
    }
    facts.compatCodes = await byId(page, 'compat-codes').locator('li').allTextContents();
    const rows = await page.locator('[data-testid^="compat-row-"]').evaluateAll((els) =>
      els.map((el) => ({
        id: el.getAttribute('data-testid'),
        state: el.getAttribute('data-state'),
        text: el.textContent,
      })),
    );
    save('compat.json', JSON.stringify({ verdict, codes: facts.compatCodes, rows }, null, 2));
    if (verdict === 'fatal')
      throw new Error(`the compat check is fatal: ${facts.compatCodes.join(', ')}`);
    await byId(page, 'compat-continue').click();
  }
}

/** The display rate, as `requestAnimationFrame` sees it over one second. */
async function measureDisplayHz(): Promise<number> {
  return page.evaluate(
    () =>
      new Promise<number>((resolveHz) => {
        let frames = 0;
        const start = performance.now();
        const tick = (): void => {
          frames += 1;
          if (performance.now() - start >= 1000) resolveHz(frames);
          else requestAnimationFrame(tick);
        };
        requestAnimationFrame(tick);
      }),
  );
}

/**
 * Holds each key for a while (`''` is hands off), in a pattern that exercises every axis and the
 * throttle. The stick drives the surfaces directly (no FCS before v0.1b), so the inputs are short
 * taps in opposing pairs with hands-off spells between them: long full-deflection holds tumble the
 * aircraft and the run would mostly measure crashes. The second half mirrors the first, so leftover
 * bank does not build into a spiral dive. Measured on the Kestrel (v0.1a): two minutes without a
 * crash, about 23 km flown, so the terrain patch re-centres at least once.
 */
const MANOEUVRE: readonly (readonly [string, number])[] = [
  ['ShiftLeft', 600],
  ['', 1500],
  ['KeyS', 300],
  ['', 1500],
  ['KeyW', 300],
  ['', 1000],
  ['KeyD', 300],
  ['', 1000],
  ['KeyA', 300],
  ['', 1500],
  ['KeyE', 400],
  ['KeyQ', 400],
  ['', 1500],
  ['PageDown', 600],
  ['', 1500],
  ['KeyS', 300],
  ['', 1500],
  ['KeyW', 300],
  ['', 1000],
  ['KeyA', 300],
  ['', 1000],
  ['KeyD', 300],
  ['', 1500],
  ['KeyQ', 400],
  ['KeyE', 400],
  ['', 1500],
];

async function fly(seconds: number): Promise<FlightSummary> {
  const fps: number[] = [];
  const hz: number[] = [];
  let frameP99 = 0;
  let frameMax = 0;
  const toasts = new Set<string>();
  const end = Date.now() + seconds * 1000;
  let step = 0;
  while (Date.now() < end) {
    const [key, ms] = MANOEUVRE[step % MANOEUVRE.length] ?? ['', 1000];
    step += 1;
    if (key !== '') await page.keyboard.down(key);
    const holdUntil = Math.min(end, Date.now() + ms);
    while (Date.now() < holdUntil) {
      await page.waitForTimeout(Math.min(500, Math.max(holdUntil - Date.now(), 0)));
      const s = await page.evaluate(() => {
        const hooks = (window as unknown as { __flying?: Hooks }).__flying;
        return hooks?.stats();
      });
      if (s?.render) {
        fps.push(s.render.fps);
        frameP99 = Math.max(frameP99, s.render.frameMsRun.p99);
        frameMax = Math.max(frameMax, s.render.frameMsRun.max);
      }
      if (s?.sim) hz.push(s.sim.hz);
      for (const t of await byId(page, 'toast').allTextContents()) toasts.add(t);
    }
    if (key !== '') await page.keyboard.up(key);
  }
  const mean = (xs: number[]): number =>
    xs.length === 0 ? 0 : xs.reduce((a, b) => a + b, 0) / xs.length;
  return {
    seconds,
    fpsAvg: mean(fps),
    fpsMin: fps.length === 0 ? 0 : Math.min(...fps),
    frameP99Ms: frameP99,
    frameMaxMs: frameMax,
    simHzAvg: mean(hz),
    toasts: [...toasts],
  };
}

try {
  // 1. Compat screen and a scripted flight.
  await page.goto(url('debug=1'));
  await passCompat();
  await byId(page, 'stage')
    .and(page.locator('[data-state="running"]'))
    .waitFor({ timeout: 60_000 });
  facts.displayHz = await measureDisplayHz();
  // The overlay has filled in by now (it refreshes four times a second).
  const text = async (id: string): Promise<string> =>
    ((await byId(page, id).getAttribute('data-value')) ?? '').replace('–', '');
  facts.adapter = await text('ov-adapter');
  facts.renderer = `${await text('ov-renderer')} ${await text('ov-device')}`.trim();
  await page.evaluate(() => {
    (window as unknown as { __flying?: Hooks }).__flying?.resetRun();
  });
  if (opts.fly > 0) {
    console.log(`flying for ${String(opts.fly)} s…`);
    facts.flight = await fly(opts.fly);
    save('flight.json', JSON.stringify(facts.flight, null, 2));
  }
  await page.screenshot({ path: join(outDir, 'flight.png') });

  // 2. The first-run benchmark.
  console.log('benchmark…');
  await page.goto(url('bench=full'));
  await passCompat();
  await byId(page, 'bench-status')
    .and(page.locator('[data-state="done"]'))
    .waitFor({ timeout: 180_000 });
  const bench = JSON.parse((await byId(page, 'bench-report').textContent()) ?? '{}') as {
    classification?: { profile: string; preset: string };
  };
  save('benchmark.json', JSON.stringify(bench, null, 2));
  if (bench.classification) facts.benchmark = bench.classification;

  // 3. PS-0, strict.
  console.log(`PS-0 for ${String(opts.seconds)} s…`);
  await page.goto(url(`perf=ps0&seconds=${String(opts.seconds)}`));
  await passCompat();
  await byId(page, 'perf-status')
    .and(page.locator('[data-state="done"]'))
    .waitFor({ timeout: (opts.seconds + 120) * 1000 });
  const report = JSON.parse((await byId(page, 'perf-report').textContent()) ?? '{}') as {
    verdict: 'pass' | 'fail';
    metrics: { simHz: number; renderFps: number };
    gates: { id: string; pass: boolean }[];
  };
  save('perf-ps0.json', JSON.stringify(report, null, 2));
  facts.perf = {
    verdict: report.verdict,
    simHz: report.metrics.simHz,
    renderFps: report.metrics.renderFps,
    failedGates: report.gates.filter((g) => !g.pass).map((g) => g.id),
  };
} catch (e) {
  const message = e instanceof Error ? e.message : String(e);
  console.error(`run stopped early: ${message}`);
  consoleErrors.push(`run stopped early: ${message}`);
} finally {
  await browser.close();
}

const judgement = judgeRun(facts, opts.machineClass);
save('summary.md', renderSummary(opts, facts, judgement, new Date().toISOString().slice(0, 10)));
console.log(`\n${judgement.pass ? 'PASS' : 'FAIL'}: reports in ${outDir}`);
for (const c of judgement.checks) console.log(`  [${c.pass ? 'x' : ' '}] ${c.text}`);
process.exit(judgement.pass ? 0 : 1);
