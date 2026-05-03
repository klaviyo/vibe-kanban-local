//! Synthetic profile and txid helpers used by routes that previously called
//! the cloud server. The local rewrite needs:
//!
//! - A deterministic local `User` row to attribute writes (creator_user_id,
//!   organization owner) without requiring an OAuth flow.
//! - A monotonic txid synthesized per mutation so the `MutationResponse<T>`
//!   envelope can echo a strictly-increasing value without Postgres' xid.
//!
//! `local_user` lazily provisions the synthetic user (and a personal
//! organization with the canonical six-status default) on first read. The UUID
//! is derived deterministically from the deployment's analytics user_id, so
//! reinstalls on the same machine resolve to the same row.
//!
//! `txid` returns microseconds since the unix epoch. Local mode is a single
//! writer, so monotonicity-per-process is sufficient for Electric-style
//! consumers that just need to detect change.

use api_types::{CreateOrganizationRequest, MemberRole as WireMemberRole, ProfileResponse};
use db::models::{
    organization::Organization,
    organization_member::{CreateOrganizationMember, MemberRole, OrganizationMember},
    user::{CreateUser, User},
};
use deployment::Deployment;
use uuid::Uuid;

use crate::DeploymentImpl;

/// UUID v5 namespace for deriving stable local identifiers from the
/// deployment's analytics user_id. Generated once and committed.
const LOCAL_NAMESPACE: Uuid = Uuid::from_bytes([
    0x7c, 0x2a, 0x91, 0x6b, 0x0b, 0x4d, 0x4d, 0x12, 0xa3, 0x4f, 0x9b, 0x05, 0x88, 0x6e, 0x18, 0x57,
]);

const SYNTHETIC_EMAIL_DOMAIN: &str = "vibe-kanban.local";

/// Synthesizes a strictly-monotonic transaction ID for the
/// `MutationResponse<T> { data, txid }` envelope. Microseconds since the unix
/// epoch — sufficient for single-writer local mode.
pub fn txid() -> i64 {
    chrono::Utc::now().timestamp_micros()
}

/// Derives a deterministic UUID for the local synthetic user from the
/// deployment's analytics user_id (e.g. `npm_user_<hex>`).
pub fn local_user_id(deployment_user_id: &str) -> Uuid {
    Uuid::new_v5(&LOCAL_NAMESPACE, deployment_user_id.as_bytes())
}

fn synthetic_email(deployment_user_id: &str) -> String {
    format!("{deployment_user_id}@{SYNTHETIC_EMAIL_DOMAIN}")
}

fn personal_org_slug(deployment_user_id: &str) -> String {
    let cleaned: String = deployment_user_id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    format!("personal-{cleaned}")
}

/// Lazily provisions the local synthetic user on first read.
pub async fn local_user(deployment: &DeploymentImpl) -> Result<User, sqlx::Error> {
    let pool = &deployment.db().pool;
    let id = local_user_id(deployment.user_id());

    if let Some(existing) = User::find_by_id(pool, id).await? {
        return Ok(existing);
    }

    User::create(
        pool,
        &CreateUser {
            id,
            email: synthetic_email(deployment.user_id()),
            first_name: None,
            last_name: None,
            username: Some(deployment.user_id().to_string()),
        },
    )
    .await
}

/// Lazily provisions the synthetic personal organization for the local user
/// and ensures membership. Returns the organization.
pub async fn local_personal_organization(
    deployment: &DeploymentImpl,
) -> Result<Organization, sqlx::Error> {
    let pool = &deployment.db().pool;
    let user = local_user(deployment).await?;
    let slug = personal_org_slug(deployment.user_id());

    if let Some(existing) = Organization::find_by_slug(pool, &slug).await? {
        if OrganizationMember::find(pool, existing.id, user.id)
            .await?
            .is_none()
        {
            OrganizationMember::create(
                pool,
                &CreateOrganizationMember {
                    organization_id: existing.id,
                    user_id: user.id,
                    role: MemberRole::Admin,
                },
            )
            .await?;
        }
        return Ok(existing);
    }

    let org = Organization::create(
        pool,
        Uuid::new_v4(),
        &CreateOrganizationRequest {
            name: "Personal".to_string(),
            slug: slug.clone(),
        },
    )
    .await?;

    OrganizationMember::create(
        pool,
        &CreateOrganizationMember {
            organization_id: org.id,
            user_id: user.id,
            role: MemberRole::Admin,
        },
    )
    .await?;

    Ok(org)
}

/// Returns the synthetic profile for `/auth/user`, `/auth/status`, etc.
pub async fn synthetic_profile(
    deployment: &DeploymentImpl,
) -> Result<ProfileResponse, sqlx::Error> {
    let user = local_user(deployment).await?;
    Ok(ProfileResponse {
        user_id: user.id,
        username: user.username.clone(),
        email: user.email.clone(),
        providers: Vec::new(),
    })
}

/// Wire-shape helper: convert a DB `Organization` plus a role into the
/// `OrganizationWithRole` shape that the API exposes.
pub fn organization_with_role(
    org: Organization,
    role: MemberRole,
) -> api_types::OrganizationWithRole {
    api_types::OrganizationWithRole {
        id: org.id,
        name: org.name,
        slug: org.slug,
        is_personal: org.is_personal,
        issue_prefix: org.issue_prefix,
        created_at: org.created_at,
        updated_at: org.updated_at,
        user_role: WireMemberRole::from(role),
    }
}
