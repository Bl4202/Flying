import { describe, expect, it } from 'vitest';
import {
  cacheNameFor,
  dropOldCaches,
  respond,
  shellUrl,
  type CacheLike,
  type CacheStorageLike,
  type StrategyDeps,
} from './sw-strategy.ts';

class FakeCache implements CacheLike {
  readonly store = new Map<string, Response>();
  readonly puts: string[] = [];
  match(request: Request | string): Promise<Response | undefined> {
    const hit = this.store.get(typeof request === 'string' ? request : request.url);
    return Promise.resolve(hit?.clone());
  }
  put(request: Request | string, response: Response): Promise<void> {
    const key = typeof request === 'string' ? request : request.url;
    this.puts.push(key);
    this.store.set(key, response);
    return Promise.resolve();
  }
}

class FakeStorage implements CacheStorageLike {
  readonly caches = new Map<string, FakeCache>();
  open(name: string): Promise<FakeCache> {
    let c = this.caches.get(name);
    if (!c) {
      c = new FakeCache();
      this.caches.set(name, c);
    }
    return Promise.resolve(c);
  }
  keys(): Promise<string[]> {
    return Promise.resolve([...this.caches.keys()]);
  }
  delete(name: string): Promise<boolean> {
    return Promise.resolve(this.caches.delete(name));
  }
}

const SCOPE = 'https://host.test/Flying/';
const A = cacheNameFor('aaaa');
const B = cacheNameFor('bbbb');

const text = async (r: Response): Promise<string> => r.text();

/** A build's worker: its cache holds the shell and an asset that build made. */
async function install(storage: FakeStorage, name: string, build: string): Promise<void> {
  const cache = await storage.open(name);
  cache.store.set(shellUrl(SCOPE), new Response(`shell ${build}`));
  cache.store.set(`${SCOPE}assets/app-${build}.js`, new Response(`app ${build}`));
}

function deps(storage: FakeStorage, cacheName: string, network: Map<string, string> | 'offline') {
  const fetched: string[] = [];
  const d: StrategyDeps = {
    caches: storage,
    cacheName,
    scopeUrl: SCOPE,
    fetch: (request) => {
      fetched.push(request.url);
      if (network === 'offline') return Promise.reject(new TypeError('offline'));
      const body = network.get(request.url);
      return Promise.resolve(
        body === undefined ? new Response('', { status: 404 }) : new Response(body),
      );
    },
    secure: (r) => {
      const out = new Response(r.body, r);
      out.headers.set('x-secured', '1');
      return out;
    },
  };
  return { d, fetched };
}

const nav = (): Request => new Request(SCOPE, { headers: { accept: 'text/html' } });
const asset = (build: string): Request => new Request(`${SCOPE}assets/app-${build}.js`);

describe('navigations', () => {
  it("are answered by the build's own shell, without asking the network", async () => {
    const storage = new FakeStorage();
    await install(storage, A, 'aaaa');
    const { d, fetched } = deps(storage, A, new Map([[SCOPE, 'shell from the network']]));
    const r = await respond(d, nav(), 'navigation');
    expect(await text(r)).toBe('shell aaaa');
    expect(r.headers.get('x-secured')).toBe('1');
    expect(fetched).toEqual([]);
  });

  it('never write a fresh page into the cache of a build that is already installed', async () => {
    const storage = new FakeStorage();
    await install(storage, A, 'aaaa');
    const { d } = deps(storage, A, new Map([[SCOPE, 'shell bbbb']]));
    await respond(d, nav(), 'navigation');
    expect((await storage.open(A)).puts).toEqual([]);
    expect(await text((await (await storage.open(A)).match(shellUrl(SCOPE))) as Response)).toBe(
      'shell aaaa',
    );
  });

  it('go to the network on a first visit, and keep what came for the next', async () => {
    const storage = new FakeStorage();
    const { d, fetched } = deps(storage, A, new Map([[SCOPE, 'first shell']]));
    expect(await text(await respond(d, nav(), 'navigation'))).toBe('first shell');
    expect(fetched).toEqual([SCOPE]);
    expect((await storage.open(A)).puts).toEqual([SCOPE]);
    const offline = deps(storage, A, 'offline');
    expect(await text(await respond(offline.d, nav(), 'navigation'))).toBe('first shell');
  });

  it('are a network error when there is neither a shell nor a network', async () => {
    const { d } = deps(new FakeStorage(), A, 'offline');
    expect((await respond(d, nav(), 'navigation')).type).toBe('error');
  });

  it('do not cache a failure', async () => {
    const storage = new FakeStorage();
    const { d } = deps(storage, A, new Map());
    const r = await respond(d, nav(), 'navigation');
    expect(r.status).toBe(404);
    expect((await storage.open(A)).puts).toEqual([]);
  });
});

describe('assets', () => {
  it("come from the build's cache first", async () => {
    const storage = new FakeStorage();
    await install(storage, A, 'aaaa');
    const { d, fetched } = deps(storage, A, 'offline');
    expect(await text(await respond(d, asset('aaaa'), 'asset'))).toBe('app aaaa');
    expect(fetched).toEqual([]);
  });

  it("come from another build's cache before the network (a tab of an older build, a newer one installing)", async () => {
    const storage = new FakeStorage();
    await install(storage, A, 'aaaa');
    await install(storage, B, 'bbbb');
    const { d, fetched } = deps(storage, B, 'offline');
    expect(await text(await respond(d, asset('aaaa'), 'asset'))).toBe('app aaaa');
    expect(fetched).toEqual([]);
  });

  it("ignore caches that are not the game's", async () => {
    const storage = new FakeStorage();
    (await storage.open('someone-elses')).store.set(
      `${SCOPE}assets/app-aaaa.js`,
      new Response('theirs'),
    );
    const { d, fetched } = deps(storage, A, new Map([[`${SCOPE}assets/app-aaaa.js`, 'network']]));
    expect(await text(await respond(d, asset('aaaa'), 'asset'))).toBe('network');
    expect(fetched).toHaveLength(1);
  });

  it('are a network error when nothing has them and there is no network', async () => {
    const { d } = deps(new FakeStorage(), A, 'offline');
    expect((await respond(d, asset('zzzz'), 'asset')).type).toBe('error');
  });
});

describe('an update installing under a running build (F6)', () => {
  it("leaves the running build's shell and assets alone, offline too", async () => {
    const storage = new FakeStorage();
    await install(storage, A, 'aaaa');
    // Build B's worker installs: it fills its own cache and does nothing else.
    await install(storage, B, 'bbbb');
    const running = deps(storage, A, 'offline');
    expect(await text(await respond(running.d, nav(), 'navigation'))).toBe('shell aaaa');
    expect(await text(await respond(running.d, asset('aaaa'), 'asset'))).toBe('app aaaa');
    expect((await storage.open(A)).puts).toEqual([]);
  });

  it('serves the new build after it takes over, and drops the old cache then', async () => {
    const storage = new FakeStorage();
    await install(storage, A, 'aaaa');
    await install(storage, B, 'bbbb');
    expect(await dropOldCaches(storage, B)).toEqual([A]);
    expect([...storage.caches.keys()]).toEqual([B]);
    const taken = deps(storage, B, 'offline');
    expect(await text(await respond(taken.d, nav(), 'navigation'))).toBe('shell bbbb');
  });
});

describe('dropOldCaches', () => {
  it("keeps the build's own cache and anything that is not the game's", async () => {
    const storage = new FakeStorage();
    await storage.open(A);
    await storage.open(B);
    await storage.open('workbox-other');
    expect(await dropOldCaches(storage, B)).toEqual([A]);
    expect([...storage.caches.keys()].sort()).toEqual([B, 'workbox-other'].sort());
  });
});

describe('the names', () => {
  it('put the build in the cache name and the scope in the shell URL', () => {
    expect(A).toBe('fg-aaaa');
    expect(shellUrl('https://host.test/Flying/')).toBe('https://host.test/Flying/');
    expect(shellUrl('https://host.test/Flying/index.html')).toBe('https://host.test/Flying/');
  });
});
