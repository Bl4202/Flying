/**
 * Turns what the pump reports about the view (`ViewInput`) and the player's settings into the
 * `ViewState` the camera rig and the HUD read (ADR-41): the camera cycles on each press, the HUD
 * declutter steps through its three levels, brightness moves in tens. The page calls `update()`
 * every animation frame, and writes `state` to the view channel (or sends it as a message when
 * `update()` says it changed).
 */
import {
  CHASE_DISTANCE_DEFAULT_M,
  CHASE_DISTANCE_RANGE_M,
  copyViewState,
  FOV_DEG_DEFAULT,
  FOV_DEG_RANGE,
  newViewState,
  type ViewState,
} from '@flying/sim-bridge/view-channel';
import { type ViewInput } from './view-input.ts';

/** One brightness press moves the HUD by this much, per cent, within the limits. */
export const HUD_BRIGHTNESS_STEP = 10;
export const HUD_BRIGHTNESS_MIN = 20;
export const HUD_BRIGHTNESS_MAX = 100;
/** The HUD declutter levels: 0 full, 1 reduced, 2 minimum. */
export const HUD_DECLUTTER_LEVELS = 3;

/** The settings the player chooses, which the view carries along. */
export interface ViewSettings {
  /** Vertical field of view, degrees, 50 to 120. */
  fovDeg?: number;
  /** Camera shake, 0 to 100 per cent. */
  shakePercent?: number;
  reducedMotion?: boolean;
  /** Chase distance, metres, 40 to 150. */
  chaseDistanceM?: number;
  /** HUD brightness, per cent. */
  hudBrightness?: number;
}

export class ViewController {
  /** The view as it stands: what the camera and the HUD read. */
  readonly state: ViewState = newViewState();
  readonly #input: ViewInput;
  #cameraSeen = 0;
  #recentreSeen = 0;
  #declutterSeen = 0;
  #brightnessSeen = 0;
  /** The field-by-field copy of the state last reported as changed. */
  readonly #sent: ViewState = newViewState();
  #primed = false;

  constructor(input: ViewInput) {
    this.#input = input;
    // Presses before this controller existed are history.
    this.#cameraSeen = input.cameraPresses;
    this.#recentreSeen = input.recentrePresses;
    this.#declutterSeen = input.declutterPresses;
    this.#brightnessSeen = input.brightnessSteps;
  }

  /** Applies the player's settings (any that are given), kept within their ranges. */
  configure(settings: ViewSettings): void {
    const s = this.state;
    const clamp = (v: number, lo: number, hi: number): number => Math.min(Math.max(v, lo), hi);
    if (settings.fovDeg !== undefined && Number.isFinite(settings.fovDeg)) {
      s.fovDeg = clamp(settings.fovDeg, FOV_DEG_RANGE[0], FOV_DEG_RANGE[1]);
    }
    if (settings.shakePercent !== undefined && Number.isFinite(settings.shakePercent)) {
      s.shake = clamp(settings.shakePercent / 100, 0, 1);
    }
    if (settings.reducedMotion !== undefined) s.reducedMotion = settings.reducedMotion;
    if (settings.chaseDistanceM !== undefined && Number.isFinite(settings.chaseDistanceM)) {
      s.chaseDistanceM = clamp(
        settings.chaseDistanceM,
        CHASE_DISTANCE_RANGE_M[0],
        CHASE_DISTANCE_RANGE_M[1],
      );
    }
    if (settings.hudBrightness !== undefined && Number.isFinite(settings.hudBrightness)) {
      s.hudBrightness = clamp(
        Math.round(settings.hudBrightness),
        HUD_BRIGHTNESS_MIN,
        HUD_BRIGHTNESS_MAX,
      );
    }
  }

  /** Sets the camera directly (a menu choice, or `?camera=`); the next press still cycles it. */
  setCamera(camera: ViewState['camera']): void {
    this.state.camera = camera;
  }

  /**
   * Reads the pump's view input into the state. Returns true if the state differs from what the
   * last call to `update()` that returned true left, so a page without a shared channel knows
   * when to send it.
   */
  update(): boolean {
    const i = this.#input;
    const s = this.state;
    s.lookYawRate = i.lookYawRate;
    s.lookPitchRate = i.lookPitchRate;
    s.mouseYawTotal = i.mouseYawTotal;
    s.mousePitchTotal = i.mousePitchTotal;
    s.zoom = i.zoom;
    s.freeLook = i.freeLook;

    const cameraPresses = i.cameraPresses - this.#cameraSeen;
    this.#cameraSeen = i.cameraPresses;
    if (cameraPresses % 2 !== 0) s.camera = s.camera === 'cockpit' ? 'chase' : 'cockpit';

    const recentres = i.recentrePresses - this.#recentreSeen;
    this.#recentreSeen = i.recentrePresses;
    s.recentreCount += recentres;

    const declutters = i.declutterPresses - this.#declutterSeen;
    this.#declutterSeen = i.declutterPresses;
    if (declutters !== 0) {
      s.hudDeclutter =
        (((s.hudDeclutter + declutters) % HUD_DECLUTTER_LEVELS) + HUD_DECLUTTER_LEVELS) %
        HUD_DECLUTTER_LEVELS;
    }

    const steps = i.brightnessSteps - this.#brightnessSeen;
    this.#brightnessSeen = i.brightnessSteps;
    if (steps !== 0) {
      s.hudBrightness = Math.min(
        Math.max(s.hudBrightness + steps * HUD_BRIGHTNESS_STEP, HUD_BRIGHTNESS_MIN),
        HUD_BRIGHTNESS_MAX,
      );
    }

    return this.#changed();
  }

  #changed(): boolean {
    const a = this.state;
    const b = this.#sent;
    const same =
      this.#primed &&
      a.camera === b.camera &&
      a.lookYawRate === b.lookYawRate &&
      a.lookPitchRate === b.lookPitchRate &&
      a.mouseYawTotal === b.mouseYawTotal &&
      a.mousePitchTotal === b.mousePitchTotal &&
      a.recentreCount === b.recentreCount &&
      a.zoom === b.zoom &&
      a.freeLook === b.freeLook &&
      a.reducedMotion === b.reducedMotion &&
      a.fovDeg === b.fovDeg &&
      a.shake === b.shake &&
      a.chaseDistanceM === b.chaseDistanceM &&
      a.hudDeclutter === b.hudDeclutter &&
      a.hudBrightness === b.hudBrightness;
    if (same) return false;
    copyViewState(b, a);
    this.#primed = true;
    return true;
  }
}

/** The settings' defaults, as the view starts. */
export const VIEW_DEFAULTS: Required<ViewSettings> = {
  fovDeg: FOV_DEG_DEFAULT,
  shakePercent: 100,
  reducedMotion: false,
  chaseDistanceM: CHASE_DISTANCE_DEFAULT_M,
  hudBrightness: HUD_BRIGHTNESS_MAX,
};
