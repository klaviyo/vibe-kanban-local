-- Additive ALTER on projects to align with the cloud contract.
-- organization_id is nullable here so existing pre-cutover rows continue to apply;
-- the synthetic-organization seed (later migration) backfills these rows.
ALTER TABLE projects
    ADD COLUMN organization_id BLOB REFERENCES organizations(id) ON DELETE CASCADE;
