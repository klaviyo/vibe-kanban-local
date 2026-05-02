-- UNIQUE backstop: matches the cloud's per-org uniqueness semantics for simple_id.
-- The cloud enforces this via a BEFORE INSERT trigger with row-level locking on
-- `organizations.issue_counter`; SQLite triggers cannot modify NEW, so we use a
-- unique index as the schema-level guarantee instead. App-level assignment is
-- still responsible for incrementing `organizations.issue_counter` and computing
-- `simple_id = issue_prefix || '-' || issue_number`.
CREATE UNIQUE INDEX idx_issues_simple_id ON issues(simple_id);
