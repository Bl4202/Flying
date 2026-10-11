/**
 * The single source of truth for response security headers (GDD 61.11, 61.12, ADR-17).
 *
 * Consumed by three places so they can never drift apart:
 *  - the Vite dev/preview servers,
 *  - the generated `dist/_headers` file for hosts that support it,
 *  - the Service Worker, which re-applies them to every same-origin response (the only way to get
 *    cross-origin isolation on a host such as GitHub Pages that cannot set headers).
 */

/** Name of the Trusted Types policy used by the app (see `trusted-types.ts`). */
export const TRUSTED_TYPES_POLICY = 'flying-app';

/** Headers that make the page cross-origin isolated (SharedArrayBuffer, WASM threads). */
export const ISOLATION_HEADERS: Readonly<Record<string, string>> = {
  'Cross-Origin-Opener-Policy': 'same-origin',
  'Cross-Origin-Embedder-Policy': 'require-corp',
  'Cross-Origin-Resource-Policy': 'same-origin',
};

/**
 * Content Security Policy. No inline script or style, no eval; `wasm-unsafe-eval` is the one
 * documented exception, needed to compile WebAssembly. Trusted Types are enforced for scripts.
 */
export const CSP_DIRECTIVES: readonly (readonly [string, string])[] = [
  ['default-src', "'self'"],
  ['script-src', "'self' 'wasm-unsafe-eval'"],
  ['worker-src', "'self'"],
  ['style-src', "'self'"],
  ['img-src', "'self' data:"],
  ['connect-src', "'self'"],
  ['object-src', "'none'"],
  ['base-uri', "'self'"],
  ['require-trusted-types-for', "'script'"],
  ['trusted-types', TRUSTED_TYPES_POLICY],
];

/** Directives allowed in a `<meta http-equiv>` policy (`frame-ancestors` etc. are header-only). */
export function cspValue(): string {
  return CSP_DIRECTIVES.map(([name, value]) => `${name} ${value}`).join('; ');
}

/** Other hardening headers. */
export const HARDENING_HEADERS: Readonly<Record<string, string>> = {
  'X-Content-Type-Options': 'nosniff',
  'Referrer-Policy': 'no-referrer',
};

/** Everything the Service Worker and production hosts send. */
export function securityHeaders(): Record<string, string> {
  return {
    ...ISOLATION_HEADERS,
    'Content-Security-Policy': cspValue(),
    ...HARDENING_HEADERS,
  };
}
