/**
 * The parts of `perf-local.ts` that decide things: reading its arguments, judging a run against
 * the reference-machine pass criteria, and writing the summary. Kept free of the browser so they
 * can be tested.
 */

export type MachineClass = 'minimum' | 'recommended' | 'high';
export type RendererArg = 'auto' | 'webgl2' | 'webgpu';

export interface LocalOptions {
  /** Directory name under `out`, e.g. `m4-webgpu`. */
  name: string;
  /** Base URL of the build, with a trailing slash. */
  base: string;
  renderer: RendererArg;
  /** CPU throttling factor for the page (1 = none). */
  throttle: number;
  /** Seconds of PS-0 measurement. */
  seconds: number;
  /** Seconds of scripted flying before the benchmark. */
  fly: number;
  headless: boolean;
  out: string;
  machineClass: MachineClass;
  /** Where the scripted flight starts: the air, or the runway the game starts on. */
  spawn: SpawnArg;
  /** Control-latency samples to take after the flight (AC-58.3); 0 skips the measurement. */
  latency: number;
  /** The scenario the measured run flies. */
  scenario: ScenarioArg;
  /** The camera of an arena run; the scenario's own (the cockpit) when absent. */
  camera: CameraArg | undefined;
  /** The drawing-buffer size of an arena run, `WxH`; the scenario's own (1920x1080) when absent. */
  res: string | undefined;
  /** The graphics preset of an arena run; the scenario's own (High) when absent. */
  quality: 'low' | 'medium' | 'high' | undefined;
}

export type SpawnArg = 'air' | 'runway';
/** The scenario the measured run flies (`?perf=`): the bare loop, or the arena circuit (v0.1c). */
export type ScenarioArg = 'ps0' | 'arena';
export type CameraArg = 'cockpit' | 'chase';

/** The gate of AC-58.3: the 95th percentile of the control latency, ms (strict, reference machine). */
export const LATENCY_P95_MS = 25;

export const DEFAULT_BASE = 'https://bl4202.github.io/Flying/';

const USAGE = `usage: node perf-local.ts --name NAME --class minimum|recommended|high
  [--base URL] [--renderer auto|webgl2|webgpu] [--throttle N] [--seconds N] [--fly N]
  [--spawn air|runway] [--latency SAMPLES] [--headless] [--out DIR]
  [--scenario ps0|arena] [--camera cockpit|chase] [--res WxH] [--quality low|medium|high]`;

export function parseLocalArgs(argv: readonly string[]): LocalOptions {
  const opts: LocalOptions = {
    name: '',
    base: DEFAULT_BASE,
    renderer: 'auto',
    throttle: 1,
    seconds: 120,
    fly: 60,
    headless: false,
    out: 'docs/exit/v0.1c/machines',
    machineClass: 'recommended',
    spawn: 'air',
    latency: 0,
    scenario: 'ps0',
    camera: undefined,
    res: undefined,
    quality: undefined,
  };
  let sawClass = false;
  let sawFly = false;
  for (let i = 0; i < argv.length; i++) {
    const flag = argv[i];
    const value = (): string => {
      const v = argv[++i];
      if (v === undefined) throw new Error(`${flag ?? ''} needs a value\n${USAGE}`);
      return v;
    };
    const number = (min: number, max: number): number => {
      const n = Number(value());
      if (!Number.isFinite(n) || n < min || n > max) {
        throw new Error(`${flag ?? ''} must be a number from ${String(min)} to ${String(max)}`);
      }
      return n;
    };
    switch (flag) {
      case '--name':
        opts.name = value();
        break;
      case '--base': {
        const b = value();
        opts.base = b.endsWith('/') ? b : `${b}/`;
        break;
      }
      case '--renderer': {
        const r = value();
        if (r !== 'auto' && r !== 'webgl2' && r !== 'webgpu') {
          throw new Error(`--renderer must be auto, webgl2 or webgpu, not ${r}`);
        }
        opts.renderer = r;
        break;
      }
      case '--class': {
        const c = value();
        if (c !== 'minimum' && c !== 'recommended' && c !== 'high') {
          throw new Error(`--class must be minimum, recommended or high, not ${c}`);
        }
        opts.machineClass = c;
        sawClass = true;
        break;
      }
      case '--throttle':
        opts.throttle = number(1, 20);
        break;
      case '--seconds':
        opts.seconds = Math.round(number(1, 3600));
        break;
      case '--fly':
        opts.fly = Math.round(number(0, 3600));
        sawFly = true;
        break;
      case '--spawn': {
        const sp = value();
        if (sp !== 'air' && sp !== 'runway') {
          throw new Error(`--spawn must be air or runway, not ${sp}`);
        }
        opts.spawn = sp;
        break;
      }
      case '--latency':
        opts.latency = Math.round(number(1, 100_000));
        break;
      case '--scenario': {
        const sc = value();
        if (sc !== 'ps0' && sc !== 'arena') {
          throw new Error(`--scenario must be ps0 or arena, not ${sc}`);
        }
        opts.scenario = sc;
        break;
      }
      case '--camera': {
        const cam = value();
        if (cam !== 'cockpit' && cam !== 'chase') {
          throw new Error(`--camera must be cockpit or chase, not ${cam}`);
        }
        opts.camera = cam;
        break;
      }
      case '--res': {
        const r = value();
        if (!/^\d{3,5}x\d{3,5}$/.test(r))
          throw new Error(`--res must be WxH, like 1920x1080, not ${r}`);
        opts.res = r;
        break;
      }
      case '--quality': {
        const qv = value();
        if (qv !== 'low' && qv !== 'medium' && qv !== 'high') {
          throw new Error(`--quality must be low, medium or high, not ${qv}`);
        }
        opts.quality = qv;
        break;
      }
      case '--out':
        opts.out = value();
        break;
      case '--headless':
        opts.headless = true;
        break;
      default:
        throw new Error(`unknown argument ${flag ?? ''}\n${USAGE}`);
    }
  }
  if (!/^[a-z0-9][a-z0-9-]*$/.test(opts.name)) {
    throw new Error(`--name is required: lower-case letters, digits and dashes\n${USAGE}`);
  }
  if (!sawClass) {
    throw new Error(`--class is required\n${USAGE}`);
  }
  // The arena run flies its own scripted circuit; the hands-on manoeuvre before it is for PS-0 runs.
  if (opts.scenario === 'arena' && !sawFly) opts.fly = 0;
  return opts;
}

/** What the scripted flight measured. */
export interface FlightSummary {
  seconds: number;
  fpsAvg: number;
  fpsMin: number;
  frameP99Ms: number;
  frameMaxMs: number;
  simHzAvg: number;
  /** Distinct toasts shown during the flight (a lost graphics device raises one). */
  toasts: string[];
}

/** The control latency the sim measured over the run, ms (AC-58.3). */
export interface LatencySummary {
  count: number;
  avg: number;
  p95: number;
  p99: number;
  max: number;
}

export interface RunFacts {
  /** The scenario the measured run flies, as the report names it: `PS-0` or `ARENA`. */
  scenario: string;
  compatVerdict: 'ok' | 'degraded' | 'fatal' | null;
  compatCodes: string[];
  adapter: string;
  renderer: string;
  /** What `requestAnimationFrame` ran at, Hz: the display rate the frame rate is judged against. */
  displayHz: number;
  flight: FlightSummary | null;
  perf: {
    verdict: 'pass' | 'fail';
    simHz: number;
    renderFps: number;
    failedGates: string[];
    /** Times the graphics device was lost in the measured window (the report's own count). */
    deviceLosses: number;
  } | null;
  benchmark: { profile: string; preset: string } | null;
  consoleErrors: string[];
  /** Whether the scripted minute was asked for (`--fly` above 0). */
  flightWanted: boolean;
  /** Samples the run was asked for (0: no latency measurement was asked for). */
  latencyWanted: number;
  latency: LatencySummary | null;
}

export interface Check {
  id: string;
  text: string;
  pass: boolean;
}

/** The pass criteria of the reference-machine runs (plan, "Reference machines"). */
export function judgeRun(facts: RunFacts, cls: MachineClass): { checks: Check[]; pass: boolean } {
  const checks: Check[] = [];
  const add = (id: string, text: string, pass: boolean): void => {
    checks.push({ id, text, pass });
  };

  add(
    'compat',
    `compat verdict is not fatal (${facts.compatVerdict ?? 'no screen shown'})`,
    facts.compatVerdict !== 'fatal',
  );
  add(
    'ps0',
    facts.perf === null
      ? `${facts.scenario} strict gates hold (the run did not finish)`
      : `${facts.scenario} strict gates hold${facts.perf.failedGates.length > 0 ? ` (failed: ${facts.perf.failedGates.join(', ')})` : ''}`,
    facts.perf?.verdict === 'pass',
  );
  const lost = facts.flight?.toasts.filter((t) => /lost/i.test(t)) ?? [];
  add(
    'clean',
    'frames render without errors or device loss',
    facts.consoleErrors.length === 0 &&
      lost.length === 0 &&
      (facts.perf?.deviceLosses ?? 0) === 0 &&
      // The scripted minute happened if it was asked for (an arena run has none by default).
      (!facts.flightWanted || facts.flight !== null),
  );
  if (facts.latencyWanted > 0) {
    const l = facts.latency;
    add(
      'latency',
      l === null
        ? `control latency p95 within ${String(LATENCY_P95_MS)} ms (not measured)`
        : `control latency p95 ${l.p95.toFixed(2)} ms within ${String(LATENCY_P95_MS)} ms, over ${String(l.count)} changes (at least ${String(facts.latencyWanted)})`,
      l !== null && l.count >= facts.latencyWanted && l.p95 <= LATENCY_P95_MS,
    );
  }
  const fps = facts.perf?.renderFps ?? facts.flight?.fpsAvg ?? 0;
  if (cls === 'minimum') {
    add('fps', `at least 30 FPS on a Minimum-class run (${fps.toFixed(1)})`, fps >= 30);
  } else {
    add(
      'fps',
      `FPS at display rate (${fps.toFixed(1)} of ${facts.displayHz.toFixed(0)} Hz)`,
      facts.displayHz > 0 && fps >= 0.95 * facts.displayHz,
    );
  }
  return { checks, pass: checks.every((c) => c.pass) };
}

/** The file the measured run's report is saved to. */
export function reportFile(opts: LocalOptions): string {
  return `perf-${opts.scenario}.json`;
}

/** What an arena run was looking through, for the summary: `, cockpit, 1920x1080, high`. */
function scenarioView(opts: LocalOptions): string {
  if (opts.scenario !== 'arena') return '';
  return `, ${opts.camera ?? 'cockpit'}, ${opts.res ?? '1920x1080'}, ${opts.quality ?? 'high'}`;
}

/** The query of the measured run's page, past `perf=`: its length and, for the arena, what it shows. */
export function scenarioQuery(opts: LocalOptions): string {
  const parts = [`perf=${opts.scenario}`, `seconds=${String(opts.seconds)}`];
  if (opts.camera !== undefined) parts.push(`camera=${opts.camera}`);
  if (opts.res !== undefined) parts.push(`res=${opts.res}`);
  if (opts.quality !== undefined) parts.push(`quality=${opts.quality}`);
  return parts.join('&');
}

export function renderSummary(
  opts: LocalOptions,
  facts: RunFacts,
  judgement: { checks: Check[]; pass: boolean },
  when: string,
): string {
  const f = facts.flight;
  const p = facts.perf;
  const lines = [
    `# Run ${opts.name}`,
    '',
    `Machine class: **${opts.machineClass}**. ${when}. Build: ${opts.base}`,
    '',
    `- Renderer requested: \`${opts.renderer}\`; used: ${facts.renderer || 'unknown'}`,
    `- Adapter: ${facts.adapter || 'none reported'}`,
    `- CPU throttling: ${opts.throttle === 1 ? 'none' : `${String(opts.throttle)}x (main thread only; workers are not throttled)`}`,
    `- Display rate (requestAnimationFrame): ${facts.displayHz.toFixed(0)} Hz`,
    `- Compat verdict: ${facts.compatVerdict ?? 'screen not shown'}${facts.compatCodes.length > 0 ? ` (${facts.compatCodes.join(', ')})` : ''}`,
    f === null
      ? '- Flight: not run'
      : `- Flight (${String(f.seconds)} s scripted, ${opts.spawn} start): ${f.fpsAvg.toFixed(1)} FPS average, ${f.fpsMin.toFixed(1)} minimum, frame p99 ${f.frameP99Ms.toFixed(1)} ms, longest frame ${f.frameMaxMs.toFixed(1)} ms, sim ${f.simHzAvg.toFixed(2)} Hz`,
    p === null
      ? `- ${facts.scenario}: did not finish`
      : `- ${facts.scenario} (${String(opts.seconds)} s${scenarioView(opts)}): ${p.verdict.toUpperCase()}, sim ${p.simHz.toFixed(3)} Hz, render ${p.renderFps.toFixed(1)} FPS`,
    opts.latency === 0
      ? '- Control latency: not measured'
      : facts.latency === null
        ? '- Control latency: did not finish'
        : `- Control latency (${String(facts.latency.count)} changes): avg ${facts.latency.avg.toFixed(2)} ms, p95 ${facts.latency.p95.toFixed(2)} ms, p99 ${facts.latency.p99.toFixed(2)} ms, max ${facts.latency.max.toFixed(2)} ms, at ${facts.displayHz.toFixed(0)} Hz`,
    facts.benchmark === null
      ? '- Benchmark: not run'
      : `- Benchmark: profile ${facts.benchmark.profile}, preset ${facts.benchmark.preset}`,
    `- Console errors: ${facts.consoleErrors.length === 0 ? 'none' : String(facts.consoleErrors.length)}`,
    '',
    `## Pass criteria: ${judgement.pass ? 'PASS' : 'FAIL'}`,
    '',
    ...judgement.checks.map((c) => `- [${c.pass ? 'x' : ' '}] ${c.text}`),
  ];
  if (facts.consoleErrors.length > 0) {
    lines.push('', '## Console errors', '', ...facts.consoleErrors.map((e) => `- ${e}`));
  }
  if (f !== null && f.toasts.length > 0) {
    lines.push('', '## Notices shown during the flight', '', ...f.toasts.map((t) => `- ${t}`));
  }
  lines.push(
    '',
    `Raw reports: \`compat.json\`, \`flight.json\`, \`latency.json\`, \`benchmark.json\`, \`${reportFile(opts)}\`.`,
    '',
  );
  return lines.join('\n');
}
