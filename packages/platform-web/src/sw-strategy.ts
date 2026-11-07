/**
 * What the Service Worker answers (GDD 61.12, ADR-19, ADR-22 as amended by ADR-44). The logic is
 * here, with the caches and the network passed in, so it is tested in Node with fakes; `sw.ts` only
 * wires it to the worker's globals.
 *
 * Each build's worker has a cache of its own, filled when the worker installs. Its answers come from
 * that cache first:
 *
 * - **Navigations** are answered by the build's own shell, and the network is asked only when the
 *   cache has none (a first visit). A page that opens is therefore always the build whose worker is
 *   in control, with the assets that build was made with. A newer build installs beside it, in its
 *   own cache, and waits; the old cache is never written to, so a new `index.html` can never end up
 *   under an old build's name.
 * - **Assets** come from the build's cache, then from the cache of another build (a tab of an older
 *   build still loading a chunk, or a newer build in the middle of installing), then the network.
 *   Everything a build emits is content-addressed, so a copy found anywhere is the right one.
 */
import type { Route } from './sw-routing.ts';

/** The prefix of every cache this worker creates; anything else belongs to someone else. */
export const CACHE_PREFIX = 'fg-';

/**
 * Entries are stored from requests the worker builds itself, which carry no `Origin` header, while
 * the page's `crossorigin` module and stylesheet requests do. A host that answers `Vary: Origin`
 * would make every lookup miss. The caches are per build and the assets content-addressed, so `Vary`
 * carries no information here.
 */
export const MATCH: CacheQueryOptions = { ignoreVary: true };

export interface CacheLike {
  match(request: Request | string, options?: CacheQueryOptions): Promise<Response | undefined>;
  put(request: Request | string, response: Response): Promise<void>;
}

export interface CacheStorageLike {
  open(name: string): Promise<CacheLike>;
  keys(): Promise<string[]>;
  delete(name: string): Promise<boolean>;
}

export interface StrategyDeps {
  caches: CacheStorageLike;
  /** This worker's cache: the one its build filled. */
  cacheName: string;
  /** The registration scope, for example `https://host/Flying/`. */
  scopeUrl: string;
  fetch: (request: Request) => Promise<Response>;
  /** Puts the isolation and security headers on what the page is given. */
  secure: (response: Response) => Response;
}

/** The cache name for a build. */
export function cacheNameFor(buildId: string): string {
  return `${CACHE_PREFIX}${buildId}`;
}

/** The URL the app's shell is cached under: the scope itself. */
export function shellUrl(scopeUrl: string): string {
  return new URL('./', scopeUrl).href;
}

/** Answers `request` as `route` says. Never rejects: a failure is a network error response. */
export async function respond(
  deps: StrategyDeps,
  request: Request,
  route: Exclude<Route, 'passthrough'>,
): Promise<Response> {
  const own = await deps.caches.open(deps.cacheName);
  if (route === 'navigation') {
    const shell = shellUrl(deps.scopeUrl);
    const cached = await own.match(shell, MATCH);
    if (cached) return deps.secure(cached);
    // Nothing precached (a first visit, before the worker finished installing): the network, kept.
    try {
      const fresh = await deps.fetch(request);
      if (fresh.ok) await own.put(shell, fresh.clone());
      return deps.secure(fresh);
    } catch {
      return Response.error();
    }
  }
  const hit = await own.match(request, MATCH);
  if (hit) return deps.secure(hit);
  for (const name of await deps.caches.keys()) {
    if (!name.startsWith(CACHE_PREFIX) || name === deps.cacheName) continue;
    const other = await (await deps.caches.open(name)).match(request, MATCH);
    if (other) return deps.secure(other);
  }
  try {
    return deps.secure(await deps.fetch(request));
  } catch {
    return Response.error();
  }
}

/** Deletes the caches of other builds (when this worker takes over). Returns the names it deleted. */
export async function dropOldCaches(caches: CacheStorageLike, keep: string): Promise<string[]> {
  const stale = (await caches.keys()).filter((n) => n.startsWith(CACHE_PREFIX) && n !== keep);
  await Promise.all(stale.map((n) => caches.delete(n)));
  return stale;
}
