/**
 * Mouse-aim (GDD 58.4, ADR-37): the mouse moves an aim direction in the world; the simulator
 * flies the aircraft towards it.
 *
 * The aim is a world-frame direction, azimuth clockwise from north in [-pi, pi] and elevation up
 * in [-pi/2, pi/2], so it stays put in the sky while the aircraft turns. It is limited to a cone
 * around where the nose points, so the cursor can never get far ahead of an aircraft that cannot
 * follow. It travels in two axes of the input frame: `CURSOR_X = az / pi`, `CURSOR_Y = el / (pi/2)`,
 * with the MOUSE_AIM flag set. The simulator turns the aim error into stick (`fcs::mouse_aim`).
 *
 * All the angle work is on plain numbers; nothing here allocates.
 */

/** The aim stays within this angle of the nose. */
export const AIM_CONE_RAD = (35 * Math.PI) / 180;
/** Radians of aim per pixel of mouse motion at sensitivity 1. */
export const AIM_RAD_PER_PX = 0.0015;

const HALF_PI = Math.PI / 2;

/** Wraps an angle to [-pi, pi]. */
export function wrapPi(a: number): number {
  let x = a % (2 * Math.PI);
  if (x > Math.PI) x -= 2 * Math.PI;
  else if (x < -Math.PI) x += 2 * Math.PI;
  return x;
}

export class MouseAim {
  /** Azimuth and elevation (a Float64Array: moving the aim makes no garbage). */
  readonly #p = new Float64Array(2);
  #active = false;

  /** Azimuth, radians clockwise from north. */
  get az(): number {
    return this.#p[0] ?? 0;
  }

  /** Elevation, radians up. */
  get el(): number {
    return this.#p[1] ?? 0;
  }

  get active(): boolean {
    return this.#active;
  }

  /** Starts aiming where the nose points. */
  enable(noseAz: number, noseEl: number): void {
    this.#active = true;
    this.#p[0] = wrapPi(noseAz);
    this.#p[1] = Math.min(Math.max(noseEl, -HALF_PI), HALF_PI);
  }

  disable(): void {
    this.#active = false;
  }

  /** Moves the aim by a mouse delta in pixels. Right turns it clockwise; up raises it. */
  move(dxPx: number, dyPx: number, sensitivity: number): void {
    if (!this.#active) return;
    const k = AIM_RAD_PER_PX * sensitivity;
    this.#p[0] = wrapPi(this.az + dxPx * k);
    this.#p[1] = Math.min(Math.max(this.el - dyPx * k, -HALF_PI), HALF_PI);
  }

  /**
   * Keeps the aim inside the cone around the nose, and lets it drift back towards the nose at
   * `springPerSecond` (0 = it stays where it was put).
   */
  follow(noseAz: number, noseEl: number, dtMs: number, springPerSecond: number): void {
    if (!this.#active) return;
    if (springPerSecond > 0) {
      const d = angleBetween(this.az, this.el, noseAz, noseEl);
      const f = 1 - Math.exp((-springPerSecond * Math.max(0, dtMs)) / 1000);
      this.#arc(this.az, this.el, noseAz, noseEl, d, f);
    }
    const d = angleBetween(noseAz, noseEl, this.az, this.el);
    if (d > AIM_CONE_RAD) this.#arc(noseAz, noseEl, this.az, this.el, d, AIM_CONE_RAD / d);
  }

  /** The `CURSOR_X` axis value. */
  get cursorX(): number {
    return this.az / Math.PI;
  }

  /** The `CURSOR_Y` axis value. */
  get cursorY(): number {
    return this.el / HALF_PI;
  }

  /**
   * Sets the aim to the point the fraction `f` of the way along the great-circle arc of angle `d`
   * from direction 0 to direction 1.
   */
  #arc(az0: number, el0: number, az1: number, el1: number, d: number, f: number): void {
    const s = Math.sin(d);
    if (s < 1e-9) return;
    const a = Math.sin((1 - f) * d) / s;
    const b = Math.sin(f * d) / s;
    const c0 = Math.cos(el0);
    const c1 = Math.cos(el1);
    // East, north, up.
    const x = a * c0 * Math.sin(az0) + b * c1 * Math.sin(az1);
    const y = a * c0 * Math.cos(az0) + b * c1 * Math.cos(az1);
    const z = a * Math.sin(el0) + b * Math.sin(el1);
    this.#p[0] = Math.atan2(x, y);
    this.#p[1] = Math.asin(Math.min(Math.max(z / Math.sqrt(x * x + y * y + z * z), -1), 1));
  }
}

/** Angle between two directions given as azimuth and elevation, radians. */
export function angleBetween(az0: number, el0: number, az1: number, el1: number): number {
  const c = Math.sin(el0) * Math.sin(el1) + Math.cos(el0) * Math.cos(el1) * Math.cos(az0 - az1);
  return Math.acos(Math.min(Math.max(c, -1), 1));
}
