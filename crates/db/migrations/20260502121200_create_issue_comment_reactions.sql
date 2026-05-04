CREATE TABLE issue_comment_reactions (
    id          BLOB PRIMARY KEY,
    comment_id  BLOB NOT NULL,
    user_id     BLOB NOT NULL,
    emoji       TEXT NOT NULL,
    created_at  TEXT NOT NULL DEFAULT (datetime('now', 'subsec')),
    UNIQUE (comment_id, user_id, emoji),
    FOREIGN KEY (comment_id) REFERENCES issue_comments(id) ON DELETE CASCADE,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);
