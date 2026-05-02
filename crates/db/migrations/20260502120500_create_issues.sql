-- Issue-number / simple_id assignment is owned by application logic on this side;
-- the cloud's per-org atomic counter trigger has no SQLite equivalent, and the
-- schema-level uniqueness backstop on simple_id lands in a follow-up migration.
-- SQLite cannot extend a CHECK-enum in place: widening the `priority` set later
-- requires the table-rebuild-and-copy migration pattern.
CREATE TABLE issues (
    id                       BLOB PRIMARY KEY,
    project_id               BLOB NOT NULL,
    issue_number             INTEGER NOT NULL,
    simple_id                TEXT NOT NULL,
    status_id                BLOB NOT NULL,
    title                    TEXT NOT NULL,
    description              TEXT,
    priority                 TEXT
                                 CHECK (priority IN ('urgent','high','medium','low')),
    start_date               TEXT,
    target_date              TEXT,
    completed_at             TEXT,
    sort_order               REAL NOT NULL DEFAULT 0,
    parent_issue_id          BLOB,
    parent_issue_sort_order  REAL,
    creator_user_id          BLOB,
    extension_metadata       TEXT NOT NULL DEFAULT '{}',
    created_at               TEXT NOT NULL DEFAULT (datetime('now', 'subsec')),
    updated_at               TEXT NOT NULL DEFAULT (datetime('now', 'subsec')),
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE,
    FOREIGN KEY (status_id) REFERENCES project_statuses(id),
    FOREIGN KEY (parent_issue_id) REFERENCES issues(id) ON DELETE SET NULL,
    FOREIGN KEY (creator_user_id) REFERENCES users(id) ON DELETE SET NULL
);

CREATE INDEX idx_issues_project_id ON issues(project_id);
CREATE INDEX idx_issues_status_id ON issues(status_id);
CREATE INDEX idx_issues_parent_issue_id ON issues(parent_issue_id);
CREATE INDEX idx_issues_simple_id ON issues(simple_id);
CREATE INDEX idx_issues_creator_user_id
    ON issues(creator_user_id)
    WHERE creator_user_id IS NOT NULL;
