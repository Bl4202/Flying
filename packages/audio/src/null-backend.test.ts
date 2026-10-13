import { describe, expect, it } from 'vitest';
import { audioBackendContract } from './audio-backend.contract.ts';
import { NullAudioBackend } from './null-backend.ts';
import { GESTURE_EVENTS, resumeOnFirstGesture, type GestureTarget } from './gesture.ts';

audioBackendContract('null', () => new NullAudioBackend());

describe('NullAudioBackend', () => {
  it('records cues only while running', async () => {
    const a = new NullAudioBackend();
    a.cue('ui-confirm');
    await a.resume();
    a.cue('warning');
    await a.suspend();
    a.cue('ui-error');
    expect(a.played).toEqual(['warning']);
  });
});

class FakeTarget implements GestureTarget {
  readonly listeners = new Map<string, Set<() => void>>();
  addEventListener(type: string, listener: () => void): void {
    const set = this.listeners.get(type) ?? new Set();
    set.add(listener);
    this.listeners.set(type, set);
  }
  removeEventListener(type: string, listener: () => void): void {
    this.listeners.get(type)?.delete(listener);
  }
  fire(type: string): void {
    for (const l of [...(this.listeners.get(type) ?? [])]) l();
  }
  count(): number {
    return [...this.listeners.values()].reduce((n, s) => n + s.size, 0);
  }
}

describe('resumeOnFirstGesture', () => {
  it('does nothing until a gesture, then resumes once and lets go of the listeners', () => {
    const audio = new NullAudioBackend();
    const target = new FakeTarget();
    resumeOnFirstGesture(audio, target);
    expect(audio.resumeCalls).toBe(0);
    expect(target.count()).toBe(GESTURE_EVENTS.length);

    target.fire('pointerdown');
    expect(audio.resumeCalls).toBe(1);
    expect(target.count()).toBe(0);
    target.fire('keydown');
    expect(audio.resumeCalls).toBe(1);
  });

  it('can be cancelled before any gesture', () => {
    const audio = new NullAudioBackend();
    const target = new FakeTarget();
    const cancel = resumeOnFirstGesture(audio, target);
    cancel();
    target.fire('keydown');
    expect(audio.resumeCalls).toBe(0);
    expect(target.count()).toBe(0);
  });
});
