CREATE TABLE issue_comments (
    id          BLOB PRIMARY KEY,
    issue_id    BLOB NOT NULL,
    author_id   BLOB,
    parent_id   BLOB,
    message     TEXT NOT NULL,
    created_at  TEXT NOT NULL DEFAULT (datetime('now', 'subsec')),
    updated_at  TEXT NOT NULL DEFAULT (datetime('now', 'subsec')),
    FOREIGN KEY (issue_id) REFERENCES issues(id) ON DELETE CASCADE,
    FOREIGN KEY (author_id) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (parent_id) REFERENCES issue_comments(id) ON DELETE SET NULL
);
