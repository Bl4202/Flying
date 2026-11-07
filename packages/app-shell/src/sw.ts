/**
 * Service Worker (GDD 61.12, ADR-19, ADR-22 as amended by ADR-44).
 *
 * 1. Offline shell: precaches everything the build emitted, under a cache named for the build, and
 *    answers from it first (`sw-strategy.ts`).
 * 2. Isolation: re-applies COOP/COEP/CORP/CSP to every same-origin response, which is the only way
 *    to get cross-origin isolation on a static host that cannot set headers.
 * 3. Updates: a new worker installs beside the running one and waits. It takes over only when the
 *    player asks, from the main menu: the page sends `skip-waiting` and reloads when the new worker
 *    is in control. A flight is never interrupted, and an old build never gets a new page.
 */
import { withSecurityHeaders } from '@flying/platform-web/sw-headers';
import { classify } from '@flying/platform-web/sw-routing';
import { cacheNameFor, dropOldCaches, respond } from '@flying/platform-web/sw-strategy';
import { SKIP_WAITING } from '@flying/platform-web/sw-update';

interface ExtendableEventLike extends Event {
  waitUntil(promise: Promise<unknown>): void;
}
interface FetchEventLike extends ExtendableEventLike {
  readonly request: Request;
  respondWith(response: Promise<Response> | Response): void;
}
interface MessageEventLike extends ExtendableEventLike {
  readonly data: unknown;
}
interface SwScope {
  readonly registration: { readonly scope: string };
  readonly clients: { claim(): Promise<void> };
  skipWaiting(): Promise<void>;
  addEventListener(type: 'install' | 'activate', listener: (e: ExtendableEventLike) => void): void;
  addEventListener(type: 'fetch', listener: (e: FetchEventLike) => void): void;
  addEventListener(type: 'message', listener: (e: MessageEventLike) => void): void;
}

const scope = self as unknown as SwScope;
const CACHE = cacheNameFor(__BUILD_ID__);

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

scope.addEventListener('install', (event) => {
  event.waitUntil(precache());
});

scope.addEventListener('activate', (event) => {
  event.waitUntil(dropOldCaches(caches, CACHE).then(() => scope.clients.claim()));
});

// The page asks a waiting worker to take over (the player chose "Update ready — reload").
scope.addEventListener('message', (event) => {
  const data = event.data as { type?: unknown } | null;
  if (data?.type === SKIP_WAITING.type) {
    event.waitUntil(scope.skipWaiting());
  }
});

scope.addEventListener('fetch', (event) => {
  const route = classify(event.request, scope.registration.scope);
  if (route === 'passthrough') {
    return;
  }
  event.respondWith(
    respond(
      {
        caches,
        cacheName: CACHE,
        scopeUrl: scope.registration.scope,
        fetch: (request) => fetch(request),
        secure: withSecurityHeaders,
      },
      event.request,
      route,
    ),
  );
});
