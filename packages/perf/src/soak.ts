/**
 * The soak's verdict (ADR-45 section 4.7, L1 to L7): pure functions over what the soak harness
 * (`tests/e2e/soak/soak.spec.ts`) sampled while the bot flew sortie after sortie in the browser.
 *
 * The harness takes a sample every 60 s of the measured window (`window.__flying.memory()`) and, at
 * each sortie start (each press of the debrief's Fly again, and the first take-off), the DOM's
 * counters after a garbage collection and the live GPU resources. A leak shows as a slope or a
 * count that will not stay put; a flat but noisy series passes.
 */

/** One sample of `window.__flying.memory()`, at `tS` seconds since the measured window began. */
export interface SoakSample {
  tS: number;
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

/** What was measured at a sortie start, after a garbage collection. */
export interface SortieStart {
  tS: number;
  /** `Memory.getDOMCounters()`: nodes and JavaScript event listeners. */
  nodes: number;
  listeners: number;
  liveBuffers: number;
  liveTextures: number;
}

export interface SoakReport {
  schema: 'soak_report/1';
  build: string;
  /** The page soaked, and where. */
  url: string;
  warmupS: number;
  seconds: number;
  /** In the measured window, oldest first. */
  samples: SoakSample[];
  sortieStarts: SortieStart[];
  consoleErrors: string[];
}

/** Which gates a run is judged by: all of them, or the quick CI check's L3, L4, L6 and L7. */
export type SoakGates = 'all' | 'quick';

export interface SoakGateResult {
  pass: boolean;
  /** The figures it was judged on, in words. */
  detail: string;
}

export type SoakVerdict = Record<
  'L1' | 'L2' | 'L3' | 'L4' | 'L5' | 'L6' | 'L7',
  SoakGateResult | null
> & {
  pass: boolean;
};

/** L1's limit: the JavaScript memory's slope, bytes per 10 minutes. */
export const L1_LIMIT_BYTES = 1_000_000;
/** L7's sim rate: on the reference machine, and in CI (ADR-28). */
export const L7_SIM_HZ = 59.5;
export const L7_SIM_HZ_CI = 57;
/** A full run must reach this many sortie starts, so L2 and L5 compare at least 4. */
export const FULL_RUN_SORTIE_STARTS = 4;

/** The least-squares slope of `v` against `t` (seconds), per 600 s; 0 for fewer than two points. */
export function slopePerTenMinutes(points: readonly { t: number; v: number }[]): number {
  const n = points.length;
  if (n < 2) return 0;
  let st = 0;
  let sv = 0;
  for (const p of points) {
    st += p.t;
    sv += p.v;
  }
  const mt = st / n;
  const mv = sv / n;
  let num = 0;
  let den = 0;
  for (const p of points) {
    num += (p.t - mt) * (p.v - mv);
    den += (p.t - mt) ** 2;
  }
  return den === 0 ? 0 : (num / den) * 600;
}

const MB = (bytes: number): string => `${(bytes / 1e6).toFixed(2)} MB`;

/** Whether every value is the same. */
function constant(values: readonly number[]): boolean {
  return values.every((v) => v === values[0]);
}

/**
 * Judges a soak. `gates: 'quick'` judges L3, L4, L6 and L7 only, without L7's sortie count (a short
 * CI run is a check of the harness, not of the leaks). `ci` takes L7's CI rate.
 */
export function soakVerdict(
  report: SoakReport,
  options: { gates?: SoakGates; ci?: boolean } = {},
): SoakVerdict {
  const quick = options.gates === 'quick';
  const s = report.samples;
  const starts = report.sortieStarts;
  const firstStart = starts[0]?.tS ?? Number.NEGATIVE_INFINITY;
  const afterStart = s.filter((x) => x.tS >= firstStart);
  const last = s.at(-1);

  const l1 = (): SoakGateResult => {
    const slope = slopePerTenMinutes(s.map((x) => ({ t: x.tS, v: x.jsBytes })));
    const source = s[0]?.jsSource ?? 'heap';
    return {
      pass: s.length >= 2 && slope <= L1_LIMIT_BYTES,
      detail: `${MB(slope)} per 10 min (${source}, ${String(s.length)} samples; limit ${MB(L1_LIMIT_BYTES)})`,
    };
  };

  const l2 = (): SoakGateResult => {
    const second = starts[1];
    const end = starts.at(-1);
    if (second === undefined || end === undefined || starts.length < 3) {
      return { pass: false, detail: `${String(starts.length)} sortie starts: need at least 3` };
    }
    return {
      pass: end.nodes <= second.nodes + 20 && end.listeners <= second.listeners + 2,
      detail: `nodes ${String(second.nodes)} -> ${String(end.nodes)}, listeners ${String(second.listeners)} -> ${String(end.listeners)}`,
    };
  };

  const l3 = (): SoakGateResult => {
    const bytes = afterStart.map((x) => x.wasmBytes);
    return {
      pass: bytes.length > 0 && constant(bytes),
      detail: `WebAssembly memory ${[...new Set(bytes)].map(MB).join(', ') || 'not sampled'}`,
    };
  };

  const l4 = (): SoakGateResult => {
    const workers = s.map((x) => x.workers);
    const bad = last === undefined ? 1 : last.silences + last.workerErrors + last.deviceLosses;
    return {
      pass: last !== undefined && bad === 0 && constant(workers),
      detail:
        last === undefined
          ? 'no samples'
          : `${String(last.silences)} silences, ${String(last.workerErrors)} worker errors, ${String(last.deviceLosses)} device losses, workers ${[...new Set(workers)].join('/')}`,
    };
  };

  const l5 = (): SoakGateResult => {
    const later = starts.slice(1);
    if (later.length < 2) {
      return { pass: false, detail: `${String(starts.length)} sortie starts: need at least 3` };
    }
    const buffers = later.map((x) => x.liveBuffers);
    const textures = later.map((x) => x.liveTextures);
    return {
      pass: constant(buffers) && constant(textures),
      detail: `buffers ${buffers.join(', ')}; textures ${textures.join(', ')}`,
    };
  };

  const l6 = (): SoakGateResult => {
    const nodes = afterStart.map((x) => x.liveAudioNodes);
    return {
      pass: nodes.length > 0 && constant(nodes),
      detail: `audio nodes ${[...new Set(nodes)].join(', ') || 'not sampled'}`,
    };
  };

  const l7 = (): SoakGateResult => {
    const hz = s.length === 0 ? 0 : s.reduce((sum, x) => sum + x.simHz, 0) / s.length;
    const want = options.ci === true ? L7_SIM_HZ_CI : L7_SIM_HZ;
    const enough = quick || starts.length >= FULL_RUN_SORTIE_STARTS;
    return {
      pass: hz >= want && report.consoleErrors.length === 0 && enough,
      detail: `${hz.toFixed(2)} Hz (need ${String(want)}), ${String(report.consoleErrors.length)} console errors, ${String(starts.length)} sortie starts${quick ? '' : ` (need ${String(FULL_RUN_SORTIE_STARTS)})`}`,
    };
  };

  const v: SoakVerdict = {
    L1: quick ? null : l1(),
    L2: quick ? null : l2(),
    L3: l3(),
    L4: l4(),
    L5: quick ? null : l5(),
    L6: l6(),
    L7: l7(),
    pass: false,
  };
  v.pass = (['L1', 'L2', 'L3', 'L4', 'L5', 'L6', 'L7'] as const).every((k) => v[k]?.pass !== false);
  return v;
}
