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

CREATE INDEX idx_issue_comments_issue_id ON issue_comments(issue_id);
CREATE INDEX idx_issue_comments_parent_id ON issue_comments(parent_id);
