use api_types::{
    self as wire, DeleteResponse, MutationResponse,
    issue_comment_reaction::{
        CreateIssueCommentReactionRequest, UpdateIssueCommentReactionRequest,
    },
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use uuid::Uuid;

use super::mutation_log;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct IssueCommentReaction {
    pub id: Uuid,
    pub comment_id: Uuid,
    pub user_id: Uuid,
    pub emoji: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct CreateIssueCommentReaction {
    pub id: Uuid,
    pub user_id: Uuid,
    pub request: CreateIssueCommentReactionRequest,
}

impl IssueCommentReaction {
    pub async fn find_by_id(pool: &SqlitePool, id: Uuid) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as!(
            IssueCommentReaction,
            r#"SELECT id         as "id!: Uuid",
                      comment_id as "comment_id!: Uuid",
                      user_id    as "user_id!: Uuid",
                      emoji,
                      created_at as "created_at!: DateTime<Utc>"
               FROM issue_comment_reactions
               WHERE id = $1"#,
            id,
        )
        .fetch_optional(pool)
        .await
    }

    pub async fn find_by_comment(
        pool: &SqlitePool,
        comment_id: Uuid,
    ) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as!(
            IssueCommentReaction,
            r#"SELECT id         as "id!: Uuid",
                      comment_id as "comment_id!: Uuid",
                      user_id    as "user_id!: Uuid",
                      emoji,
                      created_at as "created_at!: DateTime<Utc>"
               FROM issue_comment_reactions
               WHERE comment_id = $1
               ORDER BY created_at ASC"#,
            comment_id,
        )
        .fetch_all(pool)
        .await
    }

    /// Lists reactions across every comment on the given issue. Used by
    /// the kanban frontend's issue-scoped reaction shape (it pulls
    /// reactions for all comments on an issue at once, rather than
    /// fetching per-comment).
    pub async fn find_by_issue(
        pool: &SqlitePool,
        issue_id: Uuid,
    ) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as!(
            IssueCommentReaction,
            r#"SELECT r.id         as "id!: Uuid",
                      r.comment_id as "comment_id!: Uuid",
                      r.user_id    as "user_id!: Uuid",
                      r.emoji,
                      r.created_at as "created_at!: DateTime<Utc>"
               FROM issue_comment_reactions r
               INNER JOIN issue_comments c ON c.id = r.comment_id
               WHERE c.issue_id = $1
               ORDER BY r.created_at ASC"#,
            issue_id,
        )
        .fetch_all(pool)
        .await
    }

    pub async fn create(
        pool: &SqlitePool,
        data: &CreateIssueCommentReaction,
    ) -> Result<MutationResponse<Self>, sqlx::Error> {
        let mut tx = pool.begin().await?;
        let row = sqlx::query_as!(
            IssueCommentReaction,
            r#"INSERT INTO issue_comment_reactions (id, comment_id, user_id, emoji)
               VALUES ($1, $2, $3, $4)
               RETURNING id         as "id!: Uuid",
                         comment_id as "comment_id!: Uuid",
                         user_id    as "user_id!: Uuid",
                         emoji,
                         created_at as "created_at!: DateTime<Utc>""#,
            data.id,
            data.request.comment_id,
            data.user_id,
            data.request.emoji,
        )
        .fetch_one(&mut *tx)
        .await?;
        let txid = mutation_log::next_txid(&mut *tx).await?;
        tx.commit().await?;
        Ok(MutationResponse { data: row, txid })
    }

    pub async fn update(
        pool: &SqlitePool,
        id: Uuid,
        data: &UpdateIssueCommentReactionRequest,
    ) -> Result<MutationResponse<Self>, sqlx::Error> {
        let update_emoji = data.emoji.is_some();
        let emoji_value = data.emoji.clone();

        let mut tx = pool.begin().await?;
        let row = sqlx::query_as!(
            IssueCommentReaction,
            r#"UPDATE issue_comment_reactions
               SET emoji = CASE WHEN $2 THEN $3 ELSE emoji END
               WHERE id = $1
               RETURNING id         as "id!: Uuid",
                         comment_id as "comment_id!: Uuid",
                         user_id    as "user_id!: Uuid",
                         emoji,
                         created_at as "created_at!: DateTime<Utc>""#,
            id,
            update_emoji,
            emoji_value,
        )
        .fetch_one(&mut *tx)
        .await?;
        let txid = mutation_log::next_txid(&mut *tx).await?;
        tx.commit().await?;
        Ok(MutationResponse { data: row, txid })
    }

    pub async fn delete(pool: &SqlitePool, id: Uuid) -> Result<DeleteResponse, sqlx::Error> {
        let mut tx = pool.begin().await?;
        sqlx::query!("DELETE FROM issue_comment_reactions WHERE id = $1", id)
            .execute(&mut *tx)
            .await?;
        let txid = mutation_log::next_txid(&mut *tx).await?;
        tx.commit().await?;
        Ok(DeleteResponse { txid })
    }
}

impl From<IssueCommentReaction> for wire::IssueCommentReaction {
    fn from(value: IssueCommentReaction) -> Self {
        Self {
            id: value.id,
            comment_id: value.comment_id,
            user_id: value.user_id,
            emoji: value.emoji,
            created_at: value.created_at,
        }
    }
}
