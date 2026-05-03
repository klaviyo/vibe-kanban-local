CREATE INDEX idx_issues_creator_user_id
    ON issues(creator_user_id)
    WHERE creator_user_id IS NOT NULL;
