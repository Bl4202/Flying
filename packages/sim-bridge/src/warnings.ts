/**
 * Which flight warnings are on (GDD 11.11, 11.15, 54.8): one rule set that the warning strip, the
 * HUD and the warning tones all read, so they always agree.
 *
 * `warningBits` turns an ownship status into a bitmask without allocating, so it can run every
 * frame. The bits are numbered in GDD 54.8's priority order (ground collision, then stall and
 * departure, then structural, then the cautions): the lowest set bit is the most urgent warning.
 * Display texts live with the UI (`@flying/ui`), keyed by `WarningId`.
 */
import type { AircraftStatus } from './aircraft-status.ts';
import { OWNSHIP_FLAG, OWNSHIP_FLAG2 } from './layout.gen.ts';

/** One bit per warning, lowest = most urgent. */
export const WARNING = {
  PULLUP: 1 << 0,
  STALL: 1 << 1,
  DEPARTED: 1 << 2,
  OVERSTRESS: 1 << 3,
  ALPHA: 1 << 4,
  G: 1 << 5,
  OVERSPEED: 1 << 6,
  GEAR_SPEED: 1 << 7,
  FLAP_SPEED: 1 << 8,
  GEAR: 1 << 9,
} as const;

export type WarningId =
  | 'pullup'
  | 'stall'
  | 'departed'
  | 'overstress'
  | 'alpha'
  | 'g'
  | 'overspeed'
  | 'gear-speed'
  | 'flap-speed'
  | 'gear';

/** A warning needs immediate action (red); a caution needs attention (amber). */
export type Severity = 'warning' | 'caution';

export interface WarningDef {
  readonly bit: number;
  readonly id: WarningId;
  readonly severity: Severity;
}

/** Every warning, most urgent first (GDD 54.8). */
export const WARNINGS: readonly WarningDef[] = [
  { bit: WARNING.PULLUP, id: 'pullup', severity: 'warning' },
  { bit: WARNING.STALL, id: 'stall', severity: 'warning' },
  { bit: WARNING.DEPARTED, id: 'departed', severity: 'warning' },
  { bit: WARNING.OVERSTRESS, id: 'overstress', severity: 'warning' },
  { bit: WARNING.ALPHA, id: 'alpha', severity: 'caution' },
  { bit: WARNING.G, id: 'g', severity: 'caution' },
  { bit: WARNING.OVERSPEED, id: 'overspeed', severity: 'caution' },
  { bit: WARNING.GEAR_SPEED, id: 'gear-speed', severity: 'caution' },
  { bit: WARNING.FLAP_SPEED, id: 'flap-speed', severity: 'caution' },
  { bit: WARNING.GEAR, id: 'gear', severity: 'caution' },
];

/** The bits of the warnings (red); the rest are cautions. */
export const WARNING_MASK = WARNING.PULLUP | WARNING.STALL | WARNING.DEPARTED | WARNING.OVERSTRESS;
/** The bits of the cautions (amber). */
export const CAUTION_MASK =
  WARNING.ALPHA |
  WARNING.G |
  WARNING.OVERSPEED |
  WARNING.GEAR_SPEED |
  WARNING.FLAP_SPEED |
  WARNING.GEAR;

// ALGO: the ground-proximity rule is a time-to-impact test, not a terrain look-ahead (that is the
// automatic collision avoidance, not in this version). It stays quiet on the ground, when climbing,
// and in the landing configuration (gear down and slow), where a low sink to the runway is normal.
/** Warn when the ground is less than this many seconds away at the present sink rate. */
export const PULL_UP_SECONDS = 5;
/** ...and the sink is at least this fast, m/s. */
export const PULL_UP_MIN_SINK_MPS = 1;
/** Only below this height above the ground, metres. */
export const PULL_UP_MAX_AGL_M = 400;
/** Gear down and slower than this is a landing, not a dive, m/s calibrated. */
export const LANDING_CAS_MPS = 100;
/** Bit 6 of the gear bits: the lever is down. */
const GEAR_LEVER_DOWN = 1 << 6;

/**
 * The warnings that are on, as `WARNING` bits. A spectator, a wreck or no status has none.
 * Allocates nothing.
 */
export function warningBits(s: AircraftStatus | undefined): number {
  if (s === undefined || s.assist === 0) return 0;
  if ((s.flags & (OWNSHIP_FLAG.SPECTATOR | OWNSHIP_FLAG.CRASHED)) !== 0) return 0;
  const f2 = s.flags2;
  const onGround = (f2 & OWNSHIP_FLAG2.WOW) !== 0;
  const landing = (s.gearBits & GEAR_LEVER_DOWN) !== 0 && s.cas < LANDING_CAS_MPS;
  const sinking = -s.vs;
  let bits = 0;
  if (
    !onGround &&
    !landing &&
    s.agl < PULL_UP_MAX_AGL_M &&
    sinking >= PULL_UP_MIN_SINK_MPS &&
    s.agl / sinking < PULL_UP_SECONDS
  ) {
    bits |= WARNING.PULLUP;
  }
  if ((s.flags & OWNSHIP_FLAG.STALL_WARN) !== 0) bits |= WARNING.STALL;
  if ((f2 & OWNSHIP_FLAG2.DEPARTED) !== 0) bits |= WARNING.DEPARTED;
  if ((f2 & OWNSHIP_FLAG2.OVERSTRESS) !== 0) bits |= WARNING.OVERSTRESS;
  if ((f2 & OWNSHIP_FLAG2.ALPHA_LIMIT) !== 0) bits |= WARNING.ALPHA;
  if ((f2 & OWNSHIP_FLAG2.G_LIMIT) !== 0) bits |= WARNING.G;
  if ((f2 & OWNSHIP_FLAG2.OVERSPEED) !== 0) bits |= WARNING.OVERSPEED;
  if ((f2 & OWNSHIP_FLAG2.GEAR_OVERSPEED) !== 0) bits |= WARNING.GEAR_SPEED;
  if ((f2 & OWNSHIP_FLAG2.FLAP_OVERSPEED) !== 0) bits |= WARNING.FLAP_SPEED;
  if ((f2 & OWNSHIP_FLAG2.GEAR_WARNING) !== 0) bits |= WARNING.GEAR;
  return bits;
}

/** The most serious severity among `bits`, or `none`. */
export function warningSeverity(bits: number): Severity | 'none' {
  if ((bits & WARNING_MASK) !== 0) return 'warning';
  return (bits & CAUTION_MASK) !== 0 ? 'caution' : 'none';
}
