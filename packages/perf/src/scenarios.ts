/**
 * The performance scenarios (GDD 60.9). PS-0 is the empty loop, which measures the skeleton so later
 * milestones can see what each system adds. `ARENA` (v0.1c) is the first with something in it: the
 * circuit flown from the runway over the rugged arena with the sky, the terrain to the horizon, the
 * HUD and the sound on, which is what the v0.1c exit's 60 FPS is measured on. `COMBAT` (v0.1d) is the
 * bot's dogfight from the merge against a Regular bandit, with the guns, the missiles and their
 * sounds. PS-1 onwards need content that does not exist yet.
 */
import { ARENA as ARENAS, INPUT_SCRIPT, SCENARIO, SPAWN_KIND } from '@flying/sim-bridge/layout';

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
  /** The sortie the scripted pilot flies: where it starts, in which arena, and against whom. */
  start: { spawn: number; arena: number; scenario: number; skill: number };
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
  start: { spawn: SPAWN_KIND.AIR, arena: ARENAS.RUGGED, scenario: SCENARIO.FREE, skill: 0 },
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
  start: { spawn: SPAWN_KIND.RUNWAY, arena: ARENAS.RUGGED, scenario: SCENARIO.FREE, skill: 0 },
  gates: 'arena',
  view: { camera: 'cockpit', quality: 'high', res: { width: 1920, height: 1080 }, audio: true },
};

/**
 * The v0.1d dogfight run (ADR-45): the bot playtester from the merge (the bandit 3 km ahead) against
 * a Regular, which shoots back: both guns, tracers, missiles, explosions and their sounds. Judged
 * by the arena's gates.
 */
export const COMBAT: Scenario = {
  id: 'COMBAT',
  name: 'Dogfight against a Regular bandit',
  warmupS: 5,
  defaultSeconds: 120,
  script: INPUT_SCRIPT.SORTIE,
  start: {
    spawn: SPAWN_KIND.AIR,
    arena: ARENAS.RUGGED,
    scenario: SCENARIO.DUEL_MERGE,
    skill: 2,
  },
  gates: 'arena',
  view: { camera: 'cockpit', quality: 'high', res: { width: 1920, height: 1080 }, audio: true },
};

export const SCENARIOS: readonly Scenario[] = [PS0, ARENA, COMBAT];

export function findScenario(id: string): Scenario | undefined {
  return SCENARIOS.find((s) => s.id.toLowerCase() === id.toLowerCase());
}
