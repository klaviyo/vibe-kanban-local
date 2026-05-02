-- Cloud calls this table `tags`; renamed to `project_tags` here because the
-- local schema already has a `tags` table (task-template tags).
CREATE TABLE project_tags (
    id          BLOB PRIMARY KEY,
    project_id  BLOB NOT NULL,
    name        TEXT NOT NULL,
    color       TEXT NOT NULL,
    UNIQUE (project_id, name),
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
);
