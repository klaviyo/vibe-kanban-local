CREATE TABLE organizations (
    id             BLOB PRIMARY KEY,
    name           TEXT NOT NULL,
    slug           TEXT NOT NULL UNIQUE,
    is_personal    BOOLEAN NOT NULL DEFAULT FALSE,
    issue_prefix   TEXT NOT NULL DEFAULT 'ISS',
    issue_counter  INTEGER NOT NULL DEFAULT 0,
    created_at     TEXT NOT NULL DEFAULT (datetime('now', 'subsec')),
    updated_at     TEXT NOT NULL DEFAULT (datetime('now', 'subsec'))
);
