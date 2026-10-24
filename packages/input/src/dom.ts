/**
 * Connects the browser's events to an `InputPump`: key presses and releases, focus, visibility,
 * and the two clocks that sample devices between events (every animation frame, and every 4 ms
 * while the tab has focus, to catch sub-frame changes on fast displays).
 *
 * A key event tells the pump when it happened (`poke(event.timeStamp)`) and samples at once, so
 * the frame that carries the press is written inside the event handler. Mouse events are handled
 * by `MouseDevice`, which pokes the pump the same way.
 *
 * Everything is injected so it can be tested with plain event targets.
 */
import type { KeyboardState } from './keyboard.ts';
import type { InputPump } from './pump.ts';

export interface KeyEventLike {
  readonly code: string;
  readonly repeat: boolean;
  readonly target?: unknown;
  /** Milliseconds on the clock of the pump's `nowMs` (`performance.now()` for real events). */
  readonly timeStamp?: number;
  readonly ctrlKey?: boolean;
  readonly shiftKey?: boolean;
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
  /** Receives `keydown`, `keyup`, `blur`, `focus` and the gamepad connection events. */
  window: EventTarget;
  /** Receives `visibilitychange`; `visibilityState` says whether the tab is hidden. */
  document: EventTarget & { readonly visibilityState: string };
}

export interface AttachOptions {
  keyboard: KeyboardState;
  pump: InputPump;
  scheduler?: Scheduler;
  /** Milliseconds between samples on the timer. */
  timerMs?: number;
  /** Whether the tab has focus right now. */
  hasFocus?: boolean;
  /**
   * Key presses for which this returns true belong to the page (the debug overlay's chord) and
   * never reach the flight controls.
   */
  swallow?: (event: KeyEventLike) => boolean;
}

function isTyping(target: unknown): boolean {
  const tag = (target as { tagName?: unknown } | null | undefined)?.tagName;
  return typeof tag === 'string' && /^(INPUT|TEXTAREA|SELECT)$/i.test(tag);
}

/** Starts feeding `pump`. Returns a function that stops everything and removes the listeners. */
export function attachInput(targets: InputTargets, opts: AttachOptions): () => void {
  const scheduler = opts.scheduler ?? BROWSER_SCHEDULER;
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
    if (opts.swallow?.(event) === true) return;
    if (opts.pump.bindings.keyCodes.has(event.code)) event.preventDefault();
    if (opts.keyboard.keyDown(event.code, event.repeat)) {
      opts.pump.poke(event.timeStamp);
      sample();
    }
  };
  const onKeyUp = (e: Event): void => {
    const event = e as unknown as KeyEventLike;
    if (opts.keyboard.keyUp(event.code)) {
      opts.pump.poke(event.timeStamp);
      sample();
    }
  };
  // A device that appears or goes is noticed at the next sample anyway; this makes it the next
  // millisecond instead of the next few.
  const onDevice = (): void => {
    sample();
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
  targets.window.addEventListener('gamepadconnected', onDevice);
  targets.window.addEventListener('gamepaddisconnected', onDevice);
  targets.window.addEventListener('blur', onBlur);
  targets.window.addEventListener('focus', onFocus);
  targets.document.addEventListener('visibilitychange', onVisibility);
  if (focused) startClocks();

  return () => {
    stopClocks();
    targets.window.removeEventListener('keydown', onKeyDown);
    targets.window.removeEventListener('keyup', onKeyUp);
    targets.window.removeEventListener('gamepadconnected', onDevice);
    targets.window.removeEventListener('gamepaddisconnected', onDevice);
    targets.window.removeEventListener('blur', onBlur);
    targets.window.removeEventListener('focus', onFocus);
    targets.document.removeEventListener('visibilitychange', onVisibility);
  };
}
