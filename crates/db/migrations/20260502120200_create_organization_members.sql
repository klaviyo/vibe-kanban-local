-- SQLite cannot extend a CHECK-enum in place: widening the `role` set later
-- requires the table-rebuild-and-copy migration pattern (see PRAGMA legacy_alter_table).
CREATE TABLE organization_members (
    organization_id BLOB NOT NULL,
    user_id         BLOB NOT NULL,
    role            TEXT NOT NULL DEFAULT 'member'
                       CHECK (role IN ('admin','member')),
    joined_at       TEXT NOT NULL DEFAULT (datetime('now', 'subsec')),
    last_seen_at    TEXT,
    PRIMARY KEY (organization_id, user_id),
    FOREIGN KEY (organization_id) REFERENCES organizations(id) ON DELETE CASCADE,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);
