use api_types::{self as wire, DeleteResponse, MutationResponse, issue_tag::CreateIssueTagRequest};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use uuid::Uuid;

use super::mutation_log;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct IssueTag {
    pub id: Uuid,
    pub issue_id: Uuid,
    pub tag_id: Uuid,
}

impl IssueTag {
    pub async fn find_by_id(pool: &SqlitePool, id: Uuid) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as!(
            IssueTag,
            r#"SELECT id       as "id!: Uuid",
                      issue_id as "issue_id!: Uuid",
                      tag_id   as "tag_id!: Uuid"
               FROM issue_tags
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
            IssueTag,
            r#"SELECT id       as "id!: Uuid",
                      issue_id as "issue_id!: Uuid",
                      tag_id   as "tag_id!: Uuid"
               FROM issue_tags
               WHERE issue_id = $1
               ORDER BY id ASC"#,
            issue_id,
        )
        .fetch_all(pool)
        .await
    }

    /// Lists tag-links across every issue in the given project. Used by
    /// the kanban frontend's project-scoped issue-tag shape (it pulls
    /// links for all visible issues at once, rather than fetching
    /// per-issue). Mirrors `IssueFollower::find_by_project`.
    pub async fn find_by_project(
        pool: &SqlitePool,
        project_id: Uuid,
    ) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as!(
            IssueTag,
            r#"SELECT t.id       as "id!: Uuid",
                      t.issue_id as "issue_id!: Uuid",
                      t.tag_id   as "tag_id!: Uuid"
               FROM issue_tags t
               INNER JOIN issues i ON i.id = t.issue_id
               WHERE i.project_id = $1
               ORDER BY t.id ASC"#,
            project_id,
        )
        .fetch_all(pool)
        .await
    }

    pub async fn create(
        pool: &SqlitePool,
        id: Uuid,
        data: &CreateIssueTagRequest,
    ) -> Result<MutationResponse<Self>, sqlx::Error> {
        let mut tx = pool.begin().await?;
        let row = sqlx::query_as!(
            IssueTag,
            r#"INSERT INTO issue_tags (id, issue_id, tag_id)
               VALUES ($1, $2, $3)
               RETURNING id       as "id!: Uuid",
                         issue_id as "issue_id!: Uuid",
                         tag_id   as "tag_id!: Uuid""#,
            id,
            data.issue_id,
            data.tag_id,
        )
        .fetch_one(&mut *tx)
        .await?;
        let txid = mutation_log::next_txid(&mut *tx).await?;
        tx.commit().await?;
        Ok(MutationResponse { data: row, txid })
    }

    pub async fn delete(pool: &SqlitePool, id: Uuid) -> Result<DeleteResponse, sqlx::Error> {
        let mut tx = pool.begin().await?;
        sqlx::query!("DELETE FROM issue_tags WHERE id = $1", id)
            .execute(&mut *tx)
            .await?;
        let txid = mutation_log::next_txid(&mut *tx).await?;
        tx.commit().await?;
        Ok(DeleteResponse { txid })
    }

    pub async fn delete_by_issue_and_tag(
        pool: &SqlitePool,
        issue_id: Uuid,
        tag_id: Uuid,
    ) -> Result<DeleteResponse, sqlx::Error> {
        let mut tx = pool.begin().await?;
        sqlx::query!(
            "DELETE FROM issue_tags WHERE issue_id = $1 AND tag_id = $2",
            issue_id,
            tag_id,
        )
        .execute(&mut *tx)
        .await?;
        let txid = mutation_log::next_txid(&mut *tx).await?;
        tx.commit().await?;
        Ok(DeleteResponse { txid })
    }
}

impl From<IssueTag> for wire::IssueTag {
    fn from(value: IssueTag) -> Self {
        Self {
            id: value.id,
            issue_id: value.issue_id,
            tag_id: value.tag_id,
        }
    }
}
