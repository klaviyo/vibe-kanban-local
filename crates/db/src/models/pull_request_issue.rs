use api_types::{PullRequest as WirePullRequest, PullRequestStatus};
use chrono::{DateTime, Utc};
use sqlx::SqlitePool;
use uuid::Uuid;

use super::merge::MergeStatus;

/// Wire-shape `PullRequest` row joined with the local `pull_request_issues`
/// junction. Used to satisfy `GET /api/remote/pull-requests?issue_id=...`
/// from local SQLite — the cloud query joined the same way.
pub struct PullRequestIssueRepository;

impl PullRequestIssueRepository {
    /// Inserts the junction row. Idempotent on `(pull_request_id, issue_id)`.
    pub async fn link(
        pool: &SqlitePool,
        pull_request_id: &str,
        issue_id: Uuid,
    ) -> Result<(), sqlx::Error> {
        let id = Uuid::new_v4();
        sqlx::query!(
            r#"INSERT INTO pull_request_issues (id, pull_request_id, issue_id)
               VALUES ($1, $2, $3)
               ON CONFLICT(pull_request_id, issue_id) DO NOTHING"#,
            id,
            pull_request_id,
            issue_id,
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Returns every PR linked to the given issue, in wire shape. The
    /// `project_id` and (deprecated) `issue_id` columns are derived from the
    /// linked issue, mirroring the cloud's join semantics.
    #[allow(deprecated)]
    pub async fn list_by_issue(
        pool: &SqlitePool,
        issue_id: Uuid,
    ) -> Result<Vec<WirePullRequest>, sqlx::Error> {
        let rows = sqlx::query!(
            r#"SELECT
                pr.id                  as "id!: String",
                pr.workspace_id        as "workspace_id: Uuid",
                pr.pr_url              as "pr_url!: String",
                pr.pr_number           as "pr_number!: i64",
                pr.pr_status           as "pr_status!: MergeStatus",
                pr.target_branch_name  as "target_branch_name!: String",
                pr.merged_at           as "merged_at: DateTime<Utc>",
                pr.merge_commit_sha    as "merge_commit_sha: String",
                pr.created_at          as "created_at!: DateTime<Utc>",
                pr.updated_at          as "updated_at!: DateTime<Utc>",
                i.id                   as "issue_id!: Uuid",
                i.project_id           as "project_id!: Uuid"
               FROM pull_requests pr
               INNER JOIN pull_request_issues pri ON pr.id = pri.pull_request_id
               INNER JOIN issues i ON pri.issue_id = i.id
               WHERE pri.issue_id = $1
               ORDER BY pr.created_at ASC"#,
            issue_id,
        )
        .fetch_all(pool)
        .await?;

        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            // Local PR ids are TEXT-encoded UUIDs (see PullRequest::create);
            // surface them as Uuid in the wire shape, falling back to nil for
            // any historical row we cannot parse rather than failing the
            // whole list.
            let id = Uuid::parse_str(&row.id).unwrap_or_else(|_| Uuid::nil());
            let status = merge_status_to_wire(&row.pr_status);
            let pr_number_i32 = i32::try_from(row.pr_number).unwrap_or(i32::MAX);

            out.push(WirePullRequest {
                id,
                url: row.pr_url,
                number: pr_number_i32,
                status,
                merged_at: row.merged_at,
                merge_commit_sha: row.merge_commit_sha,
                target_branch_name: row.target_branch_name,
                project_id: row.project_id,
                issue_id: row.issue_id,
                workspace_id: row.workspace_id,
                created_at: row.created_at,
                updated_at: row.updated_at,
            });
        }
        Ok(out)
    }
}

fn merge_status_to_wire(status: &MergeStatus) -> PullRequestStatus {
    match status {
        MergeStatus::Open | MergeStatus::Unknown => PullRequestStatus::Open,
        MergeStatus::Merged => PullRequestStatus::Merged,
        MergeStatus::Closed => PullRequestStatus::Closed,
    }
}
