/**
 * What the flight control system and the airframe are telling the pilot (GDD 11.11, 11.15, 53.4,
 * 54.8): the ground coming up, stall, the limiters working, overspeed, overstress, the gear.
 *
 * The rules are shared with the HUD and the warning tones (`warningBits` in `@flying/sim-bridge`).
 * This module keeps the texts and `WarningStrip`, which shows them as one line of text in a single
 * element: the assist level first, then each warning with a shape glyph (a triangle for a warning,
 * a diamond for a caution) so that colour is never the only thing that tells them apart.
 */
import {
  type AircraftStatus,
  ASSIST_LEVEL,
  type Severity,
  type WarningId,
  WARNINGS,
  warningBits,
  warningSeverity,
} from '@flying/sim-bridge';
import { h } from './dom.ts';
import { t, type StringKey } from './strings.ts';

export type { Severity, WarningId } from '@flying/sim-bridge';

export interface Warning {
  id: WarningId;
  severity: Severity;
}

const GLYPH: Record<Severity, string> = { warning: '▲', caution: '◆' };

/** The text key of each warning (the HUD and the strip show the same words). */
export const WARNING_TEXT: Record<WarningId, StringKey> = {
  pullup: 'warn.pullup',
  stall: 'warn.stall',
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

/** The warnings in `bits`, most urgent first (GDD 54.8). */
export function warningsOf(bits: number): Warning[] {
  return WARNINGS.filter((w) => (bits & w.bit) !== 0).map(({ id, severity }) => ({
    id,
    severity,
  }));
}

/** The warnings that are on, most urgent first. A spectator or a wreck has none. */
export function flightWarnings(s: AircraftStatus | undefined): Warning[] {
  return warningsOf(warningBits(s));
}

/** The strip's text for an assist level and warning bits. Empty for no aircraft (assist 0). */
export function stripTextOf(assist: number, bits: number): string {
  if (assist === 0) return '';
  const parts: string[] = [];
  const name = ASSIST_SHORT[assist];
  if (name !== undefined) parts.push(t(name));
  for (const w of WARNINGS) {
    if ((bits & w.bit) !== 0) parts.push(`${GLYPH[w.severity]} ${t(WARNING_TEXT[w.id])}`);
  }
  return parts.join('  ');
}

/** The strip's text: the assist name, then each warning behind its glyph. Empty for no aircraft. */
export function stripText(s: AircraftStatus | undefined): string {
  return stripTextOf(s?.assist ?? 0, warningBits(s));
}

/** The most serious severity among the warnings, or `none`. */
export function stripSeverity(s: AircraftStatus | undefined): Severity | 'none' {
  return warningSeverity(warningBits(s));
}

/**
 * One element that carries the line. It rebuilds its text only when the assist level or the
 * warnings change, so a frame with nothing new does no string work.
 */
export class WarningStrip {
  readonly #el: HTMLElement;
  #text = '';
  #severity: Severity | 'none' = 'none';
  #assist = -1;
  #bits = -1;

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
    const assist = status?.assist ?? 0;
    const bits = warningBits(status);
    if (assist === this.#assist && bits === this.#bits) return;
    this.#assist = assist;
    this.#bits = bits;
    const text = stripTextOf(assist, bits);
    if (text !== this.#text) {
      this.#text = text;
      this.#el.textContent = text;
      this.#el.hidden = text === '';
    }
    const severity = warningSeverity(bits);
    if (severity !== this.#severity) {
      this.#severity = severity;
      this.#el.dataset['severity'] = severity;
    }
  }

  destroy(): void {
    this.#el.remove();
  }
}
