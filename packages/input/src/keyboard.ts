/**
 * Keyboard state and axis emulation (GDD 58.2, 58.3).
 *
 * Keys are identified by `KeyboardEvent.code`, the physical key, so layouts do not matter. OS
 * auto-repeat is ignored: holding a key is "down" from the first press to the release, and the
 * emulated axis has its own timing.
 */

export class KeyboardState {
  readonly #down = new Set<string>();

  /** Records a press. Returns true if it changed anything (auto-repeats and re-presses do not). */
  keyDown(code: string, repeat: boolean): boolean {
    if (repeat || this.#down.has(code)) return false;
    this.#down.add(code);
    return true;
  }

  /** Records a release. Returns true if the key was down. */
  keyUp(code: string): boolean {
    return this.#down.delete(code);
  }

  /** Forgets every key (focus lost: a release event may never arrive). */
  releaseAll(): void {
    this.#down.clear();
  }

  isDown(code: string): boolean {
    return this.#down.has(code);
  }

  /** True if any of `codes` is down. */
  anyDown(codes: readonly string[]): boolean {
    return codes.some((c) => this.#down.has(c));
  }

  get count(): number {
    return this.#down.size;
  }
}

export interface AxisEmulation {
  /** How fast the axis moves towards a held key's end, per second. */
  rampUpPerSecond: number;
  /** How fast it returns to zero when released, or crosses zero when reversing, per second. */
  returnPerSecond: number;
}

/** GDD 58.2: `r_up` 3.0 per second, `r_dn` 4.0 per second. */
export const DEFAULT_EMULATION: AxisEmulation = { rampUpPerSecond: 3, returnPerSecond: 4 };

/** A virtual stick axis driven by two keys. */
export class AxisEmulator {
  readonly #opts: AxisEmulation;
  #value = 0;

  constructor(opts: AxisEmulation = DEFAULT_EMULATION) {
    this.#opts = opts;
  }

  get value(): number {
    return this.#value;
  }

  /**
   * Advances by `dtMs`. Holding the positive key ramps towards +1, the negative towards -1, both
   * (or neither) lets go. Reversing passes through zero at the faster return rate.
   */
  update(dtMs: number, negative: boolean, positive: boolean): number {
    const target = (positive ? 1 : 0) - (negative ? 1 : 0);
    const dt = Math.max(0, dtMs) / 1000;
    let v = this.#value;
    if (target === 0) {
      v = moveTowards(v, 0, this.#opts.returnPerSecond * dt);
    } else if (v * target < 0) {
      // Heading the other way: let go of the old direction quickly, then ramp the new one.
      const toZero = Math.abs(v) / this.#opts.returnPerSecond;
      if (dt <= toZero) {
        v = moveTowards(v, 0, this.#opts.returnPerSecond * dt);
      } else {
        v = moveTowards(0, target, this.#opts.rampUpPerSecond * (dt - toZero));
      }
    } else {
      v = moveTowards(v, target, this.#opts.rampUpPerSecond * dt);
    }
    this.#value = v + 0; // never a negative zero
    return this.#value;
  }

  reset(): void {
    this.#value = 0;
  }
}

function moveTowards(from: number, to: number, maxStep: number): number {
  const d = to - from;
  return Math.abs(d) <= maxStep ? to : from + Math.sign(d) * maxStep;
}

/**
 * A throttle lever that keys and sticks push up and down. Absolute throttle on a pad or stick
 * would set the value directly; this is the incremental mode (GDD 58.2).
 */
export class ThrottleLever {
  #value: number;

  constructor(initial = 0.5) {
    this.#value = Math.min(Math.max(initial, 0), 1);
  }

  get value(): number {
    return this.#value;
  }

  /** Moves the lever by `rate` (fraction of travel per second, signed) for `dtMs`. */
  update(dtMs: number, ratePerSecond: number): number {
    this.#value = Math.min(
      Math.max(this.#value + (ratePerSecond * Math.max(0, dtMs)) / 1000, 0),
      1,
    );
    return this.#value;
  }

  set(value: number): void {
    this.#value = Math.min(Math.max(value, 0), 1);
  }
}
