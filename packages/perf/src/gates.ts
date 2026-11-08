/**
 * The PS-0 gates. They are the budgets of GDD 60.2 that already apply to a loop with nothing in
 * it: the sim must hold its rate and its per-tick cost, and the renderer's CPU cost must stay
 * inside its share of the frame. Strict, so they are checked on the reference machines; CI
 * machines are too noisy for them and only look at the rate (ADR-28).
 *
 * The arena run (v0.1c, ADR-39 P1 and P2) adds the frame rate the exit is about, the renderer's tail,
 * and the two things that must not happen while flying: a lost graphics device and a main-thread
 * task over 50 ms.
 */
import type { GateResult, PerfMetrics } from './report.ts';
import type { GateSet } from './scenarios.ts';

/** Strict limits. */
export const PS0_LIMITS = {
  simHz: 60,
  simHzTolerance: 0.1,
  droppedMs: 0,
  stepP99Ms: 7,
  stepAvgMs: 4,
  renderCpuAvgMs: 7,
} as const;

/** Strict limits of the arena run, on top of PS-0's. */
export const ARENA_LIMITS = {
  /** An average of 60 FPS: the frame interval's average, ms. */
  frameAvgMs: 16.7,
  /** A 1 % low of 45 FPS: the frame interval's 99th percentile, ms. */
  frameP99Ms: 22.2,
  /** The renderer's CPU time per frame, 99th percentile, ms. */
  renderCpuP99Ms: 11,
  /** The frame rate a Minimum-class machine must hold (run B), as a reading only. */
  minimumFps: 30,
} as const;

/** Rounding slack, so a rate of exactly 60.1 passes the ± 0.1 gate despite float arithmetic. */
const EPSILON = 1e-9;

/** The rate CI machines must still reach. */
export const CI_MIN_SIM_HZ = 57;

export function evaluateGates(m: PerfMetrics, set: GateSet = 'ps0'): GateResult[] {
  const gates = ps0Gates(m);
  return set === 'arena' ? [...gates, ...arenaGates(m)] : gates;
}

function ps0Gates(m: PerfMetrics): GateResult[] {
  const L = PS0_LIMITS;
  return [
    {
      id: 'sim-rate',
      rule: `sim rate ${String(L.simHz)} ± ${String(L.simHzTolerance)} Hz`,
      value: m.simHz,
      limit: L.simHz,
      pass: Math.abs(m.simHz - L.simHz) <= L.simHzTolerance + EPSILON,
    },
    {
      id: 'dropped',
      rule: 'no simulated time dropped after warm-up',
      value: m.droppedMs,
      limit: L.droppedMs,
      pass: m.droppedMs <= L.droppedMs,
    },
    {
      id: 'step-p99',
      rule: `tick cost p99 ≤ ${String(L.stepP99Ms)} ms`,
      value: m.step.p99,
      limit: L.stepP99Ms,
      pass: m.step.count > 0 && m.step.p99 <= L.stepP99Ms,
    },
    {
      id: 'step-avg',
      rule: `tick cost average ≤ ${String(L.stepAvgMs)} ms`,
      value: m.step.avg,
      limit: L.stepAvgMs,
      pass: m.step.count > 0 && m.step.avg <= L.stepAvgMs,
    },
    {
      id: 'render-cpu',
      rule: `render CPU average ≤ ${String(L.renderCpuAvgMs)} ms`,
      value: m.renderCpuMs.avg,
      limit: L.renderCpuAvgMs,
      pass: m.renderCpuMs.count > 0 && m.renderCpuMs.avg <= L.renderCpuAvgMs,
    },
  ];
}

function arenaGates(m: PerfMetrics): GateResult[] {
  const A = ARENA_LIMITS;
  return [
    {
      id: 'render-cpu-p99',
      rule: `render CPU p99 ≤ ${String(A.renderCpuP99Ms)} ms`,
      value: m.renderCpuMs.p99,
      limit: A.renderCpuP99Ms,
      pass: m.renderCpuMs.count > 0 && m.renderCpuMs.p99 <= A.renderCpuP99Ms,
    },
    {
      id: 'frame-avg',
      rule: `frame interval average ≤ ${String(A.frameAvgMs)} ms (60 FPS)`,
      value: m.frameMs.avg,
      limit: A.frameAvgMs,
      pass: m.frameMs.count > 0 && m.frameMs.avg <= A.frameAvgMs,
    },
    {
      id: 'frame-p99',
      rule: `frame interval p99 ≤ ${String(A.frameP99Ms)} ms (1 % low 45 FPS)`,
      value: m.frameMs.p99,
      limit: A.frameP99Ms,
      pass: m.frameMs.count > 0 && m.frameMs.p99 <= A.frameP99Ms,
    },
    {
      id: 'device-loss',
      rule: 'the graphics device is not lost',
      value: m.deviceLosses,
      limit: 0,
      pass: m.deviceLosses === 0,
    },
    {
      id: 'long-tasks',
      rule: 'no main-thread task over 50 ms after warm-up',
      value: m.longTasks ?? Number.NaN,
      limit: 0,
      // Where the browser does not report long tasks the reading is missing, not good.
      pass: m.longTasks === 0,
      ...(m.longTasks === null ? { informational: true } : {}),
    },
    {
      id: 'fps-minimum',
      rule: `at least ${String(A.minimumFps)} FPS (the Minimum-class reading, run B)`,
      value: m.renderFps,
      limit: A.minimumFps,
      pass: m.renderFps >= A.minimumFps,
      informational: true,
    },
  ];
}

export function verdictOf(gates: readonly GateResult[]): 'pass' | 'fail' {
  return gates.every((g) => g.pass || g.informational === true) ? 'pass' : 'fail';
}
