import type { AircraftStatus, CombatStatus } from '@flying/sim-bridge';
import type { AudioBackend } from './backend.ts';
import type { BusLevels, CameraMode } from './feed.ts';

/** A warning tone, or one of the duel's one-shot sounds, that has just started (audio clock). */
export interface ToneOnset {
  /** `TONE` value, or an `ONSET` code for one of the duel's one-shot sounds. */
  readonly tone: number;
  /** Seconds on the audio context's clock. */
  readonly time: number;
}

/**
 * What the game asks of the sound of flight, on top of the cues and volume every backend has: the
 * aircraft's state in, the player's mix and camera, and a report when a warning tone starts.
 */
export interface FlightAudio extends AudioBackend {
  /**
   * The aircraft's state and the duel's; call it every frame, the backend decides how often to pass
   * them on. Both are read when passed on, so the caller may refill them in place.
   */
  update(status: AircraftStatus | undefined, combat?: CombatStatus): void;
  /** Cockpit hears the outside through the canopy; chase hears it bare. */
  setCamera(camera: CameraMode): void;
  /** The player's volumes (`master` is the same as `setVolume`). */
  setBuses(levels: Partial<BusLevels>): void;
  /** Silences everything, for example while the tab is hidden; the volume is kept. */
  setMuted(muted: boolean): void;
  /** Calls `listener` when a warning tone starts. Returns a function that stops listening. */
  onOnset(listener: (onset: ToneOnset) => void): () => void;
}
