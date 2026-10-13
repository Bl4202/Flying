/**
 * Connects the browser's events to an `InputPump`: key presses and releases, focus, visibility,
 * and the two clocks that sample devices between events (every animation frame, and every 4 ms
 * while the tab has focus, to catch sub-frame changes on fast displays).
 *
 * Everything is injected so it can be tested with plain event targets.
 */
import { boundKeyCodes, type KeyboardBindings } from './bindings.ts';
import type { KeyboardState } from './keyboard.ts';
import type { InputPump } from './pump.ts';

export interface KeyEventLike {
  readonly code: string;
  readonly repeat: boolean;
  readonly target?: unknown;
  readonly ctrlKey?: boolean;
  readonly metaKey?: boolean;
  readonly altKey?: boolean;
  preventDefault(): void;
}

export interface Scheduler {
  requestFrame(callback: () => void): number;
  cancelFrame(id: number): void;
  setInterval(callback: () => void, ms: number): number;
  clearInterval(id: number): void;
}

export const BROWSER_SCHEDULER: Scheduler = {
  requestFrame: (cb) => requestAnimationFrame(cb),
  cancelFrame: (id) => {
    cancelAnimationFrame(id);
  },
  setInterval: (cb, ms) => window.setInterval(cb, ms),
  clearInterval: (id) => {
    window.clearInterval(id);
  },
};

export interface InputTargets {
  /** Receives `keydown`, `keyup`, `blur` and `focus`. */
  window: EventTarget;
  /** Receives `visibilitychange`; `visibilityState` says whether the tab is hidden. */
  document: EventTarget & { readonly visibilityState: string };
}

export interface AttachOptions {
  keyboard: KeyboardState;
  bindings: KeyboardBindings;
  pump: InputPump;
  scheduler?: Scheduler;
  /** Milliseconds between samples on the timer. */
  timerMs?: number;
  /** Whether the tab has focus right now. */
  hasFocus?: boolean;
}

function isTyping(target: unknown): boolean {
  const tag = (target as { tagName?: unknown } | null | undefined)?.tagName;
  return typeof tag === 'string' && /^(INPUT|TEXTAREA|SELECT)$/i.test(tag);
}

/** Starts feeding `pump`. Returns a function that stops everything and removes the listeners. */
export function attachInput(targets: InputTargets, opts: AttachOptions): () => void {
  const scheduler = opts.scheduler ?? BROWSER_SCHEDULER;
  const bound = boundKeyCodes(opts.bindings);
  let focused = opts.hasFocus ?? true;
  let frameId: number | undefined;
  let timerId: number | undefined;

  const sample = (): void => {
    if (focused) opts.pump.update();
  };
  const startClocks = (): void => {
    const loop = (): void => {
      sample();
      frameId = scheduler.requestFrame(loop);
    };
    frameId ??= scheduler.requestFrame(loop);
    timerId ??= scheduler.setInterval(sample, opts.timerMs ?? 4);
  };
  const stopClocks = (): void => {
    if (frameId !== undefined) scheduler.cancelFrame(frameId);
    if (timerId !== undefined) scheduler.clearInterval(timerId);
    frameId = undefined;
    timerId = undefined;
  };
  const lose = (): void => {
    if (!focused) return;
    focused = false;
    stopClocks();
    opts.keyboard.releaseAll();
    opts.pump.neutral();
  };
  const gain = (): void => {
    if (focused && frameId !== undefined) return;
    focused = true;
    startClocks();
  };

  const onKeyDown = (e: Event): void => {
    const event = e as unknown as KeyEventLike;
    if (isTyping(event.target)) return;
    if (bound.has(event.code)) event.preventDefault();
    if (opts.keyboard.keyDown(event.code, event.repeat)) sample();
  };
  const onKeyUp = (e: Event): void => {
    const event = e as unknown as KeyEventLike;
    if (opts.keyboard.keyUp(event.code)) sample();
  };
  const onBlur = (): void => {
    lose();
  };
  const onFocus = (): void => {
    gain();
  };
  const onVisibility = (): void => {
    if (targets.document.visibilityState === 'hidden') lose();
    else gain();
  };

  targets.window.addEventListener('keydown', onKeyDown);
  targets.window.addEventListener('keyup', onKeyUp);
  targets.window.addEventListener('blur', onBlur);
  targets.window.addEventListener('focus', onFocus);
  targets.document.addEventListener('visibilitychange', onVisibility);
  if (focused) startClocks();

  return () => {
    stopClocks();
    targets.window.removeEventListener('keydown', onKeyDown);
    targets.window.removeEventListener('keyup', onKeyUp);
    targets.window.removeEventListener('blur', onBlur);
    targets.window.removeEventListener('focus', onFocus);
    targets.document.removeEventListener('visibilitychange', onVisibility);
  };
}
