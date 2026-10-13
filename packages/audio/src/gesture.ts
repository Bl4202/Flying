import type { AudioBackend } from './backend.ts';

/** The slice of an event target the gesture hook listens on. */
export interface GestureTarget {
  addEventListener(type: string, listener: () => void, options?: { once?: boolean }): void;
  removeEventListener(type: string, listener: () => void): void;
}

/** Events that count as a user gesture for audio autoplay policies. */
export const GESTURE_EVENTS = ['keydown', 'pointerdown'] as const;

/**
 * Resumes `audio` on the first gesture and then stops listening. Returns a function that removes
 * the listeners if no gesture ever came.
 */
export function resumeOnFirstGesture(audio: AudioBackend, target: GestureTarget): () => void {
  const stop = (): void => {
    for (const type of GESTURE_EVENTS) {
      target.removeEventListener(type, onGesture);
    }
  };
  function onGesture(): void {
    stop();
    void audio.resume();
  }
  for (const type of GESTURE_EVENTS) {
    target.addEventListener(type, onGesture);
  }
  return stop;
}
