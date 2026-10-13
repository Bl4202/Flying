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
}

export const DEFAULT_BASE = 'https://bl4202.github.io/Flying/';

const USAGE = `usage: node perf-local.ts --name NAME --class minimum|recommended|high
  [--base URL] [--renderer auto|webgl2|webgpu] [--throttle N] [--seconds N] [--fly N]
  [--headless] [--out DIR]`;

export function parseLocalArgs(argv: readonly string[]): LocalOptions {
  const opts: LocalOptions = {
    name: '',
    base: DEFAULT_BASE,
    renderer: 'auto',
    throttle: 1,
    seconds: 120,
    fly: 60,
    headless: false,
    out: 'docs/exit/v0.0b/machines',
    machineClass: 'recommended',
  };
  let sawClass = false;
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
        break;
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

export interface RunFacts {
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
  } | null;
  benchmark: { profile: string; preset: string } | null;
  consoleErrors: string[];
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
      ? 'PS-0 strict gates hold (the run did not finish)'
      : `PS-0 strict gates hold${facts.perf.failedGates.length > 0 ? ` (failed: ${facts.perf.failedGates.join(', ')})` : ''}`,
    facts.perf?.verdict === 'pass',
  );
  const lost = facts.flight?.toasts.filter((t) => /lost/i.test(t)) ?? [];
  add(
    'clean',
    'frames render without errors or device loss',
    facts.consoleErrors.length === 0 && lost.length === 0 && facts.flight !== null,
  );
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
      : `- Flight (${String(f.seconds)} s scripted): ${f.fpsAvg.toFixed(1)} FPS average, ${f.fpsMin.toFixed(1)} minimum, frame p99 ${f.frameP99Ms.toFixed(1)} ms, longest frame ${f.frameMaxMs.toFixed(1)} ms, sim ${f.simHzAvg.toFixed(2)} Hz`,
    p === null
      ? '- PS-0: did not finish'
      : `- PS-0 (${String(opts.seconds)} s): ${p.verdict.toUpperCase()}, sim ${p.simHz.toFixed(3)} Hz, render ${p.renderFps.toFixed(1)} FPS`,
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
    'Raw reports: `compat.json`, `flight.json`, `benchmark.json`, `perf-ps0.json`.',
    '',
  );
  return lines.join('\n');
}
