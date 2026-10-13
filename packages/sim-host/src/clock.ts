/**
 * The fixed-step clock (ADR-26, GDD 0.6.1). Pure: it is told the time and answers how many ticks
 * to run, so it can be tested with a fake clock.
 *
 * Ticks owed are `floor((now - anchor) / dt) - done`, computed from the anchor each time rather
 * than by adding `dt` to a running total, so rounding never accumulates and over any stretch with
 * no stall the number of ticks run is exactly `floor(elapsed / dt)`: the rate is exact on
 * average whatever the wake-up jitter.
 *
 * If more than `maxTicksPerIteration` are owed (the worker was stalled), it runs that many and
 * gives up the rest: the clock re-anchors at the present and counts the lost time in
 * `droppedMs`. The sim runs slow rather than spiralling into ever-longer catch-up.
 *
 * Pausing stops the clock. Resuming re-anchors, so there is no burst of catch-up ticks.
 */

/**
 * Subtracting two large timestamps loses low bits, so a tick that falls due exactly at
 * `nextDue()` can compute as 0.9999999999 of a tick and wait for the next wake-up. A billionth of
 * a tick (about 17 picoseconds) closes that gap and is far below anything a clock can measure.
 */
const TICK_EPSILON = 1e-9;

export interface FixedStepOptions {
  /** Tick length, milliseconds. */
  dtMs: number;
  /** Most ticks one `update` may return. */
  maxTicksPerIteration: number;
}

export class FixedStepClock {
  readonly dtMs: number;
  readonly maxTicksPerIteration: number;
  #anchor = 0;
  #done = 0;
  #running = false;
  #droppedMs = 0;
  #dropEvents = 0;
  #total = 0;

  constructor(opts: FixedStepOptions) {
    if (
      !(opts.dtMs > 0) ||
      !Number.isInteger(opts.maxTicksPerIteration) ||
      opts.maxTicksPerIteration < 1
    ) {
      throw new RangeError(
        'FixedStepClock needs dtMs > 0 and a positive whole maxTicksPerIteration',
      );
    }
    this.dtMs = opts.dtMs;
    this.maxTicksPerIteration = opts.maxTicksPerIteration;
  }

  get running(): boolean {
    return this.#running;
  }

  /** Time given up because the worker fell too far behind, milliseconds. */
  get droppedMs(): number {
    return this.#droppedMs;
  }

  /** How many times time was given up. */
  get dropEvents(): number {
    return this.#dropEvents;
  }

  /** Ticks handed out since construction. */
  get totalTicks(): number {
    return this.#total;
  }

  /** Starts (or restarts) counting from `now`. The first tick falls due one `dt` later. */
  start(now: number): void {
    this.#anchor = now;
    this.#done = 0;
    this.#running = true;
  }

  /** Stops handing out ticks. */
  pause(): void {
    this.#running = false;
  }

  /** Same as `start`: the time spent paused is not owed. */
  resume(now: number): void {
    this.start(now);
  }

  /** Ticks to run now. */
  update(now: number): number {
    if (!this.#running) {
      return 0;
    }
    const owed = Math.floor((now - this.#anchor) / this.dtMs + TICK_EPSILON) - this.#done;
    if (owed <= 0) {
      return 0;
    }
    if (owed > this.maxTicksPerIteration) {
      const run = this.maxTicksPerIteration;
      const owedMs = now - this.#anchor - this.#done * this.dtMs;
      this.#droppedMs += owedMs - run * this.dtMs;
      this.#dropEvents += 1;
      this.#anchor = now;
      this.#done = 0;
      this.#total += run;
      return run;
    }
    this.#done += owed;
    this.#total += owed;
    return owed;
  }

  /** When the next tick falls due, on the same clock `update` is given. */
  nextDue(): number {
    return this.#anchor + (this.#done + 1) * this.dtMs;
  }
}
