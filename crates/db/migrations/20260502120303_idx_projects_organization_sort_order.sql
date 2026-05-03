CREATE INDEX idx_projects_organization_sort_order
    ON projects (organization_id, sort_order);
