/**
 * The mouse as a virtual joystick (GDD 58.4): motion moves a stick that springs back to centre.
 * Right is roll right; moving the mouse back (towards you) pulls the nose up.
 */

/** Pixels of motion for full deflection at sensitivity 1. */
export const STICK_RANGE_PX = 400;

export class MouseStick {
  /** Roll and pitch (a Float64Array: moving the stick makes no garbage). */
  readonly #p = new Float64Array(2);

  /** Roll, -1 to 1. */
  get x(): number {
    return this.#p[0] ?? 0;
  }

  /** Pitch, -1 to 1; +1 is back (nose up). */
  get y(): number {
    return this.#p[1] ?? 0;
  }

  /** Moves the stick by a mouse delta in pixels. */
  move(dxPx: number, dyPx: number, sensitivity: number): void {
    const k = sensitivity / STICK_RANGE_PX;
    this.#p[0] = clamp1(this.x + dxPx * k);
    this.#p[1] = clamp1(this.y + dyPx * k);
  }

  /**
   * Lets the stick return towards centre. `spring` is 0 (it stays where it was put) to 1 (back to
   * centre within about half a second).
   */
  update(dtMs: number, spring: number): void {
    if (!(spring > 0)) return;
    // ALGO: exponential return, time constant 0.25 s / spring.
    const f = Math.exp((-4 * spring * Math.max(0, dtMs)) / 1000);
    this.#p[0] = snap(this.x * f);
    this.#p[1] = snap(this.y * f);
  }

  reset(): void {
    this.#p.fill(0);
  }
}

const clamp1 = (v: number): number => Math.min(Math.max(v, -1), 1);
/** Below this a returning stick is at rest (and is never a negative zero). */
const snap = (v: number): number => (Math.abs(v) < 1e-4 ? 0 : v);
