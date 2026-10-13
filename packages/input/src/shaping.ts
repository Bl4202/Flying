/**
 * Per-axis shaping, exactly as GDD 58.2 specifies it. Order matters:
 *
 * ```text
 * x0 = calibrate(raw)                                     -> [-1, 1]
 * x1 = deadzone:   sign(x0) * max(0, |x0| - dc) / (1 - dc - do), clamped
 * x2 = curve:      sign(x1) * ((1 - k)|x1| + k|x1|^3)     (expo)   or   sign(x1) * |x1|^gamma
 * x3 = saturation: clamp(x2 * g, -1, 1)
 * x4 = smoothing:  x4 += (x3 - x4) * (1 - exp(-dt / tau))
 * x5 = rate limit: |x5 - previous x5| <= s_max * dt
 * x6 = invert / offset (trim)
 * ```
 *
 * The stages are plain functions so each can be tested against hand-computed values; `AxisShaper`
 * chains them and holds the two pieces of state (smoothing and rate limit).
 */

/** Centre deadzone default. */
export const DEAD_CENTRE = 0.03;
/** Outer deadzone default. */
export const DEAD_OUTER = 0.01;

export type Curve = { kind: 'expo'; k: number } | { kind: 'power'; gamma: number };

export interface AxisShape {
  /** Fraction of travel around the centre that reads as zero, 0 to 0.5. */
  deadCentre: number;
  /** Fraction of travel at the end that reads as full, 0 to 0.5. */
  deadOuter: number;
  curve: Curve;
  /** Multiplier after the curve, 0.5 to 2. */
  gain: number;
  /** Smoothing time constant, milliseconds (0 = none), up to 120. */
  tauMs: number;
  /** Largest change per second (`Infinity` = no limit). */
  maxRatePerSecond: number;
  invert: boolean;
  /** Added after inversion (trim). */
  offset: number;
}

/** Defaults for a stick axis: 30 % expo, no smoothing. */
export const STICK_SHAPE: AxisShape = {
  deadCentre: DEAD_CENTRE,
  deadOuter: DEAD_OUTER,
  curve: { kind: 'expo', k: 0.3 },
  gain: 1,
  tauMs: 0,
  maxRatePerSecond: Infinity,
  invert: false,
  offset: 0,
};

/** Defaults for a throttle or trigger axis: linear. */
export const LINEAR_SHAPE: AxisShape = { ...STICK_SHAPE, curve: { kind: 'expo', k: 0 } };

/** No shaping at all, for values that were already shaped (keyboard axis emulation). */
export const IDENTITY_SHAPE: AxisShape = {
  deadCentre: 0,
  deadOuter: 0,
  curve: { kind: 'expo', k: 0 },
  gain: 1,
  tauMs: 0,
  maxRatePerSecond: Infinity,
  invert: false,
  offset: 0,
};

const sign = (x: number): number => (x < 0 ? -1 : x > 0 ? 1 : 0);
const clamp = (x: number, lo: number, hi: number): number => Math.min(Math.max(x, lo), hi);

export interface Calibration {
  min: number;
  centre: number;
  max: number;
}

/** Maps a raw reading to [-1, 1] using the recorded minimum, centre and maximum. */
export function calibrate(raw: number, c: Calibration = { min: -1, centre: 0, max: 1 }): number {
  if (!Number.isFinite(raw)) return 0;
  const span = raw >= c.centre ? c.max - c.centre : c.centre - c.min;
  if (!(span > 0)) return 0;
  return clamp((raw - c.centre) / span, -1, 1);
}

/** Stage 1. */
export function deadzone(x: number, centre: number, outer: number): number {
  const width = 1 - centre - outer;
  if (!(width > 0)) return 0;
  // `+ 0` turns a negative zero into a positive one: the core hashes f32 bits, so -0 is a state.
  return clamp((sign(x) * Math.max(0, Math.abs(x) - centre)) / width, -1, 1) + 0;
}

/** Stage 2, exponential: `(1 - k)|x| + k|x|^3`. */
export function expoCurve(x: number, k: number): number {
  const a = Math.abs(x);
  return sign(x) * ((1 - k) * a + k * a * a * a);
}

/** Stage 2, power: `|x|^gamma`. */
export function powerCurve(x: number, gamma: number): number {
  return sign(x) * Math.abs(x) ** gamma;
}

/** Stage 3. */
export function saturate(x: number, gain: number): number {
  return clamp(x * gain, -1, 1);
}

/** Stage 4: one step of first-order smoothing. `tau <= 0` passes the target straight through. */
export function smooth(previous: number, target: number, dtMs: number, tauMs: number): number {
  if (!(tauMs > 0)) return target;
  return previous + (target - previous) * (1 - Math.exp(-dtMs / tauMs));
}

/** Stage 5: moves from `previous` towards `target` by at most `maxRate * dtSeconds`. */
export function rateLimit(
  previous: number,
  target: number,
  dtSeconds: number,
  maxRate: number,
): number {
  const step = maxRate * dtSeconds;
  return previous + clamp(target - previous, -step, step);
}

/** Stages 1 to 3, the memoryless part of the pipeline. */
export function shapeStatic(x0: number, shape: AxisShape): number {
  const x1 = deadzone(x0, shape.deadCentre, shape.deadOuter);
  const x2 =
    shape.curve.kind === 'expo' ? expoCurve(x1, shape.curve.k) : powerCurve(x1, shape.curve.gamma);
  return saturate(x2, shape.gain);
}

/** The whole pipeline for one axis, with the state that stages 4 and 5 need. */
export class AxisShaper {
  readonly shape: AxisShape;
  #smoothed = 0;
  #limited = 0;
  #primed = false;

  constructor(shape: AxisShape = STICK_SHAPE) {
    this.shape = shape;
  }

  /** Shapes one calibrated reading `x0` taken `dtMs` after the previous one. */
  step(x0: number, dtMs: number): number {
    const x3 = shapeStatic(x0, this.shape);
    if (!this.#primed) {
      // The first sample has no history: start from it instead of easing in from zero.
      this.#primed = true;
      this.#smoothed = x3;
      this.#limited = x3;
    } else {
      this.#smoothed = smooth(this.#smoothed, x3, dtMs, this.shape.tauMs);
      this.#limited = Number.isFinite(this.shape.maxRatePerSecond)
        ? rateLimit(this.#limited, this.#smoothed, dtMs / 1000, this.shape.maxRatePerSecond)
        : this.#smoothed;
    }
    const x6 = (this.shape.invert ? -this.#limited : this.#limited) + this.shape.offset;
    return clamp(x6, -1, 1) + 0;
  }

  /** Forgets history (device unplugged, focus lost). */
  reset(): void {
    this.#smoothed = 0;
    this.#limited = 0;
    this.#primed = false;
  }
}
