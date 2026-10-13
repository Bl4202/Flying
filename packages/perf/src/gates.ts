/**
 * The PS-0 gates. They are the budgets of GDD 60.2 that already apply to a loop with nothing in
 * it: the sim must hold its rate and its per-tick cost, and the renderer's CPU cost must stay
 * inside its share of the frame. Strict, so they are checked on the reference machines; CI
 * machines are too noisy for them and only look at the rate (ADR-28).
 */
import type { GateResult, PerfMetrics } from './report.ts';

/** Strict limits. */
export const PS0_LIMITS = {
  simHz: 60,
  simHzTolerance: 0.1,
  droppedMs: 0,
  stepP99Ms: 7,
  stepAvgMs: 4,
  renderCpuAvgMs: 7,
} as const;

/** Rounding slack, so a rate of exactly 60.1 passes the ± 0.1 gate despite float arithmetic. */
const EPSILON = 1e-9;

/** The rate CI machines must still reach. */
export const CI_MIN_SIM_HZ = 57;

export function evaluateGates(m: PerfMetrics): GateResult[] {
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

export function verdictOf(gates: readonly GateResult[]): 'pass' | 'fail' {
  return gates.every((g) => g.pass) ? 'pass' : 'fail';
}
