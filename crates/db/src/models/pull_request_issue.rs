use api_types::{
    MutationResponse, PullRequest as WirePullRequest, PullRequestIssue, PullRequestStatus,
};
use chrono::{DateTime, Utc};
use sqlx::SqlitePool;
use uuid::Uuid;

use super::{merge::MergeStatus, mutation_log};

/// Wire-shape `PullRequest` row joined with the local `pull_request_issues`
/// junction. Used to satisfy `GET /api/remote/pull-requests?issue_id=...`
/// from local SQLite — the cloud query joined the same way.
pub struct PullRequestIssueRepository;

impl PullRequestIssueRepository {
    /// Upserts the junction row and returns the resulting `PullRequestIssue`
    /// in a `MutationResponse`. Idempotent on `(pull_request_id, issue_id)` —
    /// when the link already exists, the existing row's id is returned (so
    /// the supplied `id` is only honored on first insert). Allocates a
    /// mutation-log txid in the same transaction as the insert so the
    /// returned envelope matches the canonical create-mutation contract.
    pub async fn link(
        pool: &SqlitePool,
        id: Uuid,
        pull_request_id: &str,
        issue_id: Uuid,
    ) -> Result<MutationResponse<PullRequestIssue>, sqlx::Error> {
        let mut tx = pool.begin().await?;
        sqlx::query!(
            r#"INSERT INTO pull_request_issues (id, pull_request_id, issue_id)
               VALUES ($1, $2, $3)
               ON CONFLICT(pull_request_id, issue_id) DO NOTHING"#,
            id,
            pull_request_id,
            issue_id,
        )
        .execute(&mut *tx)
        .await?;
        let row = sqlx::query!(
            r#"SELECT id              as "id!: Uuid",
                      pull_request_id as "pull_request_id!: String",
                      issue_id        as "issue_id!: Uuid"
               FROM pull_request_issues
               WHERE pull_request_id = $1 AND issue_id = $2"#,
            pull_request_id,
            issue_id,
        )
        .fetch_one(&mut *tx)
        .await?;
        let txid = mutation_log::next_txid(&mut *tx).await?;
        tx.commit().await?;

        Ok(MutationResponse {
            data: PullRequestIssue {
                id: row.id,
                pull_request_id: Uuid::parse_str(&row.pull_request_id)
                    .unwrap_or_else(|_| Uuid::nil()),
                issue_id: row.issue_id,
            },
            txid,
        })
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

impl PullRequestIssueRepository {
    /// Lists every PR linked to any issue in the given project, in wire
    /// shape. Used by the kanban frontend's project-scoped pull-requests
    /// shape — it pulls PRs for all visible issues at once. Mirrors the
    /// per-issue query but anchors the project filter on the joined issue.
    #[allow(deprecated)]
    pub async fn list_by_project(
        pool: &SqlitePool,
        project_id: Uuid,
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
               WHERE i.project_id = $1
               ORDER BY pr.created_at ASC"#,
            project_id,
        )
        .fetch_all(pool)
        .await?;

        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
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

    /// Lists raw `pull_request_issues` junction rows for every issue in
    /// the given project. Used by the kanban frontend's project-scoped
    /// `pull_request_issues` shape (the join itself, not the joined PR
    /// rows). The junction's `pull_request_id` column is `TEXT` (encoding
    /// a Uuid as its string form, see `PullRequest::create`); we parse
    /// each value into the wire shape's `Uuid`, falling back to `nil`
    /// for any historical row whose id is unparsable rather than failing
    /// the whole list.
    pub async fn list_links_by_project(
        pool: &SqlitePool,
        project_id: Uuid,
    ) -> Result<Vec<PullRequestIssue>, sqlx::Error> {
        let rows = sqlx::query!(
            r#"SELECT pri.id              as "id!: Uuid",
                      pri.pull_request_id as "pull_request_id!: String",
                      pri.issue_id        as "issue_id!: Uuid"
               FROM pull_request_issues pri
               INNER JOIN issues i ON i.id = pri.issue_id
               WHERE i.project_id = $1
               ORDER BY pri.id ASC"#,
            project_id,
        )
        .fetch_all(pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| PullRequestIssue {
                id: row.id,
                pull_request_id: Uuid::parse_str(&row.pull_request_id)
                    .unwrap_or_else(|_| Uuid::nil()),
                issue_id: row.issue_id,
            })
            .collect())
    }
}

fn merge_status_to_wire(status: &MergeStatus) -> PullRequestStatus {
    match status {
        MergeStatus::Open | MergeStatus::Unknown => PullRequestStatus::Open,
        MergeStatus::Merged => PullRequestStatus::Merged,
        MergeStatus::Closed => PullRequestStatus::Closed,
    }
}
