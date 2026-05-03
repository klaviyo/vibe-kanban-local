-- Junction between local workspaces and the new issue domain. Mirrors the
-- cloud's `(local_workspace_id, issue_id, project_id)` triple but is a
-- separate table so the local Workspace lifecycle is uncoupled from the
-- cutover. ON DELETE CASCADE on workspace and issue keeps the join clean.
CREATE TABLE workspace_issue_links (
    id            BLOB PRIMARY KEY,
    workspace_id  BLOB NOT NULL,
    issue_id      BLOB NOT NULL,
    project_id    BLOB NOT NULL,
    created_at    TEXT NOT NULL DEFAULT (datetime('now', 'subsec')),
    UNIQUE (workspace_id, issue_id),
    FOREIGN KEY (workspace_id) REFERENCES workspaces(id) ON DELETE CASCADE,
    FOREIGN KEY (issue_id) REFERENCES issues(id) ON DELETE CASCADE,
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
);
