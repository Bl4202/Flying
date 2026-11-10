/**
 * The words the HUD writes, which are not data: the warning texts, the assist names and the few
 * labels. The app supplies them from its strings (so they can be translated); these English ones
 * are what the HUD uses until it does, and what the tests use. Every character of every text must
 * be in the font (a test walks them all).
 */
import { WARNINGS } from '@flying/sim-bridge';

export interface HudTexts {
  /** The text of each warning, by its id (`WarningId`). */
  warnings: Readonly<Record<string, string>>;
  /** The assist level's short name, by level 1 to 4 (Full, Standard, Reduced, Manual). */
  assist: readonly [string, string, string, string];
  gear: string;
  flapsTakeOff: string;
  flapsLanding: string;
  brakes: string;
  parking: string;
  max: string;
  radar: string;
  throttle: string;
  afterburner: string;
  fuel: string;
  fuelFlow: string;
  endurance: string;
  // The duel's (ADR-41 amendment).
  gun: string;
  /** Short-range missile. */
  srm: string;
  arm: string;
  safe: string;
  tone: string;
  lock: string;
  shoot: string;
  inRange: string;
  hit: string;
  splash: string;
  /** Why a launch was refused, by `LAUNCH_INHIBIT` 1 to 7. */
  inhibit: readonly [string, string, string, string, string, string, string];
  /** The target box's range and closure labels. */
  rangeLabel: string;
  closureLabel: string;
}

export const DEFAULT_HUD_TEXTS: HudTexts = {
  warnings: {
    pullup: 'PULL UP',
    stall: 'STALL',
    departed: 'DEPARTED',
    overstress: 'OVERSTRESS',
    alpha: 'ALPHA',
    g: 'OVER G',
    overspeed: 'OVERSPEED',
    'gear-speed': 'GEAR SPEED',
    'flap-speed': 'FLAP SPEED',
    gear: 'GEAR',
  },
  assist: ['FULL', 'STD', 'RED', 'MAN'],
  gear: 'GEAR',
  flapsTakeOff: 'FLAP TO',
  flapsLanding: 'FLAP LDG',
  brakes: 'BRAKE',
  parking: 'PARK',
  max: 'MAX',
  radar: 'R',
  throttle: 'THR',
  afterburner: 'AB',
  fuel: 'FUEL',
  fuelFlow: 'FF',
  endurance: 'END',
  gun: 'GUN',
  srm: 'SRM',
  arm: 'ARM',
  safe: 'SAFE',
  tone: 'TONE',
  lock: 'LOCK',
  shoot: 'SHOOT',
  inRange: 'IN RNG',
  hit: 'HIT',
  splash: 'SPLASH',
  inhibit: ['SAFE', 'EMPTY', 'NO LOCK', 'TOO CLOSE', 'G LIMIT', 'AOA LIMIT', 'SLOW'],
  rangeLabel: 'R',
  closureLabel: 'VC',
};

/** The warning ids, most urgent first. */
export const HUD_WARNING_IDS: readonly string[] = WARNINGS.map((w) => w.id);
