use api_types::{
    self as wire, DeleteResponse, MutationResponse,
    workspace_issue_link::CreateWorkspaceIssueLinkRequest,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use uuid::Uuid;

use super::mutation_log;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct WorkspaceIssueLink {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub issue_id: Uuid,
    pub project_id: Uuid,
    pub created_at: DateTime<Utc>,
}

impl WorkspaceIssueLink {
    pub async fn find_by_id(pool: &SqlitePool, id: Uuid) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as!(
            WorkspaceIssueLink,
            r#"SELECT id           as "id!: Uuid",
                      workspace_id as "workspace_id!: Uuid",
                      issue_id     as "issue_id!: Uuid",
                      project_id   as "project_id!: Uuid",
                      created_at   as "created_at!: DateTime<Utc>"
               FROM workspace_issue_links
               WHERE id = $1"#,
            id,
        )
        .fetch_optional(pool)
        .await
    }

    pub async fn find_by_issue(
        pool: &SqlitePool,
        issue_id: Uuid,
    ) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as!(
            WorkspaceIssueLink,
            r#"SELECT id           as "id!: Uuid",
                      workspace_id as "workspace_id!: Uuid",
                      issue_id     as "issue_id!: Uuid",
                      project_id   as "project_id!: Uuid",
                      created_at   as "created_at!: DateTime<Utc>"
               FROM workspace_issue_links
               WHERE issue_id = $1
               ORDER BY created_at ASC"#,
            issue_id,
        )
        .fetch_all(pool)
        .await
    }

    pub async fn find_by_workspace(
        pool: &SqlitePool,
        workspace_id: Uuid,
    ) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as!(
            WorkspaceIssueLink,
            r#"SELECT id           as "id!: Uuid",
                      workspace_id as "workspace_id!: Uuid",
                      issue_id     as "issue_id!: Uuid",
                      project_id   as "project_id!: Uuid",
                      created_at   as "created_at!: DateTime<Utc>"
               FROM workspace_issue_links
               WHERE workspace_id = $1
               ORDER BY created_at ASC"#,
            workspace_id,
        )
        .fetch_all(pool)
        .await
    }

    pub async fn create(
        pool: &SqlitePool,
        id: Uuid,
        data: &CreateWorkspaceIssueLinkRequest,
    ) -> Result<MutationResponse<Self>, sqlx::Error> {
        let mut tx = pool.begin().await?;
        let row = sqlx::query_as!(
            WorkspaceIssueLink,
            r#"INSERT INTO workspace_issue_links (id, workspace_id, issue_id, project_id)
               VALUES ($1, $2, $3, $4)
               RETURNING id           as "id!: Uuid",
                         workspace_id as "workspace_id!: Uuid",
                         issue_id     as "issue_id!: Uuid",
                         project_id   as "project_id!: Uuid",
                         created_at   as "created_at!: DateTime<Utc>""#,
            id,
            data.workspace_id,
            data.issue_id,
            data.project_id,
        )
        .fetch_one(&mut *tx)
        .await?;
        let txid = mutation_log::next_txid(&mut *tx).await?;
        tx.commit().await?;
        Ok(MutationResponse { data: row, txid })
    }

    /// Replace any existing rows for `workspace_id` with a single row pointing
    /// at the requested issue. Each workspace has exactly one active linked
    /// issue: the cloud contract treats the relationship as singular and
    /// `get_workspace_by_local_id()` consumes only the first row, so a relink
    /// to a different issue must not leave stale rows behind. The delete, the
    /// insert, and the wire-envelope txid allocation run in one transaction
    /// so callers never observe two active links and the txid is rollback-safe.
    pub async fn replace_for_workspace(
        pool: &SqlitePool,
        workspace_id: Uuid,
        issue_id: Uuid,
        project_id: Uuid,
    ) -> Result<MutationResponse<Self>, sqlx::Error> {
        let mut tx = pool.begin().await?;

        sqlx::query!(
            "DELETE FROM workspace_issue_links WHERE workspace_id = $1",
            workspace_id,
        )
        .execute(&mut *tx)
        .await?;

        let id = Uuid::new_v4();
        let row = sqlx::query_as!(
            WorkspaceIssueLink,
            r#"INSERT INTO workspace_issue_links (id, workspace_id, issue_id, project_id)
               VALUES ($1, $2, $3, $4)
               RETURNING id           as "id!: Uuid",
                         workspace_id as "workspace_id!: Uuid",
                         issue_id     as "issue_id!: Uuid",
                         project_id   as "project_id!: Uuid",
                         created_at   as "created_at!: DateTime<Utc>""#,
            id,
            workspace_id,
            issue_id,
            project_id,
        )
        .fetch_one(&mut *tx)
        .await?;

        let txid = mutation_log::next_txid(&mut *tx).await?;
        tx.commit().await?;
        Ok(MutationResponse { data: row, txid })
    }

    /// Delete every link row for the given workspace. Idempotent: returns the
    /// number of rows actually removed. Internal infrastructure used by
    /// workspace teardown — no wire envelope needed.
    pub async fn delete_by_workspace(
        pool: &SqlitePool,
        workspace_id: Uuid,
    ) -> Result<u64, sqlx::Error> {
        let result = sqlx::query!(
            "DELETE FROM workspace_issue_links WHERE workspace_id = $1",
            workspace_id,
        )
        .execute(pool)
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn update(_: &SqlitePool, _id: Uuid) -> Result<(), sqlx::Error> {
        Ok(())
    }

    pub async fn delete(pool: &SqlitePool, id: Uuid) -> Result<DeleteResponse, sqlx::Error> {
        let mut tx = pool.begin().await?;
        sqlx::query!("DELETE FROM workspace_issue_links WHERE id = $1", id)
            .execute(&mut *tx)
            .await?;
        let txid = mutation_log::next_txid(&mut *tx).await?;
        tx.commit().await?;
        Ok(DeleteResponse { txid })
    }

    pub async fn delete_by_workspace_and_issue(
        pool: &SqlitePool,
        workspace_id: Uuid,
        issue_id: Uuid,
    ) -> Result<DeleteResponse, sqlx::Error> {
        let mut tx = pool.begin().await?;
        sqlx::query!(
            "DELETE FROM workspace_issue_links WHERE workspace_id = $1 AND issue_id = $2",
            workspace_id,
            issue_id,
        )
        .execute(&mut *tx)
        .await?;
        let txid = mutation_log::next_txid(&mut *tx).await?;
        tx.commit().await?;
        Ok(DeleteResponse { txid })
    }
}

impl From<WorkspaceIssueLink> for wire::WorkspaceIssueLink {
    fn from(value: WorkspaceIssueLink) -> Self {
        Self {
            id: value.id,
            workspace_id: value.workspace_id,
            issue_id: value.issue_id,
            project_id: value.project_id,
            created_at: value.created_at,
        }
    }
}
