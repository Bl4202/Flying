export type Route = 'navigation' | 'asset' | 'passthrough';

/**
 * Decides how the Service Worker handles a request.
 *
 * - `navigation`: network-first with a cached-shell fallback (so updates are picked up when online
 *   and the game still opens offline).
 * - `asset`: cache-first. Everything the build emits is content-addressed, so a cached copy is
 *   never stale.
 * - `passthrough`: left to the browser (non-GET, cross-origin, outside the scope).
 *
 * `scopeUrl` is the registration scope, e.g. `https://host/Flying/`.
 */
export function classify(
  request: { url: string; method: string; mode: string },
  scopeUrl: string,
): Route {
  if (request.method !== 'GET') {
    return 'passthrough';
  }
  const url = new URL(request.url);
  const scope = new URL(scopeUrl);
  if (url.origin !== scope.origin || !url.pathname.startsWith(scope.pathname)) {
    return 'passthrough';
  }
  if (request.mode === 'navigate') {
    return 'navigation';
  }
  return 'asset';
}
