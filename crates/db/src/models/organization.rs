use api_types::{
    self as wire,
    organizations::{CreateOrganizationRequest, UpdateOrganizationRequest},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use uuid::Uuid;

/// Default `issue_prefix` written for newly-created organizations on the local
/// path. Matches the schema-level default in
/// `migrations/20260502120000_create_organizations.sql`, but is written
/// explicitly so that local databases that ran an older revision of that
/// migration (which defaulted to `'ISS'`) still mint new orgs with the
/// contracted prefix.
pub const DEFAULT_ISSUE_PREFIX: &str = "VK";

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
    ) -> Result<Self, sqlx::Error> {
        sqlx::query_as!(
            Organization,
            r#"INSERT INTO organizations (id, name, slug, issue_prefix)
               VALUES ($1, $2, $3, $4)
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
            DEFAULT_ISSUE_PREFIX,
        )
        .fetch_one(pool)
        .await
    }

    pub async fn update(
        pool: &SqlitePool,
        id: Uuid,
        data: &UpdateOrganizationRequest,
    ) -> Result<Self, sqlx::Error> {
        sqlx::query_as!(
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
        .fetch_one(pool)
        .await
    }

    pub async fn delete(pool: &SqlitePool, id: Uuid) -> Result<u64, sqlx::Error> {
        let result = sqlx::query!("DELETE FROM organizations WHERE id = $1", id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected())
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
