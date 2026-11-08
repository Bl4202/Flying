/**
 * The performance scenarios (GDD 60.9). PS-0 is the empty loop, which measures the skeleton so later
 * milestones can see what each system adds. `ARENA` (v0.1c) is the first with something in it: the
 * circuit flown from the runway over the rugged arena with the sky, the terrain to the horizon, the
 * HUD and the sound on, which is what the v0.1c exit's 60 FPS is measured on. PS-1 onwards need
 * content that does not exist yet.
 */
import { ARENA as ARENAS, INPUT_SCRIPT, SPAWN_KIND } from '@flying/sim-bridge/layout';

/** Which set of gates a scenario is judged by (`gates.ts`). */
export type GateSet = 'ps0' | 'arena';

export interface Scenario {
  id: string;
  name: string;
  /** Seconds before the measured window; statistics are cleared after it. */
  warmupS: number;
  /** Length of the measured window unless the URL asks for another. */
  defaultSeconds: number;
  /** The in-core pilot (`INPUT_SCRIPT`), so no person is needed and every run flies the same way. */
  script: number;
  /** The sortie the scripted pilot flies: where it starts and in which arena. */
  start: { spawn: number; arena: number };
  /** The gates the run is judged by. */
  gates: GateSet;
  /**
   * What the page shows while it runs: the camera, the graphics preset and a fixed drawing-buffer
   * size, each unless the URL says otherwise. Absent for a scenario that measures the bare loop.
   */
  view?: {
    camera: 'cockpit' | 'chase';
    quality: 'low' | 'medium' | 'high';
    res: { width: number; height: number };
    /** The sound runs: the audio context is resumed and fed (a run needs no gesture for it). */
    audio: boolean;
  };
}

export const PS0: Scenario = {
  id: 'PS-0',
  name: 'Empty loop',
  warmupS: 3,
  defaultSeconds: 60,
  script: INPUT_SCRIPT.ERRATIC,
  start: { spawn: SPAWN_KIND.AIR, arena: ARENAS.RUGGED },
  gates: 'ps0',
};

/**
 * The v0.1c arena run (ADR-39 P1 and P2): 120 s of the circuit from the runway in the rugged arena,
 * after a 5 s warm-up, in the cockpit by default, at the High preset in a 1920 x 1080 drawing buffer.
 */
export const ARENA: Scenario = {
  id: 'ARENA',
  name: 'Circuit over the rugged arena',
  warmupS: 5,
  defaultSeconds: 120,
  script: INPUT_SCRIPT.CIRCUIT,
  start: { spawn: SPAWN_KIND.RUNWAY, arena: ARENAS.RUGGED },
  gates: 'arena',
  view: { camera: 'cockpit', quality: 'high', res: { width: 1920, height: 1080 }, audio: true },
};

export const SCENARIOS: readonly Scenario[] = [PS0, ARENA];

export function findScenario(id: string): Scenario | undefined {
  return SCENARIOS.find((s) => s.id.toLowerCase() === id.toLowerCase());
}
