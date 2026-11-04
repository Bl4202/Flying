import type { AircraftStatus } from '@flying/sim-bridge';
import { clampVolume, type AudioCue, type AudioState } from './backend.ts';
import { defaultBuses, type BusLevels, type CameraMode } from './feed.ts';
import type { FlightAudio } from './flight-audio.ts';

/**
 * Makes no sound. It keeps the same state machine as a real backend (suspended until resumed,
 * closed for good) and records what it was asked to play, so tests and the debug overlay can see
 * that the game called it.
 */
export class NullAudioBackend implements FlightAudio {
  readonly kind = 'null';
  state: AudioState = 'suspended';
  volume = 1;
  /** Cues that would have sounded, oldest first. */
  readonly played: AudioCue[] = [];
  resumeCalls = 0;
  /** What the flight audio was last told, for tests and the debug overlay. */
  camera: CameraMode = 'cockpit';
  muted = false;
  buses: BusLevels = defaultBuses();
  lastStatus: AircraftStatus | undefined;
  updates = 0;

  resume(): Promise<void> {
    this.resumeCalls += 1;
    if (this.state !== 'closed') {
      this.state = 'running';
    }
    return Promise.resolve();
  }

  suspend(): Promise<void> {
    if (this.state !== 'closed') {
      this.state = 'suspended';
    }
    return Promise.resolve();
  }

  cue(name: AudioCue): void {
    if (this.state === 'running') {
      this.played.push(name);
    }
  }

  setVolume(volume: number): void {
    if (this.state !== 'closed') {
      this.volume = clampVolume(volume);
    }
  }

  close(): Promise<void> {
    this.state = 'closed';
    return Promise.resolve();
  }

  update(status: AircraftStatus | undefined): void {
    this.lastStatus = status;
    this.updates += 1;
  }

  setCamera(camera: CameraMode): void {
    this.camera = camera;
  }

  setBuses(levels: Partial<BusLevels>): void {
    this.buses = { ...this.buses, ...levels };
    if (levels.master !== undefined) this.setVolume(levels.master);
  }

  setMuted(muted: boolean): void {
    this.muted = muted;
  }

  onOnset(): () => void {
    return () => undefined;
  }
}
