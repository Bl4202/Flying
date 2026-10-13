/**
 * The performance scenarios (GDD 60.9). v0.0b has PS-0 only: the empty loop, which measures the
 * skeleton so later milestones can see what each system adds. PS-1 onwards need content that does
 * not exist yet.
 */
import { INPUT_SCRIPT } from '@flying/sim-bridge/layout';

export interface Scenario {
  id: string;
  name: string;
  /** Seconds before the measured window; statistics are cleared after it. */
  warmupS: number;
  /** Length of the measured window unless the URL asks for another. */
  defaultSeconds: number;
  /** The in-core pilot (`INPUT_SCRIPT`), so no person is needed and every run flies the same way. */
  script: number;
}

export const PS0: Scenario = {
  id: 'PS-0',
  name: 'Empty loop',
  warmupS: 3,
  defaultSeconds: 60,
  script: INPUT_SCRIPT.ERRATIC,
};

export const SCENARIOS: readonly Scenario[] = [PS0];

export function findScenario(id: string): Scenario | undefined {
  return SCENARIOS.find((s) => s.id.toLowerCase() === id.toLowerCase());
}
