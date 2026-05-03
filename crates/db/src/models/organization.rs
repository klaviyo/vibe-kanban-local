use api_types::{
    self as wire, DeleteResponse, MutationResponse,
    organizations::{CreateOrganizationRequest, UpdateOrganizationRequest},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use uuid::Uuid;

use super::mutation_log;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Organization {
    pub id: Uuid,
    pub name: String,
    pub slug: String,
    pub is_personal: bool,
    pub issue_prefix: String,
    pub issue_counter: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Organization {
    pub async fn find_by_id(pool: &SqlitePool, id: Uuid) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as!(
            Organization,
            r#"SELECT id          as "id!: Uuid",
                      name,
                      slug,
                      is_personal as "is_personal!: bool",
                      issue_prefix,
                      issue_counter,
                      created_at  as "created_at!: DateTime<Utc>",
                      updated_at  as "updated_at!: DateTime<Utc>"
               FROM organizations
               WHERE id = $1"#,
            id
        )
        .fetch_optional(pool)
        .await
    }

    pub async fn find_by_slug(pool: &SqlitePool, slug: &str) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as!(
            Organization,
            r#"SELECT id          as "id!: Uuid",
                      name,
                      slug,
                      is_personal as "is_personal!: bool",
                      issue_prefix,
                      issue_counter,
                      created_at  as "created_at!: DateTime<Utc>",
                      updated_at  as "updated_at!: DateTime<Utc>"
               FROM organizations
               WHERE slug = $1"#,
            slug
        )
        .fetch_optional(pool)
        .await
    }

    pub async fn find_all(pool: &SqlitePool) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as!(
            Organization,
            r#"SELECT id          as "id!: Uuid",
                      name,
                      slug,
                      is_personal as "is_personal!: bool",
                      issue_prefix,
                      issue_counter,
                      created_at  as "created_at!: DateTime<Utc>",
                      updated_at  as "updated_at!: DateTime<Utc>"
               FROM organizations
               ORDER BY created_at ASC"#
        )
        .fetch_all(pool)
        .await
    }

    pub async fn create(
        pool: &SqlitePool,
        id: Uuid,
        data: &CreateOrganizationRequest,
    ) -> Result<MutationResponse<Self>, sqlx::Error> {
        let mut tx = pool.begin().await?;
        let row = sqlx::query_as!(
            Organization,
            r#"INSERT INTO organizations (id, name, slug)
               VALUES ($1, $2, $3)
               RETURNING id          as "id!: Uuid",
                         name,
                         slug,
                         is_personal as "is_personal!: bool",
                         issue_prefix,
                         issue_counter,
                         created_at  as "created_at!: DateTime<Utc>",
                         updated_at  as "updated_at!: DateTime<Utc>""#,
            id,
            data.name,
            data.slug,
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
        data: &UpdateOrganizationRequest,
    ) -> Result<MutationResponse<Self>, sqlx::Error> {
        let mut tx = pool.begin().await?;
        let row = sqlx::query_as!(
            Organization,
            r#"UPDATE organizations
               SET name = $2, updated_at = datetime('now', 'subsec')
               WHERE id = $1
               RETURNING id          as "id!: Uuid",
                         name,
                         slug,
                         is_personal as "is_personal!: bool",
                         issue_prefix,
                         issue_counter,
                         created_at  as "created_at!: DateTime<Utc>",
                         updated_at  as "updated_at!: DateTime<Utc>""#,
            id,
            data.name,
        )
        .fetch_one(&mut *tx)
        .await?;
        let txid = mutation_log::next_txid(&mut *tx).await?;
        tx.commit().await?;
        Ok(MutationResponse { data: row, txid })
    }

    pub async fn delete(pool: &SqlitePool, id: Uuid) -> Result<DeleteResponse, sqlx::Error> {
        let mut tx = pool.begin().await?;
        sqlx::query!("DELETE FROM organizations WHERE id = $1", id)
            .execute(&mut *tx)
            .await?;
        let txid = mutation_log::next_txid(&mut *tx).await?;
        tx.commit().await?;
        Ok(DeleteResponse { txid })
    }
}

impl From<Organization> for wire::Organization {
    fn from(value: Organization) -> Self {
        Self {
            id: value.id,
            name: value.name,
            slug: value.slug,
            is_personal: value.is_personal,
            issue_prefix: value.issue_prefix,
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}
