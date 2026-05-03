-- Widen issue_number / issue_counter to BIGINT so they preserve the full
-- end-to-end i64 counter width that the wire/api type now exposes. The
-- INTEGER (int4) column saturates at 2^31-1, which is small enough for
-- a busy organization to plausibly hit and silently truncate at the API
-- boundary — promoting both the column and the counter eliminates that
-- truncation point.

ALTER TABLE issues
    ALTER COLUMN issue_number TYPE BIGINT;

ALTER TABLE organizations
    ALTER COLUMN issue_counter TYPE BIGINT;

-- Recreate the trigger function with BIGINT locals so the counter value
-- it returns and assigns into NEW.issue_number is i64 throughout.
CREATE OR REPLACE FUNCTION set_issue_simple_id()
RETURNS TRIGGER AS $$
DECLARE
    v_issue_number    BIGINT;
    v_issue_prefix    VARCHAR(10);
    v_organization_id UUID;
BEGIN
    SELECT p.organization_id, o.issue_prefix
    INTO v_organization_id, v_issue_prefix
    FROM projects p
    JOIN organizations o ON o.id = p.organization_id
    WHERE p.id = NEW.project_id;

    UPDATE organizations
    SET issue_counter = issue_counter + 1
    WHERE id = v_organization_id
    RETURNING issue_counter INTO v_issue_number;

    NEW.issue_number := v_issue_number;
    NEW.simple_id    := v_issue_prefix || '-' || v_issue_number;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;
