-- Additive ALTER on projects to align with the cloud contract.
-- organization_id is nullable here so existing pre-cutover rows continue to apply;
-- the synthetic-organization seed (later migration) backfills these rows.
ALTER TABLE projects
    ADD COLUMN organization_id BLOB REFERENCES organizations(id) ON DELETE CASCADE;

ALTER TABLE projects
    ADD COLUMN color TEXT NOT NULL DEFAULT '0 0% 0%';

ALTER TABLE projects
    ADD COLUMN sort_order INTEGER NOT NULL DEFAULT 0;

CREATE INDEX idx_projects_organization_sort_order
    ON projects (organization_id, sort_order);
