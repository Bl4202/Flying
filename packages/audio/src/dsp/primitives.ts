/**
 * The building blocks of the flight audio's synthesis (ADR-42): decibels, a deterministic noise
 * source, biquad filters and a one-pole smoother. All state lives in typed arrays and every
 * per-sample function takes only the arrays and an index, so a render quantum makes no garbage
 * (`process()` allocation is tested, A7). Nothing here touches Web Audio: these are pure functions
 * over numbers, and are tested offline in Node.
 */

/** Decibels to a linear amplitude. */
export function dbToLin(db: number): number {
  return 10 ** (db / 20);
}

/** A linear amplitude to decibels (floored at -200 so silence is a number). */
export function linToDb(lin: number): number {
  return lin > 1e-10 ? 20 * Math.log10(lin) : -200;
}

/**
 * xorshift32 white noise in [-1, 1): a different sequence for every seed, the same one every run.
 * `state` is a one-element `Uint32Array` so it can be advanced without allocating.
 */
export function noise(state: Uint32Array): number {
  let x = state[0] as number;
  x ^= x << 13;
  x ^= x >>> 17;
  x ^= x << 5;
  state[0] = x;
  return (x >>> 0) / 0x8000_0000 - 1;
}

/**
 * Draws `n` times five numbers from `state`, in the order `noise` would give them, into five planes
 * of one array: draw `k` of sample `i` goes to `planes[k * stride + i]`. A block of noise made in one
 * call, so no draw is returned as a number from a call inside a loop (see `Biquad.processBlock`).
 */
export function fillNoise5(
  state: Uint32Array,
  planes: Float64Array,
  stride: number,
  n: number,
): void {
  let x = state[0] as number;
  for (let i = 0; i < n; i++) {
    for (let k = 0; k < 5; k++) {
      x ^= x << 13;
      x ^= x >>> 17;
      x ^= x << 5;
      planes[k * stride + i] = (x >>> 0) / 0x8000_0000 - 1;
    }
  }
  state[0] = x;
}

/** A noise state with a non-zero seed. */
export function noiseState(seed: number): Uint32Array {
  const s = new Uint32Array(1);
  s[0] = seed >>> 0 || 0x9e37_79b9;
  return s;
}

/** Filter kinds this synthesis uses. */
export type FilterKind = 'lowpass' | 'highpass' | 'bandpass' | 'peaking';

/**
 * A biquad: five coefficients (b0, b1, b2, a1, a2, already divided by a0) and two state values
 * (transposed direct form II) per filter, in typed arrays so many filters share no objects.
 */
export class Biquad {
  /** b0, b1, b2, a1, a2. */
  readonly c = new Float64Array(5);
  /** The two delay registers. */
  readonly z = new Float64Array(2);

  constructor(kind: FilterKind, sampleRate: number, freqHz: number, q: number, gainDb = 0) {
    this.set(kind, sampleRate, freqHz, q, gainDb);
  }

  /** Sets the coefficients (the audio-EQ cookbook's), keeping the state. */
  set(kind: FilterKind, sampleRate: number, freqHz: number, q: number, gainDb = 0): void {
    // Keep the frequency below Nyquist and the Q above zero, whatever a profile says.
    const f = Math.min(Math.max(freqHz, 1), sampleRate * 0.49);
    const qq = Math.max(q, 0.05);
    const w0 = (2 * Math.PI * f) / sampleRate;
    const cos = Math.cos(w0);
    const alpha = Math.sin(w0) / (2 * qq);
    const a = 10 ** (gainDb / 40);
    let b0: number;
    let b1: number;
    let b2: number;
    let a0: number;
    let a1: number;
    let a2: number;
    switch (kind) {
      case 'lowpass':
        b0 = (1 - cos) / 2;
        b1 = 1 - cos;
        b2 = (1 - cos) / 2;
        a0 = 1 + alpha;
        a1 = -2 * cos;
        a2 = 1 - alpha;
        break;
      case 'highpass':
        b0 = (1 + cos) / 2;
        b1 = -(1 + cos);
        b2 = (1 + cos) / 2;
        a0 = 1 + alpha;
        a1 = -2 * cos;
        a2 = 1 - alpha;
        break;
      case 'bandpass':
        // Constant 0 dB peak gain.
        b0 = alpha;
        b1 = 0;
        b2 = -alpha;
        a0 = 1 + alpha;
        a1 = -2 * cos;
        a2 = 1 - alpha;
        break;
      case 'peaking':
        b0 = 1 + alpha * a;
        b1 = -2 * cos;
        b2 = 1 - alpha * a;
        a0 = 1 + alpha / a;
        a1 = -2 * cos;
        a2 = 1 - alpha / a;
        break;
    }
    const c = this.c;
    c[0] = b0 / a0;
    c[1] = b1 / a0;
    c[2] = b2 / a0;
    c[3] = a1 / a0;
    c[4] = a2 / a0;
  }

  /** Filters one sample. */
  process(x: number): number {
    const c = this.c;
    const z = this.z;
    const y = (c[0] as number) * x + (z[0] as number);
    z[0] = (c[1] as number) * x - (c[3] as number) * y + (z[1] as number);
    z[1] = (c[2] as number) * x - (c[4] as number) * y;
    return y;
  }

  /**
   * Filters the first `n` samples of `input` into `output` (they may be the same array). The same
   * arithmetic as `process`, sample for sample, with the delay registers held in locals: one call
   * a block instead of one a sample, so no number crosses a call inside the loop (V8 boxes such a
   * number on the heap when it does not inline the call).
   */
  processBlock(input: Float64Array, output: Float64Array, n: number): void {
    const c = this.c;
    const z = this.z;
    const b0 = c[0] as number;
    const b1 = c[1] as number;
    const b2 = c[2] as number;
    const a1 = c[3] as number;
    const a2 = c[4] as number;
    let z0 = z[0] as number;
    let z1 = z[1] as number;
    for (let i = 0; i < n; i++) {
      const x = input[i] as number;
      const y = b0 * x + z0;
      z0 = b1 * x - a1 * y + z1;
      z1 = b2 * x - a2 * y;
      output[i] = y;
    }
    z[0] = z0;
    z[1] = z1;
  }

  /** Clears the state. */
  reset(): void {
    this.z.fill(0);
  }
}

/** The one-pole coefficient that moves a value `1 - 1/e` of the way in `seconds`, at `sampleRate`. */
export function onePoleCoefficient(seconds: number, sampleRate: number): number {
  return seconds <= 0 ? 1 : 1 - Math.exp(-1 / (seconds * sampleRate));
}

/** The one-pole coefficient for a block of `frames` samples. */
export function blockCoefficient(seconds: number, sampleRate: number, frames: number): number {
  return seconds <= 0 ? 1 : 1 - Math.exp(-frames / (seconds * sampleRate));
}

/** The mean power of `n` samples of `x`, as decibels relative to full scale (a sine of 1.0 is -3 dB). */
export function rmsDb(x: ArrayLike<number>, from = 0, to = x.length): number {
  let sum = 0;
  for (let i = from; i < to; i++) sum += (x[i] ?? 0) * (x[i] ?? 0);
  return linToDb(Math.sqrt(sum / Math.max(to - from, 1)));
}
