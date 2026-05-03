-- Singleton marker table that records which organization/user/project rows the
-- synthetic identity seeder wrote on first launch. The CHECK (id = 1) constraint
-- guarantees only one row can ever exist; subsequent seeder runs detect that row
-- and no-op rather than re-deriving identity. `host_identity` stores the OS-level
-- host UUID source (machine-id on Linux, gethostuuid() on macOS) at seed time so
-- the seeder can detect host renames and emit a structured warning while leaving
-- identity rows untouched.
CREATE TABLE identity_seed_marker (
    id              INTEGER PRIMARY KEY CHECK (id = 1),
    organization_id BLOB NOT NULL,
    user_id         BLOB NOT NULL,
    project_id      BLOB NOT NULL,
    host_identity   TEXT NOT NULL,
    seeded_at       TEXT NOT NULL DEFAULT (datetime('now', 'subsec')),
    FOREIGN KEY (organization_id) REFERENCES organizations(id),
    FOREIGN KEY (user_id) REFERENCES users(id),
    FOREIGN KEY (project_id) REFERENCES projects(id)
);
