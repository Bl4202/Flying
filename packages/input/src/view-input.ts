/**
 * What the player is doing with the view, as the pump reports it (ADR-41). View actions never
 * enter the input frame: the sim does not know where the pilot looks. The page reads this object
 * every animation frame and writes the camera's side of it into the view channel.
 *
 * Presses are counters, not events, because the pump's `onAction` fires only on presses and a
 * counter survives a dropped frame: the page compares a counter with the value it saw last. Mouse
 * look is a running total in radians for the same reason. The numbers live in one typed array, so
 * updating them makes no garbage.
 */

/** Indices into `ViewInput.v`. */
export const VI = {
  /** Look rates, radians per second: right and up are positive. */
  LOOK_YAW_RATE: 0,
  LOOK_PITCH_RATE: 1,
  /** Running totals of mouse look, radians: right and up are positive. */
  MOUSE_YAW_TOTAL: 2,
  MOUSE_PITCH_TOTAL: 3,
  /** 1 while the key or button is held, else 0. */
  ZOOM: 4,
  FREE_LOOK: 5,
  /** Counts of presses since the start. */
  CAMERA_PRESSES: 6,
  RECENTRE_PRESSES: 7,
  DECLUTTER_PRESSES: 8,
  /** Net brighter-minus-dimmer presses since the start. */
  BRIGHTNESS_STEPS: 9,
} as const;

export const VIEW_INPUT_SIZE = 10;

/** The rate the look keys turn the view at, radians per second (90 degrees a second). */
export const LOOK_RATE_RAD_S = Math.PI / 2;
/** Mouse look, radians per pixel at sensitivity 1 (about 57 degrees for 400 pixels). */
export const MOUSE_LOOK_RAD_PER_PX = 0.0025;

export class ViewInput {
  readonly v = new Float64Array(VIEW_INPUT_SIZE);

  get lookYawRate(): number {
    return this.v[VI.LOOK_YAW_RATE] ?? 0;
  }
  get lookPitchRate(): number {
    return this.v[VI.LOOK_PITCH_RATE] ?? 0;
  }
  get mouseYawTotal(): number {
    return this.v[VI.MOUSE_YAW_TOTAL] ?? 0;
  }
  get mousePitchTotal(): number {
    return this.v[VI.MOUSE_PITCH_TOTAL] ?? 0;
  }
  get zoom(): boolean {
    return (this.v[VI.ZOOM] ?? 0) !== 0;
  }
  get freeLook(): boolean {
    return (this.v[VI.FREE_LOOK] ?? 0) !== 0;
  }
  get cameraPresses(): number {
    return this.v[VI.CAMERA_PRESSES] ?? 0;
  }
  get recentrePresses(): number {
    return this.v[VI.RECENTRE_PRESSES] ?? 0;
  }
  get declutterPresses(): number {
    return this.v[VI.DECLUTTER_PRESSES] ?? 0;
  }
  get brightnessSteps(): number {
    return this.v[VI.BRIGHTNESS_STEPS] ?? 0;
  }

  /** Releases everything held (a menu opened, focus was lost); the totals and counters stay. */
  release(): void {
    this.v[VI.LOOK_YAW_RATE] = 0;
    this.v[VI.LOOK_PITCH_RATE] = 0;
    this.v[VI.ZOOM] = 0;
    this.v[VI.FREE_LOOK] = 0;
  }
}
