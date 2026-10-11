/**
 * Service Worker (GDD 61.12, ADR-19, ADR-22).
 *
 * 1. Offline shell: precaches everything the build emitted, under a cache named for the build.
 * 2. Isolation: re-applies COOP/COEP/CORP/CSP to every same-origin response, which is the only way
 *    to get cross-origin isolation on a static host that cannot set headers.
 * 3. Updates: a new worker installs and waits; it never skips waiting, so a mission is never
 *    interrupted. It takes over on the next launch.
 */
import { withSecurityHeaders } from '@flying/platform-web/sw-headers';
import { classify } from '@flying/platform-web/sw-routing';

interface ExtendableEventLike extends Event {
  waitUntil(promise: Promise<unknown>): void;
}
interface FetchEventLike extends ExtendableEventLike {
  readonly request: Request;
  respondWith(response: Promise<Response> | Response): void;
}
interface SwScope {
  readonly registration: { readonly scope: string };
  readonly clients: { claim(): Promise<void> };
  addEventListener(type: 'install' | 'activate', listener: (e: ExtendableEventLike) => void): void;
  addEventListener(type: 'fetch', listener: (e: FetchEventLike) => void): void;
}

const scope = self as unknown as SwScope;
const CACHE_PREFIX = 'fg-';
const CACHE = `${CACHE_PREFIX}${__BUILD_ID__}`;
/**
 * Entries are stored from requests the worker builds itself, which carry no `Origin` header, while
 * the page's `crossorigin` module and stylesheet requests do. A host that answers `Vary: Origin`
 * would therefore make every lookup miss (and offline there is nothing to fall back to). The cache
 * is per build and the assets are content-addressed, so `Vary` carries no information here.
 */
const MATCH: CacheQueryOptions = { ignoreVary: true };

interface BuildManifest {
  files: string[];
}

async function precache(): Promise<void> {
  const base = scope.registration.scope;
  const response = await fetch(
    new Request(new URL('build-manifest.json', base), { cache: 'no-store' }),
  );
  if (!response.ok) {
    throw new Error(`SYS-020: build-manifest.json returned ${String(response.status)}`);
  }
  const manifest = (await response.json()) as BuildManifest;
  const urls = ['./', 'build-manifest.json', ...manifest.files].map((f) => new URL(f, base).href);
  const cache = await caches.open(CACHE);
  // `reload` bypasses the HTTP cache so a stale copy is never pinned as this build's shell.
  await cache.addAll(urls.map((u) => new Request(u, { cache: 'reload' })));
}

async function dropOldCaches(): Promise<void> {
  const names = await caches.keys();
  await Promise.all(
    names.filter((n) => n.startsWith(CACHE_PREFIX) && n !== CACHE).map((n) => caches.delete(n)),
  );
}

async function respond(request: Request, route: 'navigation' | 'asset'): Promise<Response> {
  const cache = await caches.open(CACHE);
  if (route === 'navigation') {
    try {
      const fresh = await fetch(request);
      if (fresh.ok) {
        await cache.put(new URL('./', scope.registration.scope).href, fresh.clone());
      }
      return withSecurityHeaders(fresh);
    } catch {
      const shell = await cache.match(new URL('./', scope.registration.scope).href, MATCH);
      return shell ? withSecurityHeaders(shell) : Response.error();
    }
  }
  const hit = await cache.match(request, MATCH);
  if (hit) {
    return withSecurityHeaders(hit);
  }
  try {
    return withSecurityHeaders(await fetch(request));
  } catch {
    return Response.error();
  }
}

scope.addEventListener('install', (event) => {
  event.waitUntil(precache());
});

scope.addEventListener('activate', (event) => {
  event.waitUntil(dropOldCaches().then(() => scope.clients.claim()));
});

scope.addEventListener('fetch', (event) => {
  const route = classify(event.request, scope.registration.scope);
  if (route === 'passthrough') {
    return;
  }
  event.respondWith(respond(event.request, route));
});
