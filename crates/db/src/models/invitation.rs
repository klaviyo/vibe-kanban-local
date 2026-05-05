use api_types::{self as wire};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool, Type};
use uuid::Uuid;

use crate::models::organization_member::MemberRole;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Type, Serialize, Deserialize)]
#[sqlx(type_name = "invitation_status", rename_all = "lowercase")]
pub enum InvitationStatus {
    Pending,
    Accepted,
    Declined,
    Expired,
}

impl From<InvitationStatus> for wire::InvitationStatus {
    fn from(value: InvitationStatus) -> Self {
        match value {
            InvitationStatus::Pending => wire::InvitationStatus::Pending,
            InvitationStatus::Accepted => wire::InvitationStatus::Accepted,
            InvitationStatus::Declined => wire::InvitationStatus::Declined,
            InvitationStatus::Expired => wire::InvitationStatus::Expired,
        }
    }
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Invitation {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub invited_by_user_id: Option<Uuid>,
    pub email: String,
    pub role: MemberRole,
    pub status: InvitationStatus,
    pub token: String,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Inputs for `Invitation::create`. Bundled into a struct to keep the model
/// API readable and to avoid the clippy `too_many_arguments` warning.
#[derive(Debug, Clone)]
pub struct CreateInvitation<'a> {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub invited_by_user_id: Option<Uuid>,
    pub email: &'a str,
    pub role: MemberRole,
    pub token: &'a str,
    pub expires_at: DateTime<Utc>,
}

impl Invitation {
    pub async fn create(
        pool: &SqlitePool,
        data: &CreateInvitation<'_>,
    ) -> Result<Self, sqlx::Error> {
        sqlx::query_as!(
            Invitation,
            r#"INSERT INTO organization_invitations
                   (id, organization_id, invited_by_user_id, email, role, token, expires_at)
               VALUES ($1, $2, $3, $4, $5, $6, $7)
               RETURNING id                  as "id!: Uuid",
                         organization_id     as "organization_id!: Uuid",
                         invited_by_user_id  as "invited_by_user_id: Uuid",
                         email,
                         role                as "role!: MemberRole",
                         status              as "status!: InvitationStatus",
                         token,
                         expires_at          as "expires_at!: DateTime<Utc>",
                         created_at          as "created_at!: DateTime<Utc>",
                         updated_at          as "updated_at!: DateTime<Utc>""#,
            data.id,
            data.organization_id,
            data.invited_by_user_id,
            data.email,
            data.role,
            data.token,
            data.expires_at,
        )
        .fetch_one(pool)
        .await
    }

    pub async fn find_by_token(
        pool: &SqlitePool,
        token: &str,
    ) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as!(
            Invitation,
            r#"SELECT id                  as "id!: Uuid",
                      organization_id     as "organization_id!: Uuid",
                      invited_by_user_id  as "invited_by_user_id: Uuid",
                      email,
                      role                as "role!: MemberRole",
                      status              as "status!: InvitationStatus",
                      token,
                      expires_at          as "expires_at!: DateTime<Utc>",
                      created_at          as "created_at!: DateTime<Utc>",
                      updated_at          as "updated_at!: DateTime<Utc>"
               FROM organization_invitations
               WHERE token = $1"#,
            token,
        )
        .fetch_optional(pool)
        .await
    }

    pub async fn list_pending_by_organization(
        pool: &SqlitePool,
        organization_id: Uuid,
    ) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as!(
            Invitation,
            r#"SELECT id                  as "id!: Uuid",
                      organization_id     as "organization_id!: Uuid",
                      invited_by_user_id  as "invited_by_user_id: Uuid",
                      email,
                      role                as "role!: MemberRole",
                      status              as "status!: InvitationStatus",
                      token,
                      expires_at          as "expires_at!: DateTime<Utc>",
                      created_at          as "created_at!: DateTime<Utc>",
                      updated_at          as "updated_at!: DateTime<Utc>"
               FROM organization_invitations
               WHERE organization_id = $1 AND status = 'pending'
               ORDER BY created_at ASC"#,
            organization_id,
        )
        .fetch_all(pool)
        .await
    }

    pub async fn revoke(
        pool: &SqlitePool,
        organization_id: Uuid,
        invitation_id: Uuid,
    ) -> Result<u64, sqlx::Error> {
        let result = sqlx::query!(
            "DELETE FROM organization_invitations
             WHERE organization_id = $1 AND id = $2",
            organization_id,
            invitation_id,
        )
        .execute(pool)
        .await?;
        Ok(result.rows_affected())
    }

    /// Marks the pending invitation as accepted and (idempotently) makes the
    /// accepting user a member of the organization. Runs in one transaction so
    /// callers never observe an "accepted" invitation without the
    /// corresponding membership.
    pub async fn accept(
        pool: &SqlitePool,
        token: &str,
        user_id: Uuid,
    ) -> Result<AcceptedInvitation, AcceptError> {
        let mut tx = pool.begin().await?;

        let invitation = sqlx::query_as!(
            Invitation,
            r#"SELECT id                  as "id!: Uuid",
                      organization_id     as "organization_id!: Uuid",
                      invited_by_user_id  as "invited_by_user_id: Uuid",
                      email,
                      role                as "role!: MemberRole",
                      status              as "status!: InvitationStatus",
                      token,
                      expires_at          as "expires_at!: DateTime<Utc>",
                      created_at          as "created_at!: DateTime<Utc>",
                      updated_at          as "updated_at!: DateTime<Utc>"
               FROM organization_invitations
               WHERE token = $1"#,
            token,
        )
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(AcceptError::NotFound)?;

        if invitation.status != InvitationStatus::Pending {
            return Err(AcceptError::AlreadyResolved);
        }
        if invitation.expires_at < Utc::now() {
            return Err(AcceptError::Expired);
        }

        sqlx::query!(
            r#"UPDATE organization_invitations
               SET status     = 'accepted',
                   updated_at = datetime('now', 'subsec')
               WHERE id = $1"#,
            invitation.id,
        )
        .execute(&mut *tx)
        .await?;

        // Idempotent membership insert: if the user is already a member we
        // keep their existing role rather than overwriting it.
        sqlx::query!(
            r#"INSERT OR IGNORE INTO organization_members
                   (organization_id, user_id, role)
               VALUES ($1, $2, $3)"#,
            invitation.organization_id,
            user_id,
            invitation.role,
        )
        .execute(&mut *tx)
        .await?;

        let org_row = sqlx::query!(
            r#"SELECT id   as "id!: Uuid",
                      slug
               FROM organizations
               WHERE id = $1"#,
            invitation.organization_id,
        )
        .fetch_one(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(AcceptedInvitation {
            organization_id: org_row.id,
            organization_slug: org_row.slug,
            role: invitation.role,
        })
    }
}

#[derive(Debug, Clone)]
pub struct AcceptedInvitation {
    pub organization_id: Uuid,
    pub organization_slug: String,
    pub role: MemberRole,
}

#[derive(Debug, thiserror::Error)]
pub enum AcceptError {
    #[error("invitation not found")]
    NotFound,
    #[error("invitation already accepted, declined, or expired")]
    AlreadyResolved,
    #[error("invitation has expired")]
    Expired,
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

impl Invitation {
    /// Build the wire-shape `Invitation` payload — the cloud route bundles it
    /// inside `CreateInvitationResponse`/`ListInvitationsResponse`. Convert
    /// here so the route handler stays short.
    pub fn into_wire(self) -> wire::Invitation {
        wire::Invitation {
            id: self.id,
            organization_id: self.organization_id,
            invited_by_user_id: self.invited_by_user_id,
            email: self.email,
            role: self.role.into(),
            status: self.status.into(),
            token: self.token,
            created_at: self.created_at,
            expires_at: self.expires_at,
        }
    }
}
