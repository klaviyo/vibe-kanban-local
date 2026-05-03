-- Junction so a PR (tracked locally for `pr_monitor`) can be linked to an
-- issue from the new issue domain. Mirrors the cloud's `pull_request_issues`
-- table — the route `/api/remote/pull-requests?issue_id=...` joins through
-- this table to return only the PRs linked to a given issue.
CREATE TABLE pull_request_issues (
    id               BLOB PRIMARY KEY,
    pull_request_id  TEXT NOT NULL,
    issue_id         BLOB NOT NULL,
    created_at       TEXT NOT NULL DEFAULT (datetime('now', 'subsec')),
    UNIQUE (pull_request_id, issue_id),
    FOREIGN KEY (pull_request_id) REFERENCES pull_requests(id) ON DELETE CASCADE,
    FOREIGN KEY (issue_id) REFERENCES issues(id) ON DELETE CASCADE
);

CREATE INDEX idx_pull_request_issues_issue_id ON pull_request_issues(issue_id);
