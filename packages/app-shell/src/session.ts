/**
 * The sortie the game starts: the URL flags (`?spawn=`, `?assist=`, `?g=`) over the player's
 * settings over the game's defaults. A performance run flies the scripted pilot, which needs the
 * scripted session whatever the URL says, so every run of a scenario is the same run.
 */
import type { Flags } from '@flying/platform-web';
import { ASSIST_LEVEL, G_EFFECTS, SPAWN_KIND } from '@flying/sim-bridge/layout';
import type { Settings } from '@flying/ui';
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

export function sessionFromFlags(
  flags: Flags,
  perf: boolean,
  settings?: Pick<Settings, 'spawn' | 'assist' | 'gEffects'>,
): SessionConfig {
  if (perf) {
    return { ...SCRIPTED_SESSION };
  }
  const base = settings ?? DEFAULT_SESSION;
  return {
    spawn: flags.spawn === undefined ? base.spawn : SPAWNS[flags.spawn],
    assist: flags.assist === undefined ? base.assist : ASSISTS[flags.assist],
    gEffects: flags.g === undefined ? base.gEffects : G_DIALS[flags.g],
  };
}

/**
 * The settings this visit's session differs from the stored ones in: what the URL flags (or a
 * performance run) set. The menu shows these in place of the stored values until the player
 * changes them.
 */
export function sessionOverrides(
  session: SessionConfig,
  settings: Pick<Settings, 'spawn' | 'assist' | 'gEffects'>,
): Partial<Pick<Settings, 'spawn' | 'assist' | 'gEffects'>> {
  const out: Partial<Pick<Settings, 'spawn' | 'assist' | 'gEffects'>> = {};
  if (session.spawn !== settings.spawn) out.spawn = session.spawn;
  if (session.assist !== settings.assist) out.assist = session.assist;
  if (session.gEffects !== settings.gEffects) out.gEffects = session.gEffects;
  return out;
}
