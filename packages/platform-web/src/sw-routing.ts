export type Route = 'navigation' | 'asset' | 'passthrough';

/**
 * Decides how the Service Worker handles a request.
 *
 * - `navigation`: the build's own cached shell (the network only on a first visit), so a page that
 *   opens is always the build in control; a newer build waits until the player asks for it.
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
