use api_types::{
    self as wire, DeleteResponse, MutationResponse, issue_assignee::CreateIssueAssigneeRequest,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use uuid::Uuid;

use super::mutation_log;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct IssueAssignee {
    pub id: Uuid,
    pub issue_id: Uuid,
    pub user_id: Uuid,
    pub assigned_at: DateTime<Utc>,
}

impl IssueAssignee {
    pub async fn find_by_id(pool: &SqlitePool, id: Uuid) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as!(
            IssueAssignee,
            r#"SELECT id          as "id!: Uuid",
                      issue_id    as "issue_id!: Uuid",
                      user_id     as "user_id!: Uuid",
                      assigned_at as "assigned_at!: DateTime<Utc>"
               FROM issue_assignees
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
            IssueAssignee,
            r#"SELECT id          as "id!: Uuid",
                      issue_id    as "issue_id!: Uuid",
                      user_id     as "user_id!: Uuid",
                      assigned_at as "assigned_at!: DateTime<Utc>"
               FROM issue_assignees
               WHERE issue_id = $1
               ORDER BY assigned_at ASC"#,
            issue_id,
        )
        .fetch_all(pool)
        .await
    }

    /// Lists assignees across every issue in the given project. Used by
    /// the kanban frontend's project-scoped assignee shape (it pulls
    /// assignees for all visible issues at once, rather than fetching
    /// per-issue). Mirrors `IssueFollower::find_by_project`.
    pub async fn find_by_project(
        pool: &SqlitePool,
        project_id: Uuid,
    ) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as!(
            IssueAssignee,
            r#"SELECT a.id          as "id!: Uuid",
                      a.issue_id    as "issue_id!: Uuid",
                      a.user_id     as "user_id!: Uuid",
                      a.assigned_at as "assigned_at!: DateTime<Utc>"
               FROM issue_assignees a
               INNER JOIN issues i ON i.id = a.issue_id
               WHERE i.project_id = $1
               ORDER BY a.assigned_at ASC"#,
            project_id,
        )
        .fetch_all(pool)
        .await
    }

    pub async fn create(
        pool: &SqlitePool,
        id: Uuid,
        data: &CreateIssueAssigneeRequest,
    ) -> Result<MutationResponse<Self>, sqlx::Error> {
        let mut tx = pool.begin().await?;
        let row = sqlx::query_as!(
            IssueAssignee,
            r#"INSERT INTO issue_assignees (id, issue_id, user_id)
               VALUES ($1, $2, $3)
               RETURNING id          as "id!: Uuid",
                         issue_id    as "issue_id!: Uuid",
                         user_id     as "user_id!: Uuid",
                         assigned_at as "assigned_at!: DateTime<Utc>""#,
            id,
            data.issue_id,
            data.user_id,
        )
        .fetch_one(&mut *tx)
        .await?;
        let txid = mutation_log::next_txid(&mut *tx).await?;
        tx.commit().await?;
        Ok(MutationResponse { data: row, txid })
    }

    pub async fn delete(pool: &SqlitePool, id: Uuid) -> Result<DeleteResponse, sqlx::Error> {
        let mut tx = pool.begin().await?;
        sqlx::query!("DELETE FROM issue_assignees WHERE id = $1", id)
            .execute(&mut *tx)
            .await?;
        let txid = mutation_log::next_txid(&mut *tx).await?;
        tx.commit().await?;
        Ok(DeleteResponse { txid })
    }

    pub async fn delete_by_issue_and_user(
        pool: &SqlitePool,
        issue_id: Uuid,
        user_id: Uuid,
    ) -> Result<DeleteResponse, sqlx::Error> {
        let mut tx = pool.begin().await?;
        sqlx::query!(
            "DELETE FROM issue_assignees WHERE issue_id = $1 AND user_id = $2",
            issue_id,
            user_id,
        )
        .execute(&mut *tx)
        .await?;
        let txid = mutation_log::next_txid(&mut *tx).await?;
        tx.commit().await?;
        Ok(DeleteResponse { txid })
    }
}

impl From<IssueAssignee> for wire::IssueAssignee {
    fn from(value: IssueAssignee) -> Self {
        Self {
            id: value.id,
            issue_id: value.issue_id,
            user_id: value.user_id,
            assigned_at: value.assigned_at,
        }
    }
}
