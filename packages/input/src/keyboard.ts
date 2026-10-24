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
// The pump runs every few milliseconds and must not make garbage. V8 gives every store of a
// fractional number into an object field a fresh heap number, but a store into a Float64Array
// element allocates nothing, so the state of the classes the pump drives lives in one.

export class AxisEmulator {
  readonly #opts: AxisEmulation;
  readonly #v = new Float64Array(1);

  constructor(opts: AxisEmulation = DEFAULT_EMULATION) {
    this.#opts = opts;
  }

  get value(): number {
    return this.#v[0] ?? 0;
  }

  /**
   * Advances by `dtMs`. Holding the positive key ramps towards +1, the negative towards -1, both
   * (or neither) lets go. Reversing passes through zero at the faster return rate. Returns the
   * new value.
   */
  update(dtMs: number, negative: boolean, positive: boolean): number {
    this.step(dtMs, negative, positive);
    return this.#v[0] ?? 0;
  }

  /** `update` without a return value, for callers that read `value` and must make no garbage. */
  step(dtMs: number, negative: boolean, positive: boolean): void {
    const target = (positive ? 1 : 0) - (negative ? 1 : 0);
    const dt = Math.max(0, dtMs) / 1000;
    const up = this.#opts.rampUpPerSecond;
    const back = this.#opts.returnPerSecond;
    let v = this.#v[0] ?? 0;
    // The ramp is written out here instead of through a helper: a fractional number handed to or
    // returned from a function the compiler does not inline is a heap allocation.
    let goal = target;
    let rate = up;
    let travel = dt;
    if (target === 0) {
      rate = back;
    } else if (v * target < 0) {
      // Heading the other way: let go of the old direction quickly, then ramp the new one.
      const toZero = Math.abs(v) / back;
      if (dt <= toZero) {
        goal = 0;
        rate = back;
      } else {
        v = 0;
        travel = dt - toZero;
      }
    }
    const d = goal - v;
    const maxStep = rate * travel;
    v = Math.abs(d) <= maxStep ? goal : v + Math.sign(d) * maxStep;
    this.#v[0] = v + 0; // never a negative zero
  }

  reset(): void {
    this.#v[0] = 0;
  }
}

/**
 * A throttle lever that keys and sticks push up and down. Absolute throttle on a pad or stick
 * would set the value directly; this is the incremental mode (GDD 58.2).
 */
export class ThrottleLever {
  readonly #v = new Float64Array(1);

  constructor(initial = 0.5) {
    this.#v[0] = Math.min(Math.max(initial, 0), 1);
  }

  get value(): number {
    return this.#v[0] ?? 0;
  }

  /** Moves the lever by `rate` (fraction of travel per second, signed) for `dtMs`. */
  update(dtMs: number, ratePerSecond: number): number {
    this.move(dtMs, ratePerSecond);
    return this.#v[0] ?? 0;
  }

  /** `update` without a return value. */
  move(dtMs: number, ratePerSecond: number): void {
    this.#v[0] = Math.min(
      Math.max((this.#v[0] ?? 0) + (ratePerSecond * Math.max(0, dtMs)) / 1000, 0),
      1,
    );
  }

  set(value: number): void {
    this.#v[0] = Math.min(Math.max(value, 0), 1);
  }
}
