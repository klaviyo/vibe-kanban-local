use api_types::{self as wire, DeleteResponse, MutationResponse};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use uuid::Uuid;

use super::mutation_log;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub username: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct CreateUser {
    pub id: Uuid,
    pub email: String,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub username: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct UpdateUser {
    pub first_name: Option<Option<String>>,
    pub last_name: Option<Option<String>>,
    pub username: Option<Option<String>>,
}

impl User {
    pub async fn find_by_id(pool: &SqlitePool, id: Uuid) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as!(
            User,
            r#"SELECT id         as "id!: Uuid",
                      email,
                      first_name,
                      last_name,
                      username,
                      created_at as "created_at!: DateTime<Utc>",
                      updated_at as "updated_at!: DateTime<Utc>"
               FROM users
               WHERE id = $1"#,
            id
        )
        .fetch_optional(pool)
        .await
    }

    pub async fn find_by_email(
        pool: &SqlitePool,
        email: &str,
    ) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as!(
            User,
            r#"SELECT id         as "id!: Uuid",
                      email,
                      first_name,
                      last_name,
                      username,
                      created_at as "created_at!: DateTime<Utc>",
                      updated_at as "updated_at!: DateTime<Utc>"
               FROM users
               WHERE email = $1"#,
            email
        )
        .fetch_optional(pool)
        .await
    }

    pub async fn find_all(pool: &SqlitePool) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as!(
            User,
            r#"SELECT id         as "id!: Uuid",
                      email,
                      first_name,
                      last_name,
                      username,
                      created_at as "created_at!: DateTime<Utc>",
                      updated_at as "updated_at!: DateTime<Utc>"
               FROM users
               ORDER BY created_at ASC"#
        )
        .fetch_all(pool)
        .await
    }

    pub async fn create(
        pool: &SqlitePool,
        data: &CreateUser,
    ) -> Result<MutationResponse<Self>, sqlx::Error> {
        let mut tx = pool.begin().await?;
        let row = sqlx::query_as!(
            User,
            r#"INSERT INTO users (id, email, first_name, last_name, username)
               VALUES ($1, $2, $3, $4, $5)
               RETURNING id         as "id!: Uuid",
                         email,
                         first_name,
                         last_name,
                         username,
                         created_at as "created_at!: DateTime<Utc>",
                         updated_at as "updated_at!: DateTime<Utc>""#,
            data.id,
            data.email,
            data.first_name,
            data.last_name,
            data.username,
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
        data: &UpdateUser,
    ) -> Result<MutationResponse<Self>, sqlx::Error> {
        let update_first_name = data.first_name.is_some();
        let first_name_value = data.first_name.clone().flatten();
        let update_last_name = data.last_name.is_some();
        let last_name_value = data.last_name.clone().flatten();
        let update_username = data.username.is_some();
        let username_value = data.username.clone().flatten();

        let mut tx = pool.begin().await?;
        let row = sqlx::query_as!(
            User,
            r#"UPDATE users
               SET first_name = CASE WHEN $2 THEN $3 ELSE first_name END,
                   last_name  = CASE WHEN $4 THEN $5 ELSE last_name  END,
                   username   = CASE WHEN $6 THEN $7 ELSE username   END,
                   updated_at = datetime('now', 'subsec')
               WHERE id = $1
               RETURNING id         as "id!: Uuid",
                         email,
                         first_name,
                         last_name,
                         username,
                         created_at as "created_at!: DateTime<Utc>",
                         updated_at as "updated_at!: DateTime<Utc>""#,
            id,
            update_first_name,
            first_name_value,
            update_last_name,
            last_name_value,
            update_username,
            username_value,
        )
        .fetch_one(&mut *tx)
        .await?;
        let txid = mutation_log::next_txid(&mut *tx).await?;
        tx.commit().await?;
        Ok(MutationResponse { data: row, txid })
    }

    pub async fn delete(pool: &SqlitePool, id: Uuid) -> Result<DeleteResponse, sqlx::Error> {
        let mut tx = pool.begin().await?;
        sqlx::query!("DELETE FROM users WHERE id = $1", id)
            .execute(&mut *tx)
            .await?;
        let txid = mutation_log::next_txid(&mut *tx).await?;
        tx.commit().await?;
        Ok(DeleteResponse { txid })
    }
}

impl From<User> for wire::User {
    fn from(value: User) -> Self {
        Self {
            id: value.id,
            email: value.email,
            first_name: value.first_name,
            last_name: value.last_name,
            username: value.username,
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}
