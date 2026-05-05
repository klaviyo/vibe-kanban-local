-- Drop the foreign key on identity_seed_marker.project_id so the originally-
-- seeded project (or any project the marker happens to reference after a
-- re-point) can be deleted via the normal API. Without this fix, the
-- /api/remote/projects/{id} DELETE endpoint returns 500 with a FK-violation
-- error whenever the user tries to remove the seed project.
--
-- The marker's project_id is informational only — runtime code in
-- crates/server/src/runtime/synthetic.rs reads only user_id and
-- organization_id. Keeping the column (without the FK) preserves audit info
-- about which project the seeder wrote on first launch, while letting that
-- project be deleted later.
--
-- SQLite has no ALTER TABLE DROP CONSTRAINT, so use the standard table-rename
-- dance: create a new table with the desired schema, copy rows, drop the old,
-- rename the new.

CREATE TABLE identity_seed_marker_new (
    id              INTEGER PRIMARY KEY CHECK (id = 1),
    organization_id BLOB NOT NULL,
    user_id         BLOB NOT NULL,
    project_id      BLOB NOT NULL,
    host_identity   TEXT NOT NULL,
    host_label      TEXT NOT NULL,
    seeded_at       TEXT NOT NULL DEFAULT (datetime('now', 'subsec')),
    FOREIGN KEY (organization_id) REFERENCES organizations(id),
    FOREIGN KEY (user_id) REFERENCES users(id)
);

INSERT INTO identity_seed_marker_new
    (id, organization_id, user_id, project_id, host_identity, host_label, seeded_at)
SELECT
    id, organization_id, user_id, project_id, host_identity, host_label, seeded_at
FROM identity_seed_marker;

DROP TABLE identity_seed_marker;

ALTER TABLE identity_seed_marker_new RENAME TO identity_seed_marker;
