use api_types::{self as wire, DeleteResponse, MutationResponse};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool, Type};
use uuid::Uuid;

use super::mutation_log;

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
    ) -> Result<MutationResponse<Self>, sqlx::Error> {
        let mut tx = pool.begin().await?;
        let row = sqlx::query_as!(
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
        .fetch_one(&mut *tx)
        .await?;
        let txid = mutation_log::next_txid(&mut *tx).await?;
        tx.commit().await?;
        Ok(MutationResponse { data: row, txid })
    }

    pub async fn update_role(
        pool: &SqlitePool,
        organization_id: Uuid,
        user_id: Uuid,
        role: MemberRole,
    ) -> Result<MutationResponse<Self>, sqlx::Error> {
        let mut tx = pool.begin().await?;
        let row = sqlx::query_as!(
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
        .fetch_one(&mut *tx)
        .await?;
        let txid = mutation_log::next_txid(&mut *tx).await?;
        tx.commit().await?;
        Ok(MutationResponse { data: row, txid })
    }

    pub async fn delete(
        pool: &SqlitePool,
        organization_id: Uuid,
        user_id: Uuid,
    ) -> Result<DeleteResponse, sqlx::Error> {
        let mut tx = pool.begin().await?;
        sqlx::query!(
            "DELETE FROM organization_members WHERE organization_id = $1 AND user_id = $2",
            organization_id,
            user_id,
        )
        .execute(&mut *tx)
        .await?;
        let txid = mutation_log::next_txid(&mut *tx).await?;
        tx.commit().await?;
        Ok(DeleteResponse { txid })
    }

    /// Mirror of the cloud member-removal contract: reject self-removal,
    /// reject mutations on personal organizations, and reject removing the
    /// last admin — all inside a single transaction so the org cannot be
    /// observed without an admin.
    pub async fn remove_with_guardrails(
        pool: &SqlitePool,
        organization_id: Uuid,
        target_user_id: Uuid,
        acting_user_id: Uuid,
    ) -> Result<(), RemoveMemberError> {
        if acting_user_id == target_user_id {
            return Err(RemoveMemberError::CannotRemoveSelf);
        }

        let mut tx = pool.begin().await?;

        let org = sqlx::query!(
            r#"SELECT is_personal as "is_personal!: bool"
               FROM organizations
               WHERE id = $1"#,
            organization_id,
        )
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(RemoveMemberError::OrganizationNotFound)?;

        if org.is_personal {
            return Err(RemoveMemberError::PersonalOrganization);
        }

        let target = sqlx::query!(
            r#"SELECT role as "role!: MemberRole"
               FROM organization_members
               WHERE organization_id = $1 AND user_id = $2"#,
            organization_id,
            target_user_id,
        )
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(RemoveMemberError::MemberNotFound)?;

        if matches!(target.role, MemberRole::Admin)
            && admin_count(&mut tx, organization_id).await? <= 1
        {
            return Err(RemoveMemberError::LastAdmin);
        }

        sqlx::query!(
            "DELETE FROM organization_members WHERE organization_id = $1 AND user_id = $2",
            organization_id,
            target_user_id,
        )
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
    }

    /// Mirror of the cloud role-change contract: block self-demotion, block
    /// role changes on personal organizations, and block demoting the last
    /// admin — all inside a single transaction.
    pub async fn update_role_with_guardrails(
        pool: &SqlitePool,
        organization_id: Uuid,
        target_user_id: Uuid,
        new_role: MemberRole,
        acting_user_id: Uuid,
    ) -> Result<Self, UpdateRoleError> {
        if acting_user_id == target_user_id && matches!(new_role, MemberRole::Member) {
            return Err(UpdateRoleError::CannotDemoteSelf);
        }

        let mut tx = pool.begin().await?;

        let org = sqlx::query!(
            r#"SELECT is_personal as "is_personal!: bool"
               FROM organizations
               WHERE id = $1"#,
            organization_id,
        )
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(UpdateRoleError::OrganizationNotFound)?;

        if org.is_personal {
            return Err(UpdateRoleError::PersonalOrganization);
        }

        let target = sqlx::query_as!(
            OrganizationMember,
            r#"SELECT organization_id as "organization_id!: Uuid",
                      user_id         as "user_id!: Uuid",
                      role            as "role!: MemberRole",
                      joined_at       as "joined_at!: DateTime<Utc>",
                      last_seen_at    as "last_seen_at: DateTime<Utc>"
               FROM organization_members
               WHERE organization_id = $1 AND user_id = $2"#,
            organization_id,
            target_user_id,
        )
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(UpdateRoleError::MemberNotFound)?;

        if target.role == new_role {
            tx.commit().await?;
            return Ok(target);
        }

        if matches!(target.role, MemberRole::Admin)
            && matches!(new_role, MemberRole::Member)
            && admin_count(&mut tx, organization_id).await? <= 1
        {
            return Err(UpdateRoleError::LastAdmin);
        }

        let updated = sqlx::query_as!(
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
            target_user_id,
            new_role,
        )
        .fetch_one(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(updated)
    }
}

async fn admin_count(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    organization_id: Uuid,
) -> Result<i64, sqlx::Error> {
    let row = sqlx::query!(
        r#"SELECT COUNT(*) as "count!: i64"
           FROM organization_members
           WHERE organization_id = $1 AND role = 'admin'"#,
        organization_id,
    )
    .fetch_one(&mut **tx)
    .await?;
    Ok(row.count)
}

#[derive(Debug, thiserror::Error)]
pub enum RemoveMemberError {
    #[error("cannot remove yourself")]
    CannotRemoveSelf,
    #[error("organization not found")]
    OrganizationNotFound,
    #[error("cannot modify members of a personal organization")]
    PersonalOrganization,
    #[error("member not found")]
    MemberNotFound,
    #[error("cannot remove the last admin")]
    LastAdmin,
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum UpdateRoleError {
    #[error("cannot demote yourself")]
    CannotDemoteSelf,
    #[error("organization not found")]
    OrganizationNotFound,
    #[error("cannot modify members of a personal organization")]
    PersonalOrganization,
    #[error("member not found")]
    MemberNotFound,
    #[error("cannot demote the last admin")]
    LastAdmin,
    #[error(transparent)]
    Database(#[from] sqlx::Error),
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
