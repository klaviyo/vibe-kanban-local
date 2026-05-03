-- Singleton marker table that records which organization/user/project rows the
-- synthetic identity seeder wrote on first launch. The CHECK (id = 1) constraint
-- guarantees only one row can ever exist; subsequent seeder runs detect that row
-- and no-op rather than re-deriving identity. `host_identity` stores the OS-level
-- stable host UUID source (machine-id on Linux, gethostuuid() on macOS) used to
-- derive deterministic UUIDs; it is intentionally insensitive to hostname
-- changes. `host_label` stores the kernel hostname captured at seed time and is
-- the rename-sensitive surface compared against on subsequent launches so the
-- seeder can emit a structured warning when a host is renamed while leaving
-- identity rows untouched.
CREATE TABLE identity_seed_marker (
    id              INTEGER PRIMARY KEY CHECK (id = 1),
    organization_id BLOB NOT NULL,
    user_id         BLOB NOT NULL,
    project_id      BLOB NOT NULL,
    host_identity   TEXT NOT NULL,
    host_label      TEXT NOT NULL,
    seeded_at       TEXT NOT NULL DEFAULT (datetime('now', 'subsec')),
    FOREIGN KEY (organization_id) REFERENCES organizations(id),
    FOREIGN KEY (user_id) REFERENCES users(id),
    FOREIGN KEY (project_id) REFERENCES projects(id)
);
