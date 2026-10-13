/** What every audio backend must do, whatever makes the sound. */
import { describe, expect, it } from 'vitest';
import type { AudioBackend } from './backend.ts';

export function audioBackendContract(name: string, make: () => AudioBackend): void {
  describe(`audio backend (${name})`, () => {
    it('starts suspended at full volume', () => {
      const a = make();
      expect(a.state).toBe('suspended');
      expect(a.volume).toBe(1);
    });

    it('runs after resume, and resume can be repeated', async () => {
      const a = make();
      await a.resume();
      await a.resume();
      expect(a.state).toBe('running');
    });

    it('suspends and resumes again', async () => {
      const a = make();
      await a.resume();
      await a.suspend();
      expect(a.state).toBe('suspended');
      await a.resume();
      expect(a.state).toBe('running');
    });

    it('accepts cues in any state without throwing', async () => {
      const a = make();
      expect(() => {
        a.cue('ui-confirm');
      }).not.toThrow();
      await a.resume();
      expect(() => {
        a.cue('warning');
      }).not.toThrow();
      await a.close();
      expect(() => {
        a.cue('ui-error');
      }).not.toThrow();
    });

    it('clamps the volume to 0..1 and treats non-finite values as silent', () => {
      const a = make();
      a.setVolume(0.25);
      expect(a.volume).toBe(0.25);
      a.setVolume(7);
      expect(a.volume).toBe(1);
      a.setVolume(-3);
      expect(a.volume).toBe(0);
      a.setVolume(Number.NaN);
      expect(a.volume).toBe(0);
    });

    it('is closed for good after close', async () => {
      const a = make();
      await a.resume();
      await a.close();
      expect(a.state).toBe('closed');
      await a.resume();
      expect(a.state).toBe('closed');
      a.setVolume(0.5);
      expect(a.volume).not.toBe(0.5);
    });
  });
}
