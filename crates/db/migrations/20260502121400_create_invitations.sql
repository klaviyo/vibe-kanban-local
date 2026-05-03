-- Local mirror of the cloud's `organization_invitations` table. Existing
-- frontend / MCP callers still hit `POST /organizations/{org_id}/invitations`,
-- `GET /organizations/{org_id}/invitations`, `POST .../invitations/revoke`,
-- `GET /invitations/{token}`, and `POST /invitations/{token}/accept`; the
-- cutover keeps those URLs functional by storing pending invitations locally
-- instead of hard-failing the routes.
CREATE TABLE organization_invitations (
    id                  BLOB PRIMARY KEY,
    organization_id     BLOB NOT NULL,
    invited_by_user_id  BLOB,
    email               TEXT NOT NULL,
    role                TEXT NOT NULL DEFAULT 'member'
                            CHECK (role IN ('admin','member')),
    status              TEXT NOT NULL DEFAULT 'pending'
                            CHECK (status IN ('pending','accepted','declined','expired')),
    token               TEXT NOT NULL UNIQUE,
    expires_at          TEXT NOT NULL,
    created_at          TEXT NOT NULL DEFAULT (datetime('now', 'subsec')),
    updated_at          TEXT NOT NULL DEFAULT (datetime('now', 'subsec')),
    FOREIGN KEY (organization_id) REFERENCES organizations(id) ON DELETE CASCADE,
    FOREIGN KEY (invited_by_user_id) REFERENCES users(id) ON DELETE SET NULL
);

CREATE INDEX idx_org_invites_org ON organization_invitations(organization_id);
CREATE INDEX idx_org_invites_status_expires
    ON organization_invitations(status, expires_at);
