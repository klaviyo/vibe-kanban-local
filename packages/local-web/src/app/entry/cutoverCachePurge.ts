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

function deleteIndexedDb(name: string): Promise<void> {
  return new Promise((resolve) => {
    let request: IDBOpenDBRequest;
    try {
      request = indexedDB.deleteDatabase(name);
    } catch {
      resolve();
      return;
    }
    request.onsuccess = () => resolve();
    request.onerror = () => resolve();
    request.onblocked = () => resolve();
  });
}

async function deleteElectricIndexedDbs(): Promise<void> {
  if (typeof indexedDB === 'undefined') return;

  // indexedDB.databases() is unsupported on Firefox and older Safari; in that
  // case we no-op rather than guess at names — the sentinel will still mark
  // the install as purged so we don't loop forever.
  const factory = indexedDB as IDBFactory & {
    databases?: () => Promise<IDBDatabaseInfo[]>;
  };
  if (typeof factory.databases !== 'function') return;

  let infos: IDBDatabaseInfo[] = [];
  try {
    infos = await factory.databases();
  } catch {
    return;
  }

  const targets = infos
    .map((info) => info.name)
    .filter((name): name is string => Boolean(name))
    .filter((name) =>
      ELECTRIC_IDB_NAME_PATTERNS.some((pattern) => pattern.test(name))
    );

  await Promise.all(targets.map(deleteIndexedDb));
}

/**
 * Wipe the prior Electric sync layer's browser cache exactly once per upgraded
 * install. Subsequent loads detect the sentinel and skip the wipe; clearing
 * the sentinel and reloading re-runs the wipe as a manual recovery path.
 */
export function purgePriorElectricCacheOnce(): void {
  if (!isStorageAvailable()) return;
  if (readSentinel()) return;

  void (async () => {
    try {
      await deleteElectricIndexedDbs();
    } finally {
      clearElectricLocalStorageKeys();
      writeSentinel();
    }
  })();
}
