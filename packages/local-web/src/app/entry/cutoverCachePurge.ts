// Sentinel key written after the one-shot purge runs. Bumping the suffix
// (e.g. -v4) re-arms the purge for every install on the next deploy.
const VK_CUTOVER_SENTINEL_KEY = 'vk-cutover-cleared-v3';

// localStorage keys written by the prior Electric sync layer.
const ELECTRIC_LOCAL_STORAGE_KEYS = [
  'electric_expired_shapes',
  'electric_up_to_date_tracker',
];

// IndexedDB names matching the prior wa-sqlite + Electric stores. Conservative
// enough to leave unrelated databases intact while catching the VFS-created
// stores wa-sqlite and Electric integrations seed by default.
const ELECTRIC_IDB_NAME_PATTERNS: RegExp[] = [/wa[-_]?sqlite/i, /electric/i];

// Speculative list of IndexedDB names used by the prior Electric / wa-sqlite
// integration. We only consult this list on browsers where
// indexedDB.databases() is undefined (Firefox, older Safari) — there we have
// no way to enumerate, so we attempt deletes against each known candidate
// and treat "no such database" as a successful no-op.
//
// These names mirror the patterns in ELECTRIC_IDB_NAME_PATTERNS and the
// concrete defaults used by the IDBBatchAtomicVFS / IDBVersionedVFS examples
// shipped with wa-sqlite plus the @tanstack/electric-db-collection store
// names. They are speculative — if a pre-cutover install is found to use a
// different name, append it here. Adding a never-existed name is harmless
// (deleteDatabase resolves successfully on a missing entry).
const ELECTRIC_IDB_NAME_FALLBACK: string[] = [
  // wa-sqlite VFS defaults seen in src/examples/IDBBatchAtomicVFS.js et al.
  'wa-sqlite',
  'wa-sqlite-vfs',
  'wa-sqlite-files',
  // Common app-named SQLite stores the cutover left behind.
  'vk-sqlite',
  'vk-sqlite-cache',
  'vibe-sqlite',
  // Electric sync layer defaults.
  'electric',
  'electric-sql',
  'electricsql',
  'electric-cache',
  'electric-shapes',
];

function isStorageAvailable(): boolean {
  try {
    return typeof localStorage !== 'undefined';
  } catch {
    return false;
  }
}

function readSentinel(): string | null {
  try {
    return localStorage.getItem(VK_CUTOVER_SENTINEL_KEY);
  } catch {
    return null;
  }
}

function writeSentinel(): void {
  try {
    localStorage.setItem(VK_CUTOVER_SENTINEL_KEY, new Date().toISOString());
  } catch {
    // localStorage may be unavailable (private mode, quota, security errors).
  }
}

function clearElectricLocalStorageKeys(): void {
  for (const key of ELECTRIC_LOCAL_STORAGE_KEYS) {
    try {
      localStorage.removeItem(key);
    } catch {
      // Tolerate per-key failures so one bad key doesn't abort the rest.
    }
  }
}

function deleteIndexedDb(name: string): Promise<boolean> {
  return new Promise((resolve) => {
    let request: IDBOpenDBRequest;
    try {
      request = indexedDB.deleteDatabase(name);
    } catch {
      resolve(false);
      return;
    }
    request.onsuccess = () => resolve(true);
    // `blocked` and `error` mean the delete did not complete, so the stale
    // wa-sqlite data is still on disk. Treat both as failures so the caller
    // leaves the sentinel unset and the next launch retries the purge.
    request.onerror = () => resolve(false);
    request.onblocked = () => resolve(false);
  });
}

async function deleteElectricIndexedDbs(): Promise<boolean> {
  if (typeof indexedDB === 'undefined') return true;

  // indexedDB.databases() is unsupported on Firefox and older Safari. When
  // missing we cannot enumerate, so fall through to the explicit fallback
  // list — guessing a name and asking deleteDatabase to drop it is safe
  // (a no-op when the name doesn't exist) and is the only way to honor the
  // "stale rows from prior Electric era wiped" guarantee on those engines.
  const factory = indexedDB as IDBFactory & {
    databases?: () => Promise<IDBDatabaseInfo[]>;
  };
  if (typeof factory.databases !== 'function') {
    const results = await Promise.all(
      ELECTRIC_IDB_NAME_FALLBACK.map(deleteIndexedDb)
    );
    return results.every((ok) => ok);
  }

  let infos: IDBDatabaseInfo[] = [];
  try {
    infos = await factory.databases();
  } catch {
    return false;
  }

  const targets = infos
    .map((info) => info.name)
    .filter((name): name is string => Boolean(name))
    .filter((name) =>
      ELECTRIC_IDB_NAME_PATTERNS.some((pattern) => pattern.test(name))
    );

  const results = await Promise.all(targets.map(deleteIndexedDb));
  return results.every((ok) => ok);
}

/**
 * Wipe the prior Electric sync layer's browser cache exactly once per upgraded
 * install. Subsequent loads detect the sentinel and skip the wipe; clearing
 * the sentinel and reloading re-runs the wipe as a manual recovery path.
 *
 * Awaitable so callers can gate app mount on purge completion — otherwise
 * router/query initialization may read the stale IndexedDB contents before
 * the delete finishes.
 */
export async function purgePriorElectricCacheOnce(): Promise<void> {
  if (!isStorageAvailable()) return;
  if (readSentinel()) return;

  let purged = false;
  try {
    purged = await deleteElectricIndexedDbs();
  } finally {
    clearElectricLocalStorageKeys();
    // Only mark the install as purged if the IndexedDB wipe actually
    // succeeded; otherwise the next launch retries instead of permanently
    // skipping the wipe over stale data.
    if (purged) {
      writeSentinel();
    }
  }
}
