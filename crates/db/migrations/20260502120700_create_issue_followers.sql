CREATE TABLE issue_followers (
    id        BLOB PRIMARY KEY,
    issue_id  BLOB NOT NULL,
    user_id   BLOB NOT NULL,
    UNIQUE (issue_id, user_id),
    FOREIGN KEY (issue_id) REFERENCES issues(id) ON DELETE CASCADE,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);
