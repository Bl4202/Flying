/**
 * The throttle lever (GDD 58.2, ADR-37): absolute or incremental, with a soft detent at military
 * power and an afterburner toggle.
 *
 * The lever is a number from 0 to 1 that the pump sends as the THROTTLE axis. An absolute
 * throttle (a HOTAS lever, a slider) sets it directly. Keys, sticks and the wheel move it:
 * `step` for a rate held over time, `nudge` for one notch.
 *
 * Between 0.8 and 1.0 the engine is in afterburner. Raising the lever with a held key stops at
 * 0.8 for a pause (`DETENT_PAUSE_MS`) so the afterburner is not lit by running past it; lowering
 * it, and setting it directly, are never held up.
 */
import { ThrottleLever } from './keyboard.ts';

/** Military power, the top of the dry range. */
export const MIL_DETENT = 0.8;
/** How long a held key waits at the detent before passing it. */
export const DETENT_PAUSE_MS = 300;

export class ThrottleController {
  readonly #lever: ThrottleLever;
  /** Time spent pushing up while sitting on the detent (a Float64Array: no garbage per step). */
  readonly #dwell = new Float64Array(1);

  constructor(initial = 0.5) {
    this.#lever = new ThrottleLever(initial);
  }

  get value(): number {
    return this.#lever.value;
  }

  /** Sets the lever directly (an absolute axis, or a respawn handing back the sim's throttle). */
  set(value: number): void {
    this.#lever.set(value);
    this.#dwell[0] = 0;
  }

  /**
   * Moves the lever at `ratePerSecond` (signed) for `dtMs`, pausing at the detent on the way up.
   * Read the result from `value`.
   */
  step(dtMs: number, ratePerSecond: number): void {
    let dt = Math.max(0, dtMs);
    const v = this.#lever.value;
    if (!(ratePerSecond > 0)) {
      this.#dwell[0] = 0;
      this.#lever.move(dt, ratePerSecond);
      return;
    }
    if (v === MIL_DETENT && (this.#dwell[0] ?? 0) < DETENT_PAUSE_MS) {
      const wait = DETENT_PAUSE_MS - (this.#dwell[0] ?? 0);
      if (dt <= wait) {
        this.#dwell[0] = (this.#dwell[0] ?? 0) + dt;
        return;
      }
      // The pause ends inside this step: the rest of the step moves the lever.
      this.#dwell[0] = DETENT_PAUSE_MS;
      dt -= wait;
    }
    if (v < MIL_DETENT && v + (ratePerSecond * dt) / 1000 >= MIL_DETENT) {
      // Arrived: stop exactly on the detent and start the pause with what is left of the step.
      const used = ((MIL_DETENT - v) / ratePerSecond) * 1000;
      this.#lever.set(MIL_DETENT);
      this.#dwell[0] = Math.max(0, dt - used);
      return;
    }
    this.#lever.move(dt, ratePerSecond);
  }

  /** Moves the lever by `delta` at once (one wheel notch). Raising it lands on the detent first. */
  nudge(delta: number): void {
    const v = this.#lever.value;
    let next = v + delta;
    if (delta > 0 && v < MIL_DETENT && next > MIL_DETENT) next = MIL_DETENT;
    this.#lever.set(next);
    this.#dwell[0] = DETENT_PAUSE_MS; // a deliberate notch is not held up the next time
  }

  /** Afterburner toggle: from the afterburner range down to military power, otherwise to full. */
  toggleAfterburner(): void {
    this.#lever.set(this.#lever.value > MIL_DETENT ? MIL_DETENT : 1);
    this.#dwell[0] = DETENT_PAUSE_MS;
  }

  /** Afterburner held: full while held, back to military power on release. */
  holdAfterburner(held: boolean): void {
    if (held) this.#lever.set(1);
    else if (this.#lever.value > MIL_DETENT) this.#lever.set(MIL_DETENT);
    this.#dwell[0] = DETENT_PAUSE_MS;
  }
}
