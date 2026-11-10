/**
 * The application state machine (GDD 53.3), as a pure reducer:
 *
 *     BOOT -> COMPAT_CHECK -> FIRST_RUN -> MAIN_MENU <-> DEMO
 *                  |   \_________________^   ^
 *                  |                         | (?fly=1 and performance runs go straight to DEMO)
 *                  \-> FATAL  <- any
 *
 * `FIRST_RUN` is the setup flow of a first visit (a step at a time, skippable at any point);
 * `MAIN_MENU` is where Fly starts a sortie and the settings live (`screen`); `DEMO` is the flight.
 * `DEMO` carries two things that are independent: whether the simulation is `paused` (the debug
 * overlay's button, or the tab being hidden), and which `menu` is open over the flight: none, the
 * pause menu, or the controls editor opened from it. The page holds the simulation while a menu is
 * open, and the pause menu's "Main menu" returns to `MAIN_MENU`, where the sortie is over.
 *
 * `COMPAT_CHECK` lasts from the first probe until the check has passed and, when the screen is
 * shown, the person has continued; it remembers where the person goes next. `FATAL` is final.
 * Events that make no sense in a state are ignored, returning the same state object, so callers can
 * dispatch without checking first.
 */
import type { CompatCode, Verdict } from './compat.ts';

/** The menu open over the flight, if any (the debrief and the questionnaire hold it too). */
export type DemoMenu = null | 'pause' | 'controls' | 'debrief' | 'feel';

/** Where the game goes once the compat check is through. */
export type StartTarget = 'first-run' | 'menu' | 'fly';

/** The first-run flow's steps, in order (the calibration step is only shown for a joystick). */
export const FIRST_RUN_STEPS = [
  'device',
  'calibration',
  'assist',
  'comfort',
  'notice',
  'done',
] as const;
export type FirstRunStep = (typeof FIRST_RUN_STEPS)[number];

/** What the main menu shows: the home screen, the settings or the statistics. */
export type MenuScreen = 'home' | 'settings' | 'statistics';

export type FlowState =
  | { name: 'BOOT' }
  | {
      name: 'COMPAT_CHECK';
      /** `probing` until the report arrives, then `waiting` for the person to continue. */
      phase: 'probing' | 'waiting';
      /** Where to go on continuing (only while `waiting`). */
      after?: StartTarget;
    }
  | { name: 'FIRST_RUN'; step: FirstRunStep }
  | { name: 'MAIN_MENU'; screen: MenuScreen }
  | { name: 'DEMO'; paused: boolean; menu: DemoMenu }
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
      /** Where to go next (default: the flight, as before there was a menu). */
      after?: StartTarget;
    }
  | { type: 'acknowledge' }
  /** Go to a step of the first-run flow. */
  | { type: 'first-run-step'; step: FirstRunStep }
  /** The first-run flow is finished or skipped: on to the main menu. */
  | { type: 'first-run-done' }
  /** Open the first-run flow again (from the main menu's settings). */
  | { type: 'run-setup' }
  /** Show the home screen or the settings in the main menu. */
  | { type: 'menu-screen'; screen: MenuScreen }
  /** Fly: leave the main menu for a sortie. */
  | { type: 'fly' }
  /** The sortie is over: back to the main menu. */
  | { type: 'main-menu' }
  | { type: 'pause' }
  | { type: 'resume' }
  /** Open a menu over the flight, or close it (`null`). */
  | { type: 'menu'; menu: DemoMenu }
  | { type: 'fatal'; code: string; message: string };

export const INITIAL_FLOW: FlowState = { name: 'BOOT' };

/** The state a start target leads to. */
function startState(target: StartTarget): FlowState {
  switch (target) {
    case 'first-run':
      return { name: 'FIRST_RUN', step: 'device' };
    case 'menu':
      return { name: 'MAIN_MENU', screen: 'home' };
    case 'fly':
      return { name: 'DEMO', paused: false, menu: null };
  }
}

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
        ? { name: 'COMPAT_CHECK', phase: 'waiting', after: event.after ?? 'fly' }
        : startState(event.after ?? 'fly');
    case 'acknowledge':
      return state.name === 'COMPAT_CHECK' && state.phase === 'waiting'
        ? startState(state.after ?? 'fly')
        : state;
    case 'first-run-step':
      return state.name === 'FIRST_RUN' && state.step !== event.step
        ? { name: 'FIRST_RUN', step: event.step }
        : state;
    case 'first-run-done':
      return state.name === 'FIRST_RUN' ? { name: 'MAIN_MENU', screen: 'home' } : state;
    case 'run-setup':
      return state.name === 'MAIN_MENU' ? { name: 'FIRST_RUN', step: 'device' } : state;
    case 'menu-screen':
      return state.name === 'MAIN_MENU' && state.screen !== event.screen
        ? { name: 'MAIN_MENU', screen: event.screen }
        : state;
    case 'fly':
      return state.name === 'MAIN_MENU' ? { name: 'DEMO', paused: false, menu: null } : state;
    case 'main-menu':
      return state.name === 'DEMO' ? { name: 'MAIN_MENU', screen: 'home' } : state;
    case 'pause':
      return state.name === 'DEMO' && !state.paused ? { ...state, paused: true } : state;
    case 'resume':
      return state.name === 'DEMO' && state.paused ? { ...state, paused: false } : state;
    case 'menu':
      return state.name === 'DEMO' && state.menu !== event.menu
        ? { ...state, menu: event.menu }
        : state;
    case 'fatal':
      return { name: 'FATAL', reason: 'runtime', code: event.code, message: event.message };
  }
}
