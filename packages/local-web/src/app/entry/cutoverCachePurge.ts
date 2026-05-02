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

  // indexedDB.databases() is unsupported on Firefox and older Safari; in that
  // case we no-op rather than guess at names — the sentinel will still mark
  // the install as purged so we don't loop forever.
  const factory = indexedDB as IDBFactory & {
    databases?: () => Promise<IDBDatabaseInfo[]>;
  };
  if (typeof factory.databases !== 'function') return true;

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
