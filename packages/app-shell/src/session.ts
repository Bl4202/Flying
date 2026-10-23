/**
 * The sortie the game starts, from the URL flags (`?spawn=`, `?assist=`, `?g=`) over the game's
 * defaults. A performance run flies the scripted pilot, which needs the scripted session whatever
 * the URL says, so every run of a scenario is the same run.
 */
import type { Flags } from '@flying/platform-web';
import { ASSIST_LEVEL, G_EFFECTS, SPAWN_KIND } from '@flying/sim-bridge/layout';
import {
  DEFAULT_SESSION,
  SCRIPTED_SESSION,
  type SessionConfig,
} from '@flying/sim-bridge/sim-handle';

const SPAWNS = { runway: SPAWN_KIND.RUNWAY, air: SPAWN_KIND.AIR } as const;
const ASSISTS = {
  full: ASSIST_LEVEL.FULL,
  standard: ASSIST_LEVEL.STANDARD,
  reduced: ASSIST_LEVEL.REDUCED,
  manual: ASSIST_LEVEL.MANUAL,
} as const;
const G_DIALS = {
  off: G_EFFECTS.OFF,
  reduced: G_EFFECTS.REDUCED,
  full: G_EFFECTS.FULL,
} as const;

export function sessionFromFlags(flags: Flags, perf: boolean): SessionConfig {
  if (perf) {
    return { ...SCRIPTED_SESSION };
  }
  return {
    spawn: flags.spawn === undefined ? DEFAULT_SESSION.spawn : SPAWNS[flags.spawn],
    assist: flags.assist === undefined ? DEFAULT_SESSION.assist : ASSISTS[flags.assist],
    gEffects: flags.g === undefined ? DEFAULT_SESSION.gEffects : G_DIALS[flags.g],
  };
}
