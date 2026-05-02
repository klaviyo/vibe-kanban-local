use api_types::{
    self as wire,
    issue_comment::{CreateIssueCommentRequest, UpdateIssueCommentRequest},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use uuid::Uuid;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct IssueComment {
    pub id: Uuid,
    pub issue_id: Uuid,
    pub author_id: Option<Uuid>,
    pub parent_id: Option<Uuid>,
    pub message: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct CreateIssueComment {
    pub id: Uuid,
    pub author_id: Option<Uuid>,
    pub request: CreateIssueCommentRequest,
}

impl IssueComment {
    pub async fn find_by_id(pool: &SqlitePool, id: Uuid) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as!(
            IssueComment,
            r#"SELECT id         as "id!: Uuid",
                      issue_id   as "issue_id!: Uuid",
                      author_id  as "author_id: Uuid",
                      parent_id  as "parent_id: Uuid",
                      message,
                      created_at as "created_at!: DateTime<Utc>",
                      updated_at as "updated_at!: DateTime<Utc>"
               FROM issue_comments
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
            IssueComment,
            r#"SELECT id         as "id!: Uuid",
                      issue_id   as "issue_id!: Uuid",
                      author_id  as "author_id: Uuid",
                      parent_id  as "parent_id: Uuid",
                      message,
                      created_at as "created_at!: DateTime<Utc>",
                      updated_at as "updated_at!: DateTime<Utc>"
               FROM issue_comments
               WHERE issue_id = $1
               ORDER BY created_at ASC"#,
            issue_id,
        )
        .fetch_all(pool)
        .await
    }

    pub async fn create(pool: &SqlitePool, data: &CreateIssueComment) -> Result<Self, sqlx::Error> {
        sqlx::query_as!(
            IssueComment,
            r#"INSERT INTO issue_comments (id, issue_id, author_id, parent_id, message)
               VALUES ($1, $2, $3, $4, $5)
               RETURNING id         as "id!: Uuid",
                         issue_id   as "issue_id!: Uuid",
                         author_id  as "author_id: Uuid",
                         parent_id  as "parent_id: Uuid",
                         message,
                         created_at as "created_at!: DateTime<Utc>",
                         updated_at as "updated_at!: DateTime<Utc>""#,
            data.id,
            data.request.issue_id,
            data.author_id,
            data.request.parent_id,
            data.request.message,
        )
        .fetch_one(pool)
        .await
    }

    pub async fn update(
        pool: &SqlitePool,
        id: Uuid,
        data: &UpdateIssueCommentRequest,
    ) -> Result<Self, sqlx::Error> {
        let update_message = data.message.is_some();
        let message_value = data.message.clone();
        let update_parent_id = data.parent_id.is_some();
        let parent_id_value = data.parent_id.flatten();

        sqlx::query_as!(
            IssueComment,
            r#"UPDATE issue_comments
               SET message    = CASE WHEN $2 THEN $3 ELSE message END,
                   parent_id  = CASE WHEN $4 THEN $5 ELSE parent_id END,
                   updated_at = datetime('now', 'subsec')
               WHERE id = $1
               RETURNING id         as "id!: Uuid",
                         issue_id   as "issue_id!: Uuid",
                         author_id  as "author_id: Uuid",
                         parent_id  as "parent_id: Uuid",
                         message,
                         created_at as "created_at!: DateTime<Utc>",
                         updated_at as "updated_at!: DateTime<Utc>""#,
            id,
            update_message,
            message_value,
            update_parent_id,
            parent_id_value,
        )
        .fetch_one(pool)
        .await
    }

    pub async fn delete(pool: &SqlitePool, id: Uuid) -> Result<u64, sqlx::Error> {
        let result = sqlx::query!("DELETE FROM issue_comments WHERE id = $1", id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected())
    }
}

impl From<IssueComment> for wire::IssueComment {
    fn from(value: IssueComment) -> Self {
        Self {
            id: value.id,
            issue_id: value.issue_id,
            author_id: value.author_id,
            parent_id: value.parent_id,
            message: value.message,
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}
