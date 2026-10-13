/**
 * `perf_report/1`: what one performance run produces and what gets pasted into a bug report or
 * committed under `docs/exit/`. Plain JSON, no functions, so it survives `JSON.stringify`.
 *
 * Wall-clock numbers here describe a run; they never feed simulation state or hashes.
 */
import type { Quantiles } from '@flying/sim-bridge';

export const PERF_REPORT_SCHEMA = 'perf_report/1';

export interface MachineInfo {
  userAgent: string;
  platform: string;
  /** Logical processors, as `navigator.hardwareConcurrency` says. */
  hardwareConcurrency: number;
  /** `navigator.deviceMemory` in GB (Chrome rounds it and stops at 8), or null where absent. */
  deviceMemoryGb: number | null;
  /** Screen size in CSS pixels and the device pixel ratio, e.g. `1728x1117@2`. */
  screen: string;
  /** The WebGPU adapter as the browser describes it, or empty. */
  adapter: string;
  /** The backend and device string the renderer reported. */
  renderer: string;
}

export interface RunModes {
  variant: string;
  snapshotChannel: string;
  inputChannel: string;
  renderHost: string;
  sleeper: string;
  renderer: string;
}

export interface BuildInfo {
  id: string;
  abi: number;
  contentHash: string;
}

export interface PerfMetrics {
  /** Length of the measured window, from the sim's own report timestamps. */
  windowS: number;
  ticks: number;
  simHz: number;
  /** Simulated time given up in the window, and how many separate times. */
  droppedMs: number;
  droppedEvents: number;
  /** Per-tick cost in the Sim Worker, milliseconds, since the warm-up ended. */
  step: Quantiles;
  frames: number;
  renderFps: number;
  /** Time between frames since the warm-up ended. */
  frameMs: Quantiles;
  /** CPU time inside `render`, since the warm-up ended. */
  renderCpuMs: Quantiles;
  staleFrames: number;
  gappedFrames: number;
  inputsDropped: number;
  snapshotsDropped: number;
}

export interface GateResult {
  id: string;
  /** What is required, in words. */
  rule: string;
  value: number;
  /** The limit the value is compared with. */
  limit: number;
  pass: boolean;
}

export interface PerfReport {
  schema: typeof PERF_REPORT_SCHEMA;
  scenario: string;
  /** ISO 8601, UTC. */
  startedAt: string;
  warmupS: number;
  seconds: number;
  build: BuildInfo;
  machine: MachineInfo;
  modes: RunModes;
  metrics: PerfMetrics;
  /** The strict gates (GDD 60.2 budgets), always evaluated; `verdict` is their conjunction. */
  gates: GateResult[];
  verdict: 'pass' | 'fail';
}

const QUANTILE_KEYS = ['count', 'avg', 'p50', 'p95', 'p99', 'max'] as const;

function isQuantiles(v: unknown): v is Quantiles {
  return (
    typeof v === 'object' &&
    v !== null &&
    QUANTILE_KEYS.every((k) => typeof (v as Record<string, unknown>)[k] === 'number')
  );
}

/** Whether `v` has the shape of a `perf_report/1` (it does not re-check the gate arithmetic). */
export function isPerfReport(v: unknown): v is PerfReport {
  if (typeof v !== 'object' || v === null) return false;
  const r = v as Record<string, unknown>;
  const m = r['metrics'] as Record<string, unknown> | undefined;
  return (
    r['schema'] === PERF_REPORT_SCHEMA &&
    typeof r['scenario'] === 'string' &&
    typeof r['startedAt'] === 'string' &&
    typeof r['seconds'] === 'number' &&
    (r['verdict'] === 'pass' || r['verdict'] === 'fail') &&
    Array.isArray(r['gates']) &&
    typeof r['build'] === 'object' &&
    typeof r['machine'] === 'object' &&
    typeof r['modes'] === 'object' &&
    m !== undefined &&
    typeof m['simHz'] === 'number' &&
    typeof m['renderFps'] === 'number' &&
    isQuantiles(m['step']) &&
    isQuantiles(m['renderCpuMs']) &&
    isQuantiles(m['frameMs'])
  );
}
