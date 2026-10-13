/**
 * Small metric types for hot paths: recording never allocates.
 */

export class Counter {
  #value = 0;
  inc(by = 1): void {
    this.#value += by;
  }
  get value(): number {
    return this.#value;
  }
  reset(): void {
    this.#value = 0;
  }
}

export class Gauge {
  #value = 0;
  set(v: number): void {
    this.#value = v;
  }
  get value(): number {
    return this.#value;
  }
}

/**
 * Fixed-bucket histogram over `[0, max]`: `buckets` equal-width bins plus one overflow bin.
 * Quantiles are exact to the bin width (`max / buckets`) and always err high, which is the safe
 * direction for latency gates. Negative and non-finite samples count as zero and as overflow.
 */
export class Histogram {
  readonly max: number;
  readonly buckets: number;
  #width: number;
  #counts: Uint32Array;
  #count = 0;
  #sum = 0;
  #min = Infinity;
  #peak = -Infinity;

  constructor(max: number, buckets = 256) {
    if (!(max > 0) || !Number.isInteger(buckets) || buckets < 1) {
      throw new RangeError('Histogram needs max > 0 and a whole number of buckets');
    }
    this.max = max;
    this.buckets = buckets;
    this.#width = max / buckets;
    this.#counts = new Uint32Array(buckets + 1);
  }

  record(v: number): void {
    let i: number;
    if (Number.isNaN(v)) {
      i = this.buckets;
    } else if (v <= 0) {
      i = 0;
    } else {
      i = Math.min(Math.floor(v / this.#width), this.buckets);
    }
    this.#counts[i] = (this.#counts[i] ?? 0) + 1;
    this.#count += 1;
    if (Number.isFinite(v)) {
      this.#sum += v;
      if (v < this.#min) this.#min = v;
      if (v > this.#peak) this.#peak = v;
    }
  }

  get count(): number {
    return this.#count;
  }

  get mean(): number {
    return this.#count === 0 ? 0 : this.#sum / this.#count;
  }

  get min(): number {
    return this.#count === 0 ? 0 : this.#min;
  }

  /** Largest finite sample seen. */
  get peak(): number {
    return this.#count === 0 ? 0 : this.#peak;
  }

  /** Samples above `max` (or NaN). */
  get overflow(): number {
    return this.#counts[this.buckets] ?? 0;
  }

  /**
   * The value below which a fraction `q` of samples fall, as the upper edge of the bin that holds
   * it (clamped to the largest sample seen, so it never exceeds reality). 0 when empty.
   */
  quantile(q: number): number {
    if (this.#count === 0) return 0;
    const target = Math.max(1, Math.ceil(Math.min(Math.max(q, 0), 1) * this.#count));
    let seen = 0;
    for (let i = 0; i <= this.buckets; i++) {
      seen += this.#counts[i] ?? 0;
      if (seen >= target) {
        return i === this.buckets ? this.#peak : Math.min((i + 1) * this.#width, this.#peak);
      }
    }
    return this.#peak;
  }

  reset(): void {
    this.#counts.fill(0);
    this.#count = 0;
    this.#sum = 0;
    this.#min = Infinity;
    this.#peak = -Infinity;
  }
}
