use api_types as wire;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool, Type};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Type, Serialize, Deserialize)]
#[sqlx(type_name = "member_role", rename_all = "lowercase")]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MemberRole {
    Admin,
    Member,
}

impl From<MemberRole> for wire::MemberRole {
    fn from(value: MemberRole) -> Self {
        match value {
            MemberRole::Admin => wire::MemberRole::Admin,
            MemberRole::Member => wire::MemberRole::Member,
        }
    }
}

impl From<wire::MemberRole> for MemberRole {
    fn from(value: wire::MemberRole) -> Self {
        match value {
            wire::MemberRole::Admin => MemberRole::Admin,
            wire::MemberRole::Member => MemberRole::Member,
        }
    }
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct OrganizationMember {
    pub organization_id: Uuid,
    pub user_id: Uuid,
    pub role: MemberRole,
    pub joined_at: DateTime<Utc>,
    pub last_seen_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct CreateOrganizationMember {
    pub organization_id: Uuid,
    pub user_id: Uuid,
    pub role: MemberRole,
}

impl OrganizationMember {
    pub async fn find(
        pool: &SqlitePool,
        organization_id: Uuid,
        user_id: Uuid,
    ) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as!(
            OrganizationMember,
            r#"SELECT organization_id as "organization_id!: Uuid",
                      user_id         as "user_id!: Uuid",
                      role            as "role!: MemberRole",
                      joined_at       as "joined_at!: DateTime<Utc>",
                      last_seen_at    as "last_seen_at: DateTime<Utc>"
               FROM organization_members
               WHERE organization_id = $1 AND user_id = $2"#,
            organization_id,
            user_id,
        )
        .fetch_optional(pool)
        .await
    }

    pub async fn find_by_organization(
        pool: &SqlitePool,
        organization_id: Uuid,
    ) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as!(
            OrganizationMember,
            r#"SELECT organization_id as "organization_id!: Uuid",
                      user_id         as "user_id!: Uuid",
                      role            as "role!: MemberRole",
                      joined_at       as "joined_at!: DateTime<Utc>",
                      last_seen_at    as "last_seen_at: DateTime<Utc>"
               FROM organization_members
               WHERE organization_id = $1
               ORDER BY joined_at ASC"#,
            organization_id,
        )
        .fetch_all(pool)
        .await
    }

    pub async fn find_by_user(pool: &SqlitePool, user_id: Uuid) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as!(
            OrganizationMember,
            r#"SELECT organization_id as "organization_id!: Uuid",
                      user_id         as "user_id!: Uuid",
                      role            as "role!: MemberRole",
                      joined_at       as "joined_at!: DateTime<Utc>",
                      last_seen_at    as "last_seen_at: DateTime<Utc>"
               FROM organization_members
               WHERE user_id = $1
               ORDER BY joined_at ASC"#,
            user_id,
        )
        .fetch_all(pool)
        .await
    }

    pub async fn create(
        pool: &SqlitePool,
        data: &CreateOrganizationMember,
    ) -> Result<Self, sqlx::Error> {
        sqlx::query_as!(
            OrganizationMember,
            r#"INSERT INTO organization_members (organization_id, user_id, role)
               VALUES ($1, $2, $3)
               RETURNING organization_id as "organization_id!: Uuid",
                         user_id         as "user_id!: Uuid",
                         role            as "role!: MemberRole",
                         joined_at       as "joined_at!: DateTime<Utc>",
                         last_seen_at    as "last_seen_at: DateTime<Utc>""#,
            data.organization_id,
            data.user_id,
            data.role,
        )
        .fetch_one(pool)
        .await
    }

    pub async fn update_role(
        pool: &SqlitePool,
        organization_id: Uuid,
        user_id: Uuid,
        role: MemberRole,
    ) -> Result<Self, sqlx::Error> {
        sqlx::query_as!(
            OrganizationMember,
            r#"UPDATE organization_members
               SET role = $3
               WHERE organization_id = $1 AND user_id = $2
               RETURNING organization_id as "organization_id!: Uuid",
                         user_id         as "user_id!: Uuid",
                         role            as "role!: MemberRole",
                         joined_at       as "joined_at!: DateTime<Utc>",
                         last_seen_at    as "last_seen_at: DateTime<Utc>""#,
            organization_id,
            user_id,
            role,
        )
        .fetch_one(pool)
        .await
    }

    pub async fn delete(
        pool: &SqlitePool,
        organization_id: Uuid,
        user_id: Uuid,
    ) -> Result<u64, sqlx::Error> {
        let result = sqlx::query!(
            "DELETE FROM organization_members WHERE organization_id = $1 AND user_id = $2",
            organization_id,
            user_id,
        )
        .execute(pool)
        .await?;
        Ok(result.rows_affected())
    }
}

impl From<OrganizationMember> for wire::OrganizationMember {
    fn from(value: OrganizationMember) -> Self {
        Self {
            organization_id: value.organization_id,
            user_id: value.user_id,
            role: value.role.into(),
            joined_at: value.joined_at,
            last_seen_at: value.last_seen_at,
        }
    }
}
