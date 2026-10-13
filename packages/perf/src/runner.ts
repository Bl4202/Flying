/**
 * Runs a scenario against a live sim and renderer and turns the statistics they already publish
 * into a `PerfReport`. It never touches either of them except to clear their run-level statistics
 * once the warm-up is over; time comes from the reports' own timestamps, so when this code happens
 * to look does not matter.
 */
import type { RenderStats } from '@flying/render/messages';
import type { SimStats } from '@flying/sim-bridge';
import { evaluateGates, verdictOf } from './gates.ts';
import {
  PERF_REPORT_SCHEMA,
  type BuildInfo,
  type MachineInfo,
  type PerfMetrics,
  type PerfReport,
  type RunModes,
} from './report.ts';
import type { Scenario } from './scenarios.ts';

export interface PerfTarget {
  /** The newest report from each thread, or undefined before the first. */
  stats(): { sim: SimStats | undefined; render: RenderStats | undefined };
  /** Forgets run-level statistics on both threads. */
  resetRun(): void;
}

export interface RunnerEnv {
  /** Milliseconds on the clock the stats timestamps use (shared across threads). */
  sharedNowMs(): number;
  sleep(ms: number): Promise<void>;
}

export interface Progress {
  phase: 'waiting' | 'warmup' | 'measure' | 'done';
  elapsedS: number;
  totalS: number;
}

export interface RunOptions {
  scenario: Scenario;
  /** Length of the measured window. */
  seconds: number;
  target: PerfTarget;
  env: RunnerEnv;
  build: () => BuildInfo;
  machine: () => MachineInfo;
  modes: () => RunModes;
  now: () => Date;
  onProgress?: (p: Progress) => void;
  /** How long to wait for the first reports, milliseconds. */
  statsTimeoutMs?: number;
}

type Both = { sim: SimStats; render: RenderStats };

/** How soon after the reset a report must have been made to count as "after" it. */
const AFTER_RESET_MS = 50;
const POLL_MS = 50;

async function waitForBoth(
  opts: RunOptions,
  accept: (s: Both) => boolean,
  timeoutMs: number,
  what: string,
): Promise<Both> {
  const waited = { ms: 0 };
  for (;;) {
    const { sim, render } = opts.target.stats();
    if (sim !== undefined && render !== undefined && accept({ sim, render })) {
      return { sim, render };
    }
    if (waited.ms >= timeoutMs) {
      throw new Error(`SYS-120: no ${what} arrived within ${String(timeoutMs)} ms`);
    }
    await opts.env.sleep(POLL_MS);
    waited.ms += POLL_MS;
  }
}

/** Sleeps `totalMs` in one-second steps, reporting progress at each. */
async function sleepWithProgress(
  opts: RunOptions,
  phase: 'warmup' | 'measure',
  totalS: number,
): Promise<void> {
  for (let elapsed = 0; elapsed < totalS;) {
    const step = Math.min(1, totalS - elapsed);
    await opts.env.sleep(step * 1000);
    elapsed += step;
    opts.onProgress?.({ phase, elapsedS: elapsed, totalS });
  }
}

export function computeMetrics(base: Both, end: Both): PerfMetrics {
  const windowS = Math.max((end.sim.atMs - base.sim.atMs) / 1000, 1e-9);
  const renderWindowS = Math.max((end.render.atMs - base.render.atMs) / 1000, 1e-9);
  const ticks = end.sim.tick - base.sim.tick;
  const frames = end.render.frames - base.render.frames;
  return {
    windowS,
    ticks,
    simHz: ticks / windowS,
    droppedMs: end.sim.droppedMs - base.sim.droppedMs,
    droppedEvents: end.sim.droppedEvents - base.sim.droppedEvents,
    step: end.sim.stepRun,
    frames,
    renderFps: frames / renderWindowS,
    frameMs: end.render.frameMsRun,
    renderCpuMs: end.render.renderCpuRun,
    staleFrames: end.render.staleFrames - base.render.staleFrames,
    gappedFrames: end.render.gappedFrames - base.render.gappedFrames,
    inputsDropped: end.sim.inputsDropped - base.sim.inputsDropped,
    snapshotsDropped: end.sim.snapshotsDropped - base.sim.snapshotsDropped,
  };
}

export async function runScenario(opts: RunOptions): Promise<PerfReport> {
  const timeout = opts.statsTimeoutMs ?? 30_000;
  const total = opts.scenario.warmupS + opts.seconds;
  const startedAt = opts.now().toISOString();

  opts.onProgress?.({ phase: 'waiting', elapsedS: 0, totalS: total });
  await waitForBoth(opts, () => true, timeout, 'statistics from the sim and the renderer');

  await sleepWithProgress(opts, 'warmup', opts.scenario.warmupS);

  // Clear the run-level statistics, then take the baseline from the first reports made after that.
  const resetAt = opts.env.sharedNowMs();
  opts.target.resetRun();
  const base = await waitForBoth(
    opts,
    (s) => s.sim.atMs >= resetAt + AFTER_RESET_MS && s.render.atMs >= resetAt + AFTER_RESET_MS,
    timeout,
    'statistics after the warm-up',
  );

  await sleepWithProgress(opts, 'measure', opts.seconds);
  const end = await waitForBoth(
    opts,
    (s) => s.sim.atMs > base.sim.atMs && s.render.atMs > base.render.atMs,
    timeout,
    'closing statistics',
  );

  const metrics = computeMetrics(base, end);
  const gates = evaluateGates(metrics);
  opts.onProgress?.({ phase: 'done', elapsedS: total, totalS: total });
  return {
    schema: PERF_REPORT_SCHEMA,
    scenario: opts.scenario.id,
    startedAt,
    warmupS: opts.scenario.warmupS,
    seconds: opts.seconds,
    build: opts.build(),
    machine: opts.machine(),
    modes: opts.modes(),
    metrics,
    gates,
    verdict: verdictOf(gates),
  };
}
