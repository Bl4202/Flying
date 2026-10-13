import { describe, expect, it } from 'vitest';
import {
  memoryKv,
  openKv,
  type IdbDatabaseLike,
  type IdbFactoryLike,
  type IdbOpenRequestLike,
  type IdbRequestLike,
  type IdbStoreLike,
  type IdbTransactionLike,
} from './kv.ts';

/** Just enough IndexedDB: a map behind the event-callback API, completing on a later tick. */
function fakeIdb(opts: { failOpen?: boolean; blocked?: boolean } = {}): IdbFactoryLike {
  const data = new Map<string, unknown>();
  let created = false;
  return {
    open(): IdbOpenRequestLike {
      const request: IdbOpenRequestLike = {
        result: undefined as unknown as IdbDatabaseLike,
        error: opts.failOpen === true ? 'denied' : null,
        onsuccess: null,
        onerror: null,
        onupgradeneeded: null,
        onblocked: null,
      };
      const db: IdbDatabaseLike = {
        createObjectStore: () => {
          created = true;
          return {};
        },
        close: () => undefined,
        transaction(): IdbTransactionLike {
          const tx: IdbTransactionLike = {
            oncomplete: null,
            onerror: null,
            onabort: null,
            error: null,
            objectStore: (): IdbStoreLike => {
              const op = <T>(f: () => T): IdbRequestLike<T> => {
                const r: IdbRequestLike<T> = {
                  result: f(),
                  error: null,
                  onsuccess: null,
                  onerror: null,
                };
                setTimeout(() => tx.oncomplete?.(), 0);
                return r;
              };
              return {
                get: (key) => op(() => structuredClone(data.get(key))),
                put: (value, key) => op(() => void data.set(key, structuredClone(value))),
                delete: (key) => op(() => void data.delete(key)),
              };
            },
          };
          return tx;
        },
      };
      request.result = db;
      setTimeout(() => {
        if (opts.blocked === true) request.onblocked?.();
        else if (opts.failOpen === true) request.onerror?.();
        else {
          request.onupgradeneeded?.();
          request.onsuccess?.();
        }
      }, 0);
      return request;
    },
    get created() {
      return created;
    },
  } as IdbFactoryLike;
}

describe('memoryKv', () => {
  it('stores copies, not references', async () => {
    const kv = memoryKv();
    const value = { a: [1, 2] };
    await kv.set('k', value);
    value.a.push(3);
    expect(await kv.get('k')).toEqual({ a: [1, 2] });
    expect(await kv.get('missing')).toBeUndefined();
    await kv.delete('k');
    expect(await kv.get('k')).toBeUndefined();
    expect(kv.backend).toBe('memory');
  });
});

describe('openKv', () => {
  it('falls back to memory when there is no IndexedDB', async () => {
    expect((await openKv('x', undefined)).backend).toBe('memory');
  });

  it('falls back to memory when opening fails or is blocked', async () => {
    expect((await openKv('x', fakeIdb({ failOpen: true }))).backend).toBe('memory');
    expect((await openKv('x', fakeIdb({ blocked: true }))).backend).toBe('memory');
  });

  it('round-trips through the database', async () => {
    const kv = await openKv('x', fakeIdb());
    expect(kv.backend).toBe('indexeddb');
    await kv.set('compat', { acknowledged: true, signature: 'abc' });
    expect(await kv.get('compat')).toEqual({ acknowledged: true, signature: 'abc' });
    expect(await kv.get('other')).toBeUndefined();
    await kv.delete('compat');
    expect(await kv.get('compat')).toBeUndefined();
  });
});
