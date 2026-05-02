use api_types::{self as wire, issue_follower::CreateIssueFollowerRequest};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use uuid::Uuid;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct IssueFollower {
    pub id: Uuid,
    pub issue_id: Uuid,
    pub user_id: Uuid,
}

impl IssueFollower {
    pub async fn find_by_id(pool: &SqlitePool, id: Uuid) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as!(
            IssueFollower,
            r#"SELECT id       as "id!: Uuid",
                      issue_id as "issue_id!: Uuid",
                      user_id  as "user_id!: Uuid"
               FROM issue_followers
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
            IssueFollower,
            r#"SELECT id       as "id!: Uuid",
                      issue_id as "issue_id!: Uuid",
                      user_id  as "user_id!: Uuid"
               FROM issue_followers
               WHERE issue_id = $1"#,
            issue_id,
        )
        .fetch_all(pool)
        .await
    }

    pub async fn create(
        pool: &SqlitePool,
        id: Uuid,
        data: &CreateIssueFollowerRequest,
    ) -> Result<Self, sqlx::Error> {
        sqlx::query_as!(
            IssueFollower,
            r#"INSERT INTO issue_followers (id, issue_id, user_id)
               VALUES ($1, $2, $3)
               RETURNING id       as "id!: Uuid",
                         issue_id as "issue_id!: Uuid",
                         user_id  as "user_id!: Uuid""#,
            id,
            data.issue_id,
            data.user_id,
        )
        .fetch_one(pool)
        .await
    }

    pub async fn update(_: &SqlitePool, _id: Uuid) -> Result<(), sqlx::Error> {
        Ok(())
    }

    pub async fn delete(pool: &SqlitePool, id: Uuid) -> Result<u64, sqlx::Error> {
        let result = sqlx::query!("DELETE FROM issue_followers WHERE id = $1", id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected())
    }

    pub async fn delete_by_issue_and_user(
        pool: &SqlitePool,
        issue_id: Uuid,
        user_id: Uuid,
    ) -> Result<u64, sqlx::Error> {
        let result = sqlx::query!(
            "DELETE FROM issue_followers WHERE issue_id = $1 AND user_id = $2",
            issue_id,
            user_id,
        )
        .execute(pool)
        .await?;
        Ok(result.rows_affected())
    }
}

impl From<IssueFollower> for wire::IssueFollower {
    fn from(value: IssueFollower) -> Self {
        Self {
            id: value.id,
            issue_id: value.issue_id,
            user_id: value.user_id,
        }
    }
}
