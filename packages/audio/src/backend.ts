/**
 * The audio seam (GDD 61.5): what the rest of the game asks of sound, whatever makes the sound.
 *
 * v0.0b has only the null backend. A later milestone adds Web Audio beeps and samples, then worklet
 * synthesis with spatial mixing; each must pass `audioBackendContract` and callers do not change.
 *
 * Browsers keep an `AudioContext` suspended until the page has had a user gesture, so the app
 * calls `resume()` on the first key press or click and never earlier.
 */

/** Short sounds the game can ask for. The set grows with the game; backends ignore unknown ones. */
export type AudioCue = 'ui-confirm' | 'ui-error' | 'warning';

export type AudioState = 'suspended' | 'running' | 'closed';

export interface AudioBackend {
  readonly kind: string;
  readonly state: AudioState;
  /** Starts (or restarts) output. Safe to call repeatedly; resolves when the state has settled. */
  resume(): Promise<void>;
  /** Stops output but keeps the backend usable. */
  suspend(): Promise<void>;
  /** Plays a cue now. Does nothing unless the backend is running. */
  cue(name: AudioCue): void;
  /** Master volume, 0 (silent) to 1. Out-of-range and non-finite values are clamped. */
  setVolume(volume: number): void;
  readonly volume: number;
  /** Releases everything. After this the state is `closed` and every call does nothing. */
  close(): Promise<void>;
}

/** Clamps to 0..1; non-finite values give 0 so a bad setting is silent rather than loud. */
export function clampVolume(v: number): number {
  return Number.isFinite(v) ? Math.min(1, Math.max(0, v)) : 0;
}
