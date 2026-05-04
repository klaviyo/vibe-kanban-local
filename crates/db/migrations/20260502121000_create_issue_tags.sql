CREATE TABLE issue_tags (
    id        BLOB PRIMARY KEY,
    issue_id  BLOB NOT NULL,
    tag_id    BLOB NOT NULL,
    UNIQUE (issue_id, tag_id),
    FOREIGN KEY (issue_id) REFERENCES issues(id) ON DELETE CASCADE,
    FOREIGN KEY (tag_id) REFERENCES project_tags(id) ON DELETE CASCADE
);
