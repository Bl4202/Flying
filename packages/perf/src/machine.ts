import type { MachineInfo } from './report.ts';

/** The parts of the global scope that describe the machine; injectable for tests. */
export interface MachineEnv {
  navigator: {
    userAgent: string;
    platform?: string;
    hardwareConcurrency?: number;
    deviceMemory?: number;
  };
  screen?: { width: number; height: number };
  devicePixelRatio?: number;
}

/**
 * What a report says about the machine it ran on. Nothing here identifies the person: it is what
 * any web page can read, and the same facts a bug report would ask for.
 */
export function describeMachine(env: MachineEnv, adapter: string, renderer: string): MachineInfo {
  const screen = env.screen;
  return {
    userAgent: env.navigator.userAgent,
    platform: env.navigator.platform ?? '',
    hardwareConcurrency: env.navigator.hardwareConcurrency ?? 0,
    deviceMemoryGb: env.navigator.deviceMemory ?? null,
    screen:
      screen === undefined
        ? ''
        : `${String(screen.width)}x${String(screen.height)}@${String(env.devicePixelRatio ?? 1)}`,
    adapter,
    renderer,
  };
}

/** The real global scope, for the app. */
export function browserMachineEnv(): MachineEnv {
  return globalThis;
}
