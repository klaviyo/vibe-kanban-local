import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { IDBFactory } from 'fake-indexeddb';

// Manually polyfill globalThis.localStorage and globalThis.indexedDB before
// each test so the module-under-test sees fresh state. Ordinary node test
// runs have neither.

interface StorageStub {
  store: Map<string, string>;
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
  removeItem(key: string): void;
  clear(): void;
  readonly length: number;
  key(index: number): string | null;
}

function makeLocalStorage(): StorageStub {
  const store = new Map<string, string>();
  return {
    store,
    getItem: (k) => (store.has(k) ? (store.get(k) as string) : null),
    setItem: (k, v) => {
      store.set(k, String(v));
    },
    removeItem: (k) => {
      store.delete(k);
    },
    clear: () => store.clear(),
    get length() {
      return store.size;
    },
    key: (i) => Array.from(store.keys())[i] ?? null,
  };
}

const SENTINEL_KEY = 'vk-cutover-cleared-v3';

async function seedIndexedDb(name: string): Promise<void> {
  // Open the database so it's registered with the factory; otherwise
  // fake-indexeddb won't return it from databases(). The version write is
  // immaterial — we just need the entry to exist for delete to be observable.
  await new Promise<void>((resolve, reject) => {
    const req = indexedDB.open(name, 1);
    req.onupgradeneeded = () => {
      try {
        req.result.createObjectStore('s');
      } catch {
        // The store may already exist on a re-open; ignore.
      }
    };
    req.onsuccess = () => {
      req.result.close();
      resolve();
    };
    req.onerror = () => reject(req.error);
  });
}

async function listIndexedDbNames(): Promise<string[]> {
  const factory = indexedDB as IDBFactory & {
    databases?: () => Promise<IDBDatabaseInfo[]>;
  };
  if (typeof factory.databases !== 'function') return [];
  const infos = await factory.databases();
  return infos.map((i) => i.name).filter((n): n is string => Boolean(n));
}

let localStorageStub: StorageStub;

beforeEach(async () => {
  // Reset module cache so import { purgePriorElectricCacheOnce } gets a
  // fresh closure each test (no shared internal state, but be defensive).
  vi.resetModules();

  // Fresh in-memory IndexedDB factory for every test.
  globalThis.indexedDB = new IDBFactory() as unknown as IDBFactory;

  // Fresh localStorage stub.
  localStorageStub = makeLocalStorage();
  Object.defineProperty(globalThis, 'localStorage', {
    value: localStorageStub,
    configurable: true,
    writable: true,
  });
});

afterEach(() => {
  delete (globalThis as { indexedDB?: unknown }).indexedDB;
  delete (globalThis as { localStorage?: unknown }).localStorage;
  vi.restoreAllMocks();
});

describe('purgePriorElectricCacheOnce', () => {
  it('first launch: deletes wa-sqlite + electric IndexedDB databases and writes the sentinel', async () => {
    await seedIndexedDb('wa-sqlite-db');
    await seedIndexedDb('wa_sqlite_other');
    await seedIndexedDb('electric-shapes');
    await seedIndexedDb('unrelated-db');
    localStorageStub.setItem('electric_expired_shapes', 'cached');
    localStorageStub.setItem('electric_up_to_date_tracker', 'cached');
    localStorageStub.setItem('user-pref', 'keep-me');

    const { purgePriorElectricCacheOnce } = await import('./cutoverCachePurge');
    await purgePriorElectricCacheOnce();

    const remaining = await listIndexedDbNames();
    expect(remaining.sort()).toEqual(['unrelated-db']);

    expect(localStorageStub.getItem('electric_expired_shapes')).toBeNull();
    expect(localStorageStub.getItem('electric_up_to_date_tracker')).toBeNull();
    expect(localStorageStub.getItem('user-pref')).toBe('keep-me');
    expect(localStorageStub.getItem(SENTINEL_KEY)).not.toBeNull();
  });

  it('subsequent launches: skips the wipe when the sentinel is already present', async () => {
    await seedIndexedDb('wa-sqlite-db');
    await seedIndexedDb('electric-shapes');
    localStorageStub.setItem(SENTINEL_KEY, '2026-05-03T00:00:00.000Z');
    localStorageStub.setItem('electric_expired_shapes', 'still-here');

    const beforeSentinel = localStorageStub.getItem(SENTINEL_KEY);

    const { purgePriorElectricCacheOnce } = await import('./cutoverCachePurge');
    await purgePriorElectricCacheOnce();

    const remaining = await listIndexedDbNames();
    expect(remaining.sort()).toEqual(['electric-shapes', 'wa-sqlite-db']);
    expect(localStorageStub.getItem('electric_expired_shapes')).toBe(
      'still-here'
    );
    expect(localStorageStub.getItem(SENTINEL_KEY)).toBe(beforeSentinel);
  });

  it('recovery: clearing the sentinel re-arms the wipe on the next call', async () => {
    await seedIndexedDb('wa-sqlite-db');
    await seedIndexedDb('electric-shapes');

    const { purgePriorElectricCacheOnce } = await import('./cutoverCachePurge');

    // First call wipes and sets the sentinel.
    await purgePriorElectricCacheOnce();
    expect(await listIndexedDbNames()).toEqual([]);
    const firstSentinel = localStorageStub.getItem(SENTINEL_KEY);
    expect(firstSentinel).not.toBeNull();

    // Reseed the stale databases, simulating a regression / reinstall.
    await seedIndexedDb('wa-sqlite-db');
    await seedIndexedDb('electric-shapes');

    // Sentinel still set: second call must NOT wipe.
    await purgePriorElectricCacheOnce();
    expect((await listIndexedDbNames()).sort()).toEqual([
      'electric-shapes',
      'wa-sqlite-db',
    ]);

    // Clear sentinel and call again: wipe must re-run.
    localStorageStub.removeItem(SENTINEL_KEY);
    await purgePriorElectricCacheOnce();
    expect(await listIndexedDbNames()).toEqual([]);
    const reArmedSentinel = localStorageStub.getItem(SENTINEL_KEY);
    expect(reArmedSentinel).not.toBeNull();
  });

  it('does not throw when localStorage is unavailable', async () => {
    // Simulate environments where localStorage access throws (private mode,
    // strict cookie/storage policies). The hook must swallow the error and
    // resolve without writing anything.
    Object.defineProperty(globalThis, 'localStorage', {
      get() {
        throw new Error('SecurityError: storage unavailable');
      },
      configurable: true,
    });

    const { purgePriorElectricCacheOnce } = await import('./cutoverCachePurge');
    await expect(purgePriorElectricCacheOnce()).resolves.toBeUndefined();
  });

  it('does not throw and does not write the sentinel when indexedDB is undefined', async () => {
    delete (globalThis as { indexedDB?: unknown }).indexedDB;

    const { purgePriorElectricCacheOnce } = await import('./cutoverCachePurge');
    await purgePriorElectricCacheOnce();

    // Without indexedDB the wipe is a no-op success — but the localStorage
    // companion clears still run, and the sentinel is recorded so we don't
    // re-attempt forever.
    expect(localStorageStub.getItem(SENTINEL_KEY)).not.toBeNull();
  });
});
