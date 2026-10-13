/**
 * URL flags: switches for debugging, testing and measuring, parsed from the query string.
 *
 * Unknown keys are ignored and bad values fall back to the default, each with a warning, so a
 * mistyped flag is visible rather than silently a different configuration.
 *
 * | Flag | Values |
 * |---|---|
 * | `debug` | `1` (overlay), `perf` (overlay plus the perf panel) |
 * | `renderer` | `auto` (default), `webgpu`, `webgl2`, `null` |
 * | `flags` | comma list of `transfer`, `mainrender`, `timer`, `stall` (test switches) |
 * | `compat` | `1`: show the compat screen even if it was acknowledged |
 * | `selftest` | `1` (golden hash), `renderer` (renderer contract) |
 * | `perf` | `ps0`: run the PS-0 scenario |
 * | `seconds` | duration of a perf run, 1 to 3600 |
 * | `bench` | `quick` or `full`: run the first-run benchmark |
 */

export type RendererChoice = 'auto' | 'webgpu' | 'webgl2' | 'null';

export interface Flags {
  debug: false | 'overlay' | 'perf';
  renderer: RendererChoice;
  /** Force the degraded snapshot channel (transferable buffers) even when isolated. */
  transfer: boolean;
  /** Run the render loop on the main thread. */
  mainrender: boolean;
  /** Force the timer sleeper in the Sim Worker (no `Atomics.wait`). */
  timer: boolean;
  /** Expose the stall hooks that tests use to freeze a thread. */
  stall: boolean;
  compat: boolean;
  selftest: false | 'hash' | 'renderer';
  perf: false | 'ps0';
  seconds: number | undefined;
  bench: false | 'quick' | 'full';
  /** Problems found while parsing, in order. */
  warnings: string[];
}

const RENDERERS: readonly RendererChoice[] = ['auto', 'webgpu', 'webgl2', 'null'];
const SWITCHES = ['transfer', 'mainrender', 'timer', 'stall'] as const;
const KNOWN = new Set([
  'debug',
  'renderer',
  'flags',
  'compat',
  'selftest',
  'perf',
  'seconds',
  'bench',
]);

export function defaultFlags(): Flags {
  return {
    debug: false,
    renderer: 'auto',
    transfer: false,
    mainrender: false,
    timer: false,
    stall: false,
    compat: false,
    selftest: false,
    perf: false,
    seconds: undefined,
    bench: false,
    warnings: [],
  };
}

/** Parses `location.search` (with or without the leading `?`). */
export function parseFlags(search: string): Flags {
  const flags = defaultFlags();
  const params = new URLSearchParams(search);
  const bad = (key: string, value: string, allowed: string): void => {
    flags.warnings.push(`ignored ?${key}=${value} (use ${allowed})`);
  };

  for (const key of new Set(params.keys())) {
    if (!KNOWN.has(key)) {
      flags.warnings.push(`unknown flag ?${key}`);
    }
  }

  const debug = params.get('debug');
  if (debug === '1') {
    flags.debug = 'overlay';
  } else if (debug === 'perf') {
    flags.debug = 'perf';
  } else if (debug !== null) {
    bad('debug', debug, '1 or perf');
  }

  const renderer = params.get('renderer');
  if (renderer !== null) {
    const choice = RENDERERS.find((r) => r === renderer);
    if (choice === undefined) {
      bad('renderer', renderer, RENDERERS.join(', '));
    } else {
      flags.renderer = choice;
    }
  }

  const list = params.get('flags');
  if (list !== null) {
    for (const item of list.split(',').filter((s) => s !== '')) {
      const name = SWITCHES.find((s) => s === item);
      if (name === undefined) {
        bad('flags', item, SWITCHES.join(', '));
      } else {
        flags[name] = true;
      }
    }
  }

  const compat = params.get('compat');
  if (compat === '1') {
    flags.compat = true;
  } else if (compat !== null) {
    bad('compat', compat, '1');
  }

  const selftest = params.get('selftest');
  if (selftest === '1') {
    flags.selftest = 'hash';
  } else if (selftest === 'renderer') {
    flags.selftest = 'renderer';
  } else if (selftest !== null) {
    bad('selftest', selftest, '1 or renderer');
  }

  const perf = params.get('perf');
  if (perf === 'ps0') {
    flags.perf = 'ps0';
  } else if (perf !== null) {
    bad('perf', perf, 'ps0');
  }

  const seconds = params.get('seconds');
  if (seconds !== null) {
    const n = Number(seconds);
    if (Number.isInteger(n) && n >= 1 && n <= 3600) {
      flags.seconds = n;
    } else {
      bad('seconds', seconds, 'a whole number from 1 to 3600');
    }
  }

  const bench = params.get('bench');
  if (bench === 'quick' || bench === 'full') {
    flags.bench = bench;
  } else if (bench !== null) {
    bad('bench', bench, 'quick or full');
  }

  return flags;
}
