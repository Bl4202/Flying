import { clampVolume, type AudioBackend, type AudioCue, type AudioState } from './backend.ts';

/**
 * Makes no sound. It keeps the same state machine as a real backend (suspended until resumed,
 * closed for good) and records what it was asked to play, so tests and the debug overlay can see
 * that the game called it.
 */
export class NullAudioBackend implements AudioBackend {
  readonly kind = 'null';
  state: AudioState = 'suspended';
  volume = 1;
  /** Cues that would have sounded, oldest first. */
  readonly played: AudioCue[] = [];
  resumeCalls = 0;

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
}
