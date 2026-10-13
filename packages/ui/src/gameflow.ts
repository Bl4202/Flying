/**
 * The application state machine (GDD 53.3), as a pure reducer. v0.0b has the states that exist
 * before there is a menu:
 *
 *     BOOT -> COMPAT_CHECK -> DEMO
 *                  \-> FATAL  <- DEMO
 *
 * `COMPAT_CHECK` lasts from the first probe until the check has passed and, when the screen is
 * shown, the person has continued. `FATAL` is final. Events that make no sense in a state are
 * ignored, returning the same state object, so callers can dispatch without checking first.
 */
import type { CompatCode, Verdict } from './compat.ts';

export type FlowState =
  | { name: 'BOOT' }
  | {
      name: 'COMPAT_CHECK';
      /** `probing` until the report arrives, then `waiting` for the person to continue. */
      phase: 'probing' | 'waiting';
    }
  | { name: 'DEMO'; paused: boolean }
  | {
      name: 'FATAL';
      /** `compat`: the browser cannot run the game; `runtime`: something died while running. */
      reason: 'compat' | 'runtime';
      code: string;
      message: string;
    };

export type FlowEvent =
  | { type: 'begin-check' }
  | {
      type: 'check-done';
      verdict: Verdict;
      /** Whether the screen is shown and has to be acknowledged. */
      showScreen: boolean;
      /** The first missing required feature's code, when fatal. */
      code?: CompatCode;
    }
  | { type: 'acknowledge' }
  | { type: 'pause' }
  | { type: 'resume' }
  | { type: 'fatal'; code: string; message: string };

export const INITIAL_FLOW: FlowState = { name: 'BOOT' };

export function flowReducer(state: FlowState, event: FlowEvent): FlowState {
  if (state.name === 'FATAL') {
    return state;
  }
  switch (event.type) {
    case 'begin-check':
      return state.name === 'BOOT' ? { name: 'COMPAT_CHECK', phase: 'probing' } : state;
    case 'check-done':
      if (state.name !== 'COMPAT_CHECK' || state.phase !== 'probing') {
        return state;
      }
      if (event.verdict === 'fatal') {
        return {
          name: 'FATAL',
          reason: 'compat',
          code: event.code ?? 'SYS-103',
          message: 'compat check failed',
        };
      }
      return event.showScreen
        ? { name: 'COMPAT_CHECK', phase: 'waiting' }
        : { name: 'DEMO', paused: false };
    case 'acknowledge':
      return state.name === 'COMPAT_CHECK' && state.phase === 'waiting'
        ? { name: 'DEMO', paused: false }
        : state;
    case 'pause':
      return state.name === 'DEMO' && !state.paused ? { name: 'DEMO', paused: true } : state;
    case 'resume':
      return state.name === 'DEMO' && state.paused ? { name: 'DEMO', paused: false } : state;
    case 'fatal':
      return { name: 'FATAL', reason: 'runtime', code: event.code, message: event.message };
  }
}
