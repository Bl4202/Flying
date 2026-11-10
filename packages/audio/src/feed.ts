/**
 * From the game's state to the synthesis's parameters (ADR-42): a pure function the page runs a few
 * dozen times a second. It reads the ownship status the HUD and the warning strip already read, and
 * writes `FlightDsp.params` or, in the browser, the worklet's `AudioParam` values. It makes no garbage.
 */
import type { AircraftStatus } from '@flying/sim-bridge';
import { COMBAT_COUNTER, type CombatStatus } from '@flying/sim-bridge/combat-status';
import { COMBAT_FLAG, OWNSHIP_FLAG, OWNSHIP_FLAG2 } from '@flying/sim-bridge/layout';
import { warningBits } from '@flying/sim-bridge/warnings';
import { P } from './dsp/flight-dsp.ts';

export type CameraMode = 'cockpit' | 'chase';

/** The player's volumes, linear 0 to 1 (the settings' Audio tab). */
export interface BusLevels {
  master: number;
  aircraft: number;
  warnings: number;
  ui: number;
}

export function defaultBuses(): BusLevels {
  return { master: 1, aircraft: 1, warnings: 1, ui: 1 };
}

/**
 * Writes the aircraft's sound parameters into `out` (indices `P`, `PARAM_COUNT` long). A missing
 * status, a spectator and a wreck are silent: the engine stopped and nothing in the air. The master
 * volume (`P.MASTER`) is not written; whatever applies it does.
 *
 * The duel's come from `combat`: the gun while it fires, the bandit's gun at the bandit's range while
 * it fires, the seeker, the session's counters (a rise is a sound) and the newest explosion's range.
 * The counters are passed on even for a wreck, so the explosion that made it is heard.
 */
export function fillParams(
  out: Float64Array,
  s: AircraftStatus | undefined,
  combat: CombatStatus | undefined,
  camera: CameraMode,
  aircraftBus: number,
  warningsBus: number,
): void {
  out[P.COCKPIT] = camera === 'cockpit' ? 1 : 0;
  out[P.AIRCRAFT_BUS] = aircraftBus;
  out[P.WARNINGS_BUS] = warningsBus;
  fillCombat(out, combat, s);
  if (s === undefined || (s.flags & (OWNSHIP_FLAG.SPECTATOR | OWNSHIP_FLAG.CRASHED)) !== 0) {
    out[P.CORE] = 0;
    out[P.AB_LEVEL] = 0;
    out[P.ENGINE_BITS] = 0;
    out[P.QBAR] = 0;
    out[P.ALPHA] = 0;
    out[P.ALPHA_WARN] = 0;
    out[P.AIRBRAKE] = 0;
    out[P.GROUND_SPEED] = 0;
    out[P.ON_GROUND] = 0;
    out[P.WARNINGS] = 0;
    return;
  }
  out[P.CORE] = s.engineCore;
  out[P.AB_LEVEL] = s.abLevel;
  out[P.ENGINE_BITS] = s.engineBits;
  out[P.QBAR] = s.qbar;
  out[P.ALPHA] = s.alpha;
  out[P.ALPHA_WARN] = s.alphaWarn;
  out[P.AIRBRAKE] = s.airbrake;
  out[P.GROUND_SPEED] = s.groundSpeed;
  out[P.ON_GROUND] = (s.flags2 & OWNSHIP_FLAG2.WOW) !== 0 ? 1 : 0;
  out[P.WARNINGS] = warningBits(s);
}

/** The duel's parameters; all silent without a duel. */
function fillCombat(
  out: Float64Array,
  c: CombatStatus | undefined,
  s: AircraftStatus | undefined,
): void {
  const flags = c === undefined ? 0 : c.flags;
  const flying =
    s !== undefined && (s.flags & (OWNSHIP_FLAG.SPECTATOR | OWNSHIP_FLAG.CRASHED)) === 0;
  const alive = c !== undefined && flying && (flags & COMBAT_FLAG.ACTIVE) !== 0;
  out[P.GUN_FIRING] = alive && (flags & COMBAT_FLAG.GUN_FIRING) !== 0 ? 1 : 0;
  out[P.ENEMY_GUN_DIST_M] =
    alive && (flags & COMBAT_FLAG.BANDIT_GUN_FIRING) !== 0 ? Math.max(c.banditRange, 1) : 0;
  out[P.SEEKER] = alive ? c.seeker : 0;
  out[P.SIGNAL] = alive ? c.signal : 0;
  out[P.LAUNCHES] = c === undefined ? 0 : (c.counters[COMBAT_COUNTER.MISSILES_FIRED] ?? 0);
  out[P.HITS_TAKEN] = c === undefined ? 0 : (c.counters[COMBAT_COUNTER.HITS_TAKEN] ?? 0);
  out[P.EXPLOSIONS] = c === undefined ? 0 : (c.counters[COMBAT_COUNTER.EXPLOSIONS] ?? 0);
  out[P.EXPLOSION_DIST_M] = c === undefined ? 0 : c.explosionRange;
}
