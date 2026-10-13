/**
 * `?perf=ps0`: flies the scripted pilot for a fixed time, measures the loop, and shows the
 * report. The run starts as soon as the demo is up; `?seconds=N` sets the length of the measured
 * window and the warm-up comes on top of it.
 */
import { findScenario, runScenario, type PerfReport } from '@flying/perf';
import { mountPerfPanel } from '@flying/ui';
import { sharedNowMs } from '@flying/platform-web';
import type { Demo } from './demo.ts';

export interface PerfRunArgs {
  root: HTMLElement;
  demo: Demo;
  /** The scenario id from the URL (`ps0`). */
  scenarioId: string;
  seconds: number | undefined;
  onBack: () => void;
}

/** `ps0` in the URL, `PS-0` as the scenario is named. */
function scenarioName(flag: string): string {
  return flag.toLowerCase().replace(/^ps(\d)/, 'PS-$1');
}

export async function runPerfPage(args: PerfRunArgs): Promise<PerfReport | undefined> {
  const scenario = findScenario(scenarioName(args.scenarioId));
  if (scenario === undefined) {
    throw new Error(`SYS-121: there is no scenario ${args.scenarioId}`);
  }
  const panel = mountPerfPanel(args.root, { scenario: scenario.id, onBack: args.onBack });
  try {
    const report = await runScenario({
      scenario,
      seconds: args.seconds ?? scenario.defaultSeconds,
      target: args.demo.perf.target,
      env: {
        sharedNowMs,
        sleep: (ms) =>
          new Promise<void>((resolve) => {
            setTimeout(resolve, ms);
          }),
      },
      build: () => args.demo.perf.build(),
      machine: () => args.demo.perf.machine(),
      modes: () => args.demo.perf.modes(),
      now: () => new Date(),
      onProgress: (p) => {
        panel.progress(p);
      },
    });
    panel.done(report);
    return report;
  } catch (e) {
    panel.fail(e instanceof Error ? e.message : String(e));
    return undefined;
  }
}
