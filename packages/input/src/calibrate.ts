/**
 * Axis calibration (GDD 58.2): the logic behind the editor's calibration dialog.
 *
 * Two steps. With every stick and lever at rest, `centre` records where each axis sits and how
 * much it wanders. Then the person moves everything through its full travel and `range` records
 * the smallest and largest readings. An axis that wandered during the first step is a ghost and
 * is left alone; one that did not move in the second is left uncalibrated.
 */
import { MAX_AXES, type PadSlot } from './devices/gamepad.ts';
import { calibrationKey, type ControlsProfile } from './profile.ts';
import { type Calibration } from './shaping.ts';

/** An axis that wanders more than this while at rest is a ghost. */
export const CALIBRATION_GHOST_NOISE = 0.2;
/** An axis must travel at least this far in the range step to be calibrated. */
export const CALIBRATION_MIN_SPAN = 0.4;
/** Samples needed in the centre step before it can finish. */
export const CENTRE_SAMPLES = 10;

export type CalibrationStep = 'centre' | 'range' | 'done';

export class CalibrationWizard {
  #step: CalibrationStep = 'centre';
  #samples = 0;
  readonly #sum = new Float64Array(MAX_AXES);
  readonly #restLo = new Float64Array(MAX_AXES);
  readonly #restHi = new Float64Array(MAX_AXES);
  readonly #lo = new Float64Array(MAX_AXES);
  readonly #hi = new Float64Array(MAX_AXES);
  #axes = 0;

  get step(): CalibrationStep {
    return this.#step;
  }

  /** How many samples the centre step has so far. */
  get centreSamples(): number {
    return this.#samples;
  }

  /** Feeds one reading of the device being calibrated. */
  update(slot: PadSlot): void {
    if (this.#step === 'done' || !slot.present) return;
    this.#axes = Math.max(this.#axes, slot.axisCount);
    for (let a = 0; a < slot.axisCount; a++) {
      if (a === slot.hatAxis) continue;
      const v = slot.axes[a] ?? 0;
      if (this.#step === 'centre') {
        if (this.#samples === 0) {
          this.#restLo[a] = v;
          this.#restHi[a] = v;
        } else {
          this.#restLo[a] = Math.min(this.#restLo[a] ?? v, v);
          this.#restHi[a] = Math.max(this.#restHi[a] ?? v, v);
        }
        this.#sum[a] = (this.#sum[a] ?? 0) + v;
        this.#lo[a] = v;
        this.#hi[a] = v;
      } else {
        this.#lo[a] = Math.min(this.#lo[a] ?? v, v);
        this.#hi[a] = Math.max(this.#hi[a] ?? v, v);
      }
    }
    if (this.#step === 'centre') this.#samples += 1;
  }

  /** Moves to the next step. The centre step needs `CENTRE_SAMPLES` readings first. */
  next(): CalibrationStep {
    if (this.#step === 'centre' && this.#samples >= CENTRE_SAMPLES) this.#step = 'range';
    else if (this.#step === 'range') this.#step = 'done';
    return this.#step;
  }

  /** True for an axis that wandered while at rest. */
  isGhost(axis: number): boolean {
    return (this.#restHi[axis] ?? 0) - (this.#restLo[axis] ?? 0) > CALIBRATION_GHOST_NOISE;
  }

  /** Axes seen so far that are ghosts. */
  ghosts(): number[] {
    const out: number[] = [];
    for (let a = 0; a < this.#axes; a++) if (this.#samples > 0 && this.isGhost(a)) out.push(a);
    return out;
  }

  /** The calibration of one axis, or `undefined` if it is a ghost or did not move enough. */
  result(axis: number): Calibration | undefined {
    if (this.#step !== 'done' || this.#samples === 0 || axis >= this.#axes) return undefined;
    if (this.isGhost(axis)) return undefined;
    const centre = (this.#sum[axis] ?? 0) / this.#samples;
    const min = Math.min(this.#lo[axis] ?? centre, centre);
    const max = Math.max(this.#hi[axis] ?? centre, centre);
    if (max - min < CALIBRATION_MIN_SPAN) return undefined;
    return { min, centre, max };
  }

  /** Every axis that calibrated, as `[axis, calibration]`. */
  results(): [number, Calibration][] {
    const out: [number, Calibration][] = [];
    for (let a = 0; a < this.#axes; a++) {
      const c = this.result(a);
      if (c !== undefined) out.push([a, c]);
    }
    return out;
  }
}

/** A copy of `profile` with the wizard's results stored for `device`. */
export function applyCalibration(
  profile: ControlsProfile,
  device: string,
  wizard: CalibrationWizard,
): ControlsProfile {
  const calibration = { ...profile.calibration };
  for (const [axis, c] of wizard.results()) calibration[calibrationKey(device, axis)] = c;
  return { ...profile, calibration };
}
