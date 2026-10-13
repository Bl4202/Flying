/**
 * Tiny persistent key-value store over IndexedDB, with an in-memory fallback.
 *
 * Used for things the app should remember across visits (the compat acknowledgement, the
 * benchmark result). Values must survive structured clone. If IndexedDB is missing, blocked
 * (private windows, some embedded browsers) or fails to open, the store still works for the life
 * of the page and `backend` says so; it never throws on open.
 */

export interface Kv {
  readonly backend: 'indexeddb' | 'memory';
  get<T>(key: string): Promise<T | undefined>;
  set(key: string, value: unknown): Promise<void>;
  delete(key: string): Promise<void>;
}

/** The subset of `IDBFactory` this module uses. */
export interface IdbFactoryLike {
  open(name: string, version?: number): IdbOpenRequestLike;
}
export interface IdbRequestLike<T> {
  result: T;
  error: unknown;
  onsuccess: (() => void) | null;
  onerror: (() => void) | null;
}
export interface IdbOpenRequestLike extends IdbRequestLike<IdbDatabaseLike> {
  onupgradeneeded: (() => void) | null;
  onblocked: (() => void) | null;
}
export interface IdbDatabaseLike {
  createObjectStore(name: string): unknown;
  transaction(store: string, mode: 'readonly' | 'readwrite'): IdbTransactionLike;
  close(): void;
}
export interface IdbTransactionLike {
  objectStore(name: string): IdbStoreLike;
  oncomplete: (() => void) | null;
  onerror: (() => void) | null;
  onabort: (() => void) | null;
  error: unknown;
}
export interface IdbStoreLike {
  get(key: string): IdbRequestLike<unknown>;
  put(value: unknown, key: string): IdbRequestLike<unknown>;
  delete(key: string): IdbRequestLike<unknown>;
}

const STORE = 'kv';

export function memoryKv(): Kv {
  const map = new Map<string, unknown>();
  return {
    backend: 'memory',
    get: <T>(key: string) => Promise.resolve(map.get(key) as T | undefined),
    set: (key, value) => {
      map.set(key, structuredClone(value));
      return Promise.resolve();
    },
    delete: (key) => {
      map.delete(key);
      return Promise.resolve();
    },
  };
}

/** Opens (creating if needed) the database `name`. */
export async function openKv(
  name: string,
  factory: IdbFactoryLike | undefined = (globalThis as { indexedDB?: IdbFactoryLike }).indexedDB,
): Promise<Kv> {
  if (factory === undefined) {
    return memoryKv();
  }
  let db: IdbDatabaseLike;
  try {
    db = await new Promise<IdbDatabaseLike>((resolve, reject) => {
      const request = factory.open(name, 1);
      request.onupgradeneeded = () => {
        request.result.createObjectStore(STORE);
      };
      request.onsuccess = () => {
        resolve(request.result);
      };
      request.onerror = () => {
        reject(new Error(String(request.error)));
      };
      request.onblocked = () => {
        reject(new Error('blocked'));
      };
    });
  } catch {
    return memoryKv();
  }

  const run = <T>(
    mode: 'readonly' | 'readwrite',
    op: (store: IdbStoreLike) => IdbRequestLike<unknown>,
  ): Promise<T> =>
    new Promise<T>((resolve, reject) => {
      const tx = db.transaction(STORE, mode);
      const request = op(tx.objectStore(STORE));
      tx.oncomplete = () => {
        resolve(request.result as T);
      };
      tx.onerror = tx.onabort = () => {
        reject(new Error(String(tx.error)));
      };
    });

  return {
    backend: 'indexeddb',
    get: <T>(key: string) => run<T | undefined>('readonly', (s) => s.get(key)),
    set: async (key, value) => {
      await run<unknown>('readwrite', (s) => s.put(value, key));
    },
    delete: async (key) => {
      await run<unknown>('readwrite', (s) => s.delete(key));
    },
  };
}
