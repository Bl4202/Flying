/**
 * What the flight control system and the airframe are telling the pilot (GDD 11.11, 11.15, 53.4):
 * stall, the limiters working, overspeed, overstress, the gear, the ground coming up.
 *
 * `flightWarnings` decides from the ownship status which warnings are on; `WarningStrip` shows
 * them as one line of text in a single element: the assist level first, then each warning with a
 * shape glyph (a triangle for a warning, a diamond for a caution) so that colour is never the only
 * thing that tells them apart.
 */
import { type AircraftStatus, ASSIST_LEVEL, OWNSHIP_FLAG, OWNSHIP_FLAG2 } from '@flying/sim-bridge';
import { h } from './dom.ts';
import { t, type StringKey } from './strings.ts';

export type WarningId =
  | 'stall'
  | 'pullup'
  | 'departed'
  | 'overstress'
  | 'alpha'
  | 'g'
  | 'overspeed'
  | 'gear-speed'
  | 'flap-speed'
  | 'gear';

export type Severity = 'warning' | 'caution';

export interface Warning {
  id: WarningId;
  severity: Severity;
}

const GLYPH: Record<Severity, string> = { warning: '▲', caution: '◆' };

const TEXT: Record<WarningId, StringKey> = {
  stall: 'warn.stall',
  pullup: 'warn.pullup',
  departed: 'warn.departed',
  overstress: 'warn.overstress',
  alpha: 'warn.alpha',
  g: 'warn.g',
  overspeed: 'warn.overspeed',
  'gear-speed': 'warn.gear-speed',
  'flap-speed': 'warn.flap-speed',
  gear: 'warn.gear',
};

const ASSIST_SHORT: Record<number, StringKey> = {
  [ASSIST_LEVEL.FULL]: 'assist.short.full',
  [ASSIST_LEVEL.STANDARD]: 'assist.short.standard',
  [ASSIST_LEVEL.REDUCED]: 'assist.short.reduced',
  [ASSIST_LEVEL.MANUAL]: 'assist.short.manual',
};

// ALGO: the ground-proximity rule is a time-to-impact test, not a terrain look-ahead (that is the
// automatic collision avoidance, not in this version). It stays quiet on the ground, when climbing,
// and in the landing configuration (gear down and slow), where a low sink to the runway is normal.
/** Warn when the ground is less than this many seconds away at the present sink rate. */
const PULL_UP_SECONDS = 5;
/** ...and the sink is at least this fast, m/s. */
const PULL_UP_MIN_SINK_MPS = 1;
/** Only below this height above the ground, metres. */
const PULL_UP_MAX_AGL_M = 400;
/** Gear down and slower than this is a landing, not a dive, m/s calibrated. */
const LANDING_CAS_MPS = 100;
/** Bit 6 of the gear bits: the lever is down. */
const GEAR_LEVER_DOWN = 1 << 6;

/** The warnings that are on, most urgent first. A spectator or a wreck has none. */
export function flightWarnings(s: AircraftStatus | undefined): Warning[] {
  if (s === undefined || s.assist === 0) return [];
  if ((s.flags & (OWNSHIP_FLAG.SPECTATOR | OWNSHIP_FLAG.CRASHED)) !== 0) return [];
  const on = (bit: number): boolean => (s.flags2 & bit) !== 0;
  const onGround = on(OWNSHIP_FLAG2.WOW);
  const landing = (s.gearBits & GEAR_LEVER_DOWN) !== 0 && s.cas < LANDING_CAS_MPS;
  const sinking = -s.vs;
  const groundComing =
    !onGround &&
    !landing &&
    s.agl < PULL_UP_MAX_AGL_M &&
    sinking >= PULL_UP_MIN_SINK_MPS &&
    s.agl / sinking < PULL_UP_SECONDS;

  const out: Warning[] = [];
  const add = (id: WarningId, severity: Severity, when: boolean): void => {
    if (when) out.push({ id, severity });
  };
  add('stall', 'warning', (s.flags & OWNSHIP_FLAG.STALL_WARN) !== 0);
  add('pullup', 'warning', groundComing);
  add('departed', 'warning', on(OWNSHIP_FLAG2.DEPARTED));
  add('overstress', 'warning', on(OWNSHIP_FLAG2.OVERSTRESS));
  add('alpha', 'caution', on(OWNSHIP_FLAG2.ALPHA_LIMIT));
  add('g', 'caution', on(OWNSHIP_FLAG2.G_LIMIT));
  add('overspeed', 'caution', on(OWNSHIP_FLAG2.OVERSPEED));
  add('gear-speed', 'caution', on(OWNSHIP_FLAG2.GEAR_OVERSPEED));
  add('flap-speed', 'caution', on(OWNSHIP_FLAG2.FLAP_OVERSPEED));
  add('gear', 'caution', on(OWNSHIP_FLAG2.GEAR_WARNING));
  return out;
}

/** The strip's text: the assist name, then each warning behind its glyph. Empty for no aircraft. */
export function stripText(s: AircraftStatus | undefined): string {
  if (s === undefined || s.assist === 0) return '';
  const parts: string[] = [];
  const assist = ASSIST_SHORT[s.assist];
  if (assist !== undefined) parts.push(t(assist));
  for (const w of flightWarnings(s)) parts.push(`${GLYPH[w.severity]} ${t(TEXT[w.id])}`);
  return parts.join('  ');
}

/** The most serious severity among the warnings, or `none`. */
export function stripSeverity(s: AircraftStatus | undefined): Severity | 'none' {
  const all = flightWarnings(s);
  if (all.some((w) => w.severity === 'warning')) return 'warning';
  return all.length > 0 ? 'caution' : 'none';
}

/** One element that carries the line. It rewrites its text only when the text changes. */
export class WarningStrip {
  readonly #el: HTMLElement;
  #text = '';
  #severity: Severity | 'none' = 'none';

  constructor(root: HTMLElement) {
    this.#el = h(root.ownerDocument, 'div', {
      class: 'warning-strip',
      testid: 'warning-strip',
      data: { severity: 'none' },
      attrs: { role: 'status', 'aria-live': 'polite' },
    });
    this.#el.hidden = true;
    root.append(this.#el);
  }

  get element(): HTMLElement {
    return this.#el;
  }

  update(status: AircraftStatus | undefined): void {
    const text = stripText(status);
    if (text !== this.#text) {
      this.#text = text;
      this.#el.textContent = text;
      this.#el.hidden = text === '';
    }
    const severity = stripSeverity(status);
    if (severity !== this.#severity) {
      this.#severity = severity;
      this.#el.dataset['severity'] = severity;
    }
  }

  destroy(): void {
    this.#el.remove();
  }
}
