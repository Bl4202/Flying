import { scriptUrl } from './trusted-types.ts';

export type IsolationOutcome = 'isolated' | 'reloading' | 'degraded';
export type ServiceWorkerStatus = 'unsupported' | 'active' | 'failed';

const GUARD_KEY = 'fg-coi-reload';

export interface IsolationState {
  isolated: boolean;
  swSupported: boolean;
  swFailed: boolean;
  reloadedAlready: boolean;
}

/**
 * Pure decision table. A host that cannot set COOP/COEP gets them from our Service Worker, but only
 * on loads the worker controls, so after the first install the page reloads exactly once.
 */
export function decideIsolation(s: IsolationState): 'isolated' | 'reload' | 'degraded' {
  if (s.isolated) {
    return 'isolated';
  }
  if (!s.swSupported || s.swFailed || s.reloadedAlready) {
    return 'degraded';
  }
  return 'reload';
}

function guardGet(): boolean {
  try {
    return sessionStorage.getItem(GUARD_KEY) === '1';
  } catch {
    return true; // cannot guard against loops, so never reload
  }
}

function guardSet(on: boolean): void {
  try {
    if (on) {
      sessionStorage.setItem(GUARD_KEY, '1');
    } else {
      sessionStorage.removeItem(GUARD_KEY);
    }
  } catch {
    // storage unavailable
  }
}

function waitForController(ms: number): Promise<void> {
  return new Promise((resolve) => {
    if (navigator.serviceWorker.controller) {
      resolve();
      return;
    }
    const timer = setTimeout(resolve, ms);
    navigator.serviceWorker.addEventListener(
      'controllerchange',
      () => {
        clearTimeout(timer);
        resolve();
      },
      { once: true },
    );
  });
}

/** Rejects if `promise` has not settled within `ms`. */
export function withTimeout<T>(promise: Promise<T>, ms: number, what: string): Promise<T> {
  return new Promise<T>((resolve, reject) => {
    const timer = setTimeout(() => {
      reject(new Error(`${what} timed out after ${String(ms)} ms`));
    }, ms);
    promise.then(
      (v) => {
        clearTimeout(timer);
        resolve(v);
      },
      (e: unknown) => {
        clearTimeout(timer);
        reject(e instanceof Error ? e : new Error(String(e)));
      },
    );
  });
}

export interface RegisterOptions {
  swUrl: string;
  scope: string;
  controllerWaitMs?: number;
  /** Upper bound for register + ready; a blocked or broken worker must not hang boot. */
  registerTimeoutMs?: number;
}

/**
 * Registers the Service Worker and, if the page is not cross-origin isolated, reloads once so the
 * worker can serve it with COOP/COEP.
 */
export async function ensureIsolation(
  opts: RegisterOptions,
): Promise<{ outcome: IsolationOutcome; serviceWorker: ServiceWorkerStatus }> {
  const swSupported = 'serviceWorker' in navigator;
  let swFailed = false;
  if (swSupported) {
    try {
      await withTimeout(
        (async () => {
          await navigator.serviceWorker.register(scriptUrl(opts.swUrl), { scope: opts.scope });
          await navigator.serviceWorker.ready;
        })(),
        opts.registerTimeoutMs ?? 5000,
        'service worker registration',
      );
      await waitForController(opts.controllerWaitMs ?? 3000);
    } catch {
      swFailed = true;
    }
  }
  const serviceWorker: ServiceWorkerStatus = !swSupported
    ? 'unsupported'
    : swFailed
      ? 'failed'
      : 'active';
  const decision = decideIsolation({
    isolated: crossOriginIsolated,
    swSupported,
    swFailed,
    reloadedAlready: guardGet(),
  });
  if (decision === 'isolated') {
    guardSet(false);
    return { outcome: 'isolated', serviceWorker };
  }
  if (decision === 'reload') {
    guardSet(true);
    location.reload();
    return { outcome: 'reloading', serviceWorker };
  }
  return { outcome: 'degraded', serviceWorker };
}
