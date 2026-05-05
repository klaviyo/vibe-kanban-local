CREATE TABLE project_statuses (
    id          BLOB PRIMARY KEY,
    project_id  BLOB NOT NULL,
    name        TEXT NOT NULL,
    color       TEXT NOT NULL,
    sort_order  INTEGER NOT NULL DEFAULT 0,
    hidden      BOOLEAN NOT NULL DEFAULT FALSE,
    created_at  TEXT NOT NULL DEFAULT (datetime('now', 'subsec')),
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
);
