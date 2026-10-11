import { TRUSTED_TYPES_POLICY } from './headers.ts';

interface TrustedTypesApi {
  createPolicy(
    name: string,
    rules: { createScriptURL: (input: string) => string },
  ): { createScriptURL(input: string): unknown };
}

let policy: { createScriptURL(input: string): unknown } | undefined;

/**
 * Wraps a same-origin script URL for `new Worker(...)` and `serviceWorker.register(...)`, which
 * are Trusted Types sinks under our CSP. Cross-origin URLs are rejected.
 *
 * Returns the plain string when the browser has no Trusted Types support.
 */
export function scriptUrl(url: string, origin: string = location.origin): string {
  const resolved = new URL(url, origin);
  if (resolved.origin !== origin) {
    throw new Error(`refusing cross-origin script URL ${resolved.href}`);
  }
  const tt = (globalThis as { trustedTypes?: TrustedTypesApi }).trustedTypes;
  if (!tt) {
    return resolved.href;
  }
  policy ??= tt.createPolicy(TRUSTED_TYPES_POLICY, { createScriptURL: (s) => s });
  // The sinks accept a TrustedScriptURL object where a string is typed.
  return policy.createScriptURL(resolved.href) as string;
}
