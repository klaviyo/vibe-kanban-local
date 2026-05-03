-- SQLite cannot extend a CHECK-enum in place: widening the `relationship_type`
-- set later requires the table-rebuild-and-copy migration pattern.
CREATE TABLE issue_relationships (
    id                 BLOB PRIMARY KEY,
    issue_id           BLOB NOT NULL,
    related_issue_id   BLOB NOT NULL,
    relationship_type  TEXT NOT NULL
                          CHECK (relationship_type IN ('blocking','related','has_duplicate')),
    created_at         TEXT NOT NULL DEFAULT (datetime('now', 'subsec')),
    UNIQUE (issue_id, related_issue_id, relationship_type),
    CHECK (issue_id != related_issue_id),
    FOREIGN KEY (issue_id) REFERENCES issues(id) ON DELETE CASCADE,
    FOREIGN KEY (related_issue_id) REFERENCES issues(id) ON DELETE CASCADE
);
