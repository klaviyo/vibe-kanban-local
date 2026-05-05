CREATE INDEX idx_organization_members_org_role
    ON organization_members (organization_id, role);
