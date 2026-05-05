use api_types::{self as wire, workspace_issue_link::CreateWorkspaceIssueLinkRequest};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use uuid::Uuid;

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
    ) -> Result<Self, sqlx::Error> {
        sqlx::query_as!(
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
        .fetch_one(pool)
        .await
    }

    pub async fn delete(pool: &SqlitePool, id: Uuid) -> Result<u64, sqlx::Error> {
        let result = sqlx::query!("DELETE FROM workspace_issue_links WHERE id = $1", id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected())
    }

    pub async fn delete_by_workspace_and_issue(
        pool: &SqlitePool,
        workspace_id: Uuid,
        issue_id: Uuid,
    ) -> Result<u64, sqlx::Error> {
        let result = sqlx::query!(
            "DELETE FROM workspace_issue_links WHERE workspace_id = $1 AND issue_id = $2",
            workspace_id,
            issue_id,
        )
        .execute(pool)
        .await?;
        Ok(result.rows_affected())
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
