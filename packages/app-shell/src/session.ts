/**
 * The sortie the game starts: the URL flags (`?spawn=`, `?assist=`, `?g=`, `?arena=`, `?scenario=`,
 * `?skill=`) over the player's settings over the game's defaults. Free flight has no bandit (skill
 * 0); a duel's bandit is the URL's, else the player's choice, else the Recruit. A performance run
 * flies the scripted pilot, which needs the scripted session whatever the URL says, so every run of
 * a scenario is the same run.
 */
import type { Flags } from '@flying/platform-web';
import { ARENA, ASSIST_LEVEL, G_EFFECTS, SCENARIO, SPAWN_KIND } from '@flying/sim-bridge/layout';
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
const ARENAS = { rugged: ARENA.RUGGED, plain: ARENA.PLAIN } as const;
const SCENARIOS = {
  free: SCENARIO.FREE,
  duel: SCENARIO.DUEL,
  merge: SCENARIO.DUEL_MERGE,
} as const;

/** The settings a session is made from (the arena joined them with settings v2, the duel in v0.1d). */
type SessionSettings = Pick<Settings, 'spawn' | 'assist' | 'gEffects'> & {
  arena?: number;
  scenario?: number;
  banditSkill?: number;
};

/** The settings the URL can set for a visit, which the menus show in place of the stored ones. */
export type OverridableSettings = Pick<
  Settings,
  'spawn' | 'assist' | 'gEffects' | 'arena' | 'scenario' | 'banditSkill'
>;

export function sessionFromFlags(
  flags: Flags,
  /** A performance run: `true` for the bare scripted session, or the scenario whose start it flies. */
  perf: boolean | { start: { spawn: number; arena: number } },
  settings?: SessionSettings,
): SessionConfig {
  if (perf !== false) {
    return { ...SCRIPTED_SESSION, ...(perf === true ? {} : perf.start) };
  }
  const base = settings ?? DEFAULT_SESSION;
  const scenario =
    flags.scenario === undefined
      ? (settings?.scenario ?? DEFAULT_SESSION.scenario)
      : SCENARIOS[flags.scenario];
  return {
    spawn: flags.spawn === undefined ? base.spawn : SPAWNS[flags.spawn],
    assist: flags.assist === undefined ? base.assist : ASSISTS[flags.assist],
    gEffects: flags.g === undefined ? base.gEffects : G_DIALS[flags.g],
    arena: flags.arena === undefined ? (base.arena ?? DEFAULT_SESSION.arena) : ARENAS[flags.arena],
    scenario,
    skill: scenario === SCENARIO.FREE ? 0 : (flags.skill ?? settings?.banditSkill ?? 1),
  };
}

/**
 * The settings this visit's session differs from the stored ones in: what the URL flags (or a
 * performance run) set. The menu shows these in place of the stored values until the player
 * changes them.
 */
export function sessionOverrides(
  session: SessionConfig,
  settings: OverridableSettings,
): Partial<OverridableSettings> {
  const out: Partial<OverridableSettings> = {};
  if (session.spawn !== settings.spawn) out.spawn = session.spawn;
  if (session.assist !== settings.assist) out.assist = session.assist;
  if (session.gEffects !== settings.gEffects) out.gEffects = session.gEffects;
  if (session.arena !== settings.arena) out.arena = session.arena;
  if (session.scenario !== settings.scenario) out.scenario = session.scenario;
  // Free flight's skill 0 is not a choice: the stored bandit stays the one the menu shows.
  if (session.skill !== 0 && session.skill !== settings.banditSkill) {
    out.banditSkill = session.skill;
  }
  return out;
}
