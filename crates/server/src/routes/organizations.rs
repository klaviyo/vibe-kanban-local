use api_types::{
    AcceptInvitationResponse, CreateInvitationRequest, CreateInvitationResponse,
    CreateOrganizationRequest, CreateOrganizationResponse, GetInvitationResponse,
    GetOrganizationResponse, ListInvitationsResponse, ListMembersResponse,
    ListOrganizationsResponse, Organization as WireOrganization, OrganizationMemberWithProfile,
    RevokeInvitationRequest, UpdateMemberRoleRequest, UpdateMemberRoleResponse,
    UpdateOrganizationRequest,
};
use axum::{
    Router,
    extract::{Json, Path, State},
    http::StatusCode,
    response::Json as ResponseJson,
    routing::{delete, get, patch, post},
};
use chrono::{Duration, Utc};
use db::models::{
    invitation::{AcceptError, CreateInvitation, Invitation},
    organization::Organization,
    organization_member::{MemberRole, OrganizationMember, RemoveMemberError, UpdateRoleError},
    user::User,
};
use deployment::Deployment;
use utils::response::ApiResponse;
use uuid::Uuid;

use crate::{DeploymentImpl, error::ApiError, runtime::synthetic};

/// Cloud invitations expire after 7 days; mirror that here so a local-mode
/// `accept` flow rejects stale tokens with the same "expired" semantics.
const INVITATION_TTL_DAYS: i64 = 7;

pub fn router() -> Router<DeploymentImpl> {
    Router::new()
        .route("/organizations", get(list_organizations))
        .route("/organizations", post(create_organization))
        .route("/organizations/{id}", get(get_organization))
        .route("/organizations/{id}", patch(update_organization))
        .route("/organizations/{id}", delete(delete_organization))
        .route(
            "/organizations/{org_id}/invitations",
            post(create_invitation),
        )
        .route("/organizations/{org_id}/invitations", get(list_invitations))
        .route(
            "/organizations/{org_id}/invitations/revoke",
            post(revoke_invitation),
        )
        .route("/invitations/{token}", get(get_invitation))
        .route("/invitations/{token}/accept", post(accept_invitation))
        .route("/organizations/{org_id}/members", get(list_members))
        .route(
            "/organizations/{org_id}/members/{user_id}",
            delete(remove_member),
        )
        .route(
            "/organizations/{org_id}/members/{user_id}/role",
            patch(update_member_role),
        )
}

async fn list_organizations(
    State(deployment): State<DeploymentImpl>,
) -> Result<ResponseJson<ApiResponse<ListOrganizationsResponse>>, ApiError> {
    let pool = &deployment.db().pool;
    // Ensure the synthetic user + personal organization exist before listing.
    synthetic::local_personal_organization(&deployment).await?;

    let user = synthetic::local_user(&deployment).await?;
    let memberships = OrganizationMember::find_by_user(pool, user.id).await?;

    let mut organizations = Vec::with_capacity(memberships.len());
    for member in memberships {
        if let Some(org) = Organization::find_by_id(pool, member.organization_id).await? {
            organizations.push(synthetic::organization_with_role(org, member.role));
        }
    }

    Ok(ResponseJson(ApiResponse::success(
        ListOrganizationsResponse { organizations },
    )))
}

async fn get_organization(
    State(deployment): State<DeploymentImpl>,
    Path(id): Path<Uuid>,
) -> Result<ResponseJson<ApiResponse<GetOrganizationResponse>>, ApiError> {
    let pool = &deployment.db().pool;

    let user = synthetic::local_user(&deployment).await?;
    let organization = Organization::find_by_id(pool, id)
        .await?
        .ok_or_else(|| ApiError::BadRequest("Organization not found".to_string()))?;

    let member = OrganizationMember::find(pool, id, user.id)
        .await?
        .ok_or_else(|| ApiError::Forbidden("Not a member of this organization".to_string()))?;

    let user_role = match member.role {
        MemberRole::Admin => "ADMIN".to_string(),
        MemberRole::Member => "MEMBER".to_string(),
    };

    Ok(ResponseJson(ApiResponse::success(
        GetOrganizationResponse {
            organization: WireOrganization::from(organization),
            user_role,
        },
    )))
}

async fn create_organization(
    State(deployment): State<DeploymentImpl>,
    Json(request): Json<CreateOrganizationRequest>,
) -> Result<ResponseJson<ApiResponse<CreateOrganizationResponse>>, ApiError> {
    let pool = &deployment.db().pool;
    let user = synthetic::local_user(&deployment).await?;

    let org = Organization::create(pool, Uuid::new_v4(), &request).await?;

    OrganizationMember::create(
        pool,
        &db::models::organization_member::CreateOrganizationMember {
            organization_id: org.id,
            user_id: user.id,
            role: MemberRole::Admin,
        },
    )
    .await?;

    deployment
        .track_if_analytics_allowed(
            "organization_created",
            serde_json::json!({
                "org_id": org.id.to_string(),
            }),
        )
        .await;

    let response = CreateOrganizationResponse {
        organization: synthetic::organization_with_role(org, MemberRole::Admin),
    };

    Ok(ResponseJson(ApiResponse::success(response)))
}

async fn update_organization(
    State(deployment): State<DeploymentImpl>,
    Path(id): Path<Uuid>,
    Json(request): Json<UpdateOrganizationRequest>,
) -> Result<ResponseJson<ApiResponse<WireOrganization>>, ApiError> {
    let pool = &deployment.db().pool;
    let updated = Organization::update(pool, id, &request).await?;
    Ok(ResponseJson(ApiResponse::success(WireOrganization::from(
        updated,
    ))))
}

async fn delete_organization(
    State(deployment): State<DeploymentImpl>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    let pool = &deployment.db().pool;
    Organization::delete(pool, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

// Local invitations are stored in `organization_invitations` (mirrors the
// cloud's table). We don't send invitation emails locally — the synthetic
// user flow accepts tokens directly — but the route surface (`POST/GET
// /organizations/{id}/invitations`, `POST .../revoke`, `GET
// /invitations/{token}`, `POST /invitations/{token}/accept`) keeps
// pre-cutover frontend / MCP callers working unchanged.

async fn create_invitation(
    State(deployment): State<DeploymentImpl>,
    Path(org_id): Path<Uuid>,
    Json(request): Json<CreateInvitationRequest>,
) -> Result<ResponseJson<ApiResponse<CreateInvitationResponse>>, ApiError> {
    let pool = &deployment.db().pool;
    let user = synthetic::local_user(&deployment).await?;

    let token = Uuid::new_v4().to_string();
    let expires_at = Utc::now() + Duration::days(INVITATION_TTL_DAYS);
    let role = MemberRole::from(request.role);

    let invitation = Invitation::create(
        pool,
        &CreateInvitation {
            id: Uuid::new_v4(),
            organization_id: org_id,
            invited_by_user_id: Some(user.id),
            email: &request.email,
            role,
            token: &token,
            expires_at,
        },
    )
    .await?;

    Ok(ResponseJson(ApiResponse::success(
        CreateInvitationResponse {
            invitation: invitation.into_wire(),
        },
    )))
}

async fn list_invitations(
    State(deployment): State<DeploymentImpl>,
    Path(org_id): Path<Uuid>,
) -> Result<ResponseJson<ApiResponse<ListInvitationsResponse>>, ApiError> {
    let pool = &deployment.db().pool;
    let invitations = Invitation::list_pending_by_organization(pool, org_id).await?;
    let invitations = invitations.into_iter().map(Invitation::into_wire).collect();
    Ok(ResponseJson(ApiResponse::success(
        ListInvitationsResponse { invitations },
    )))
}

async fn get_invitation(
    State(deployment): State<DeploymentImpl>,
    Path(token): Path<String>,
) -> Result<ResponseJson<ApiResponse<GetInvitationResponse>>, ApiError> {
    let pool = &deployment.db().pool;
    let invitation = Invitation::find_by_token(pool, &token)
        .await?
        .ok_or_else(|| ApiError::BadRequest("Invitation not found".to_string()))?;

    let organization = Organization::find_by_id(pool, invitation.organization_id)
        .await?
        .ok_or_else(|| ApiError::BadRequest("Organization not found".to_string()))?;

    Ok(ResponseJson(ApiResponse::success(GetInvitationResponse {
        id: invitation.id,
        organization_slug: organization.slug,
        role: invitation.role.into(),
        expires_at: invitation.expires_at,
    })))
}

async fn revoke_invitation(
    State(deployment): State<DeploymentImpl>,
    Path(org_id): Path<Uuid>,
    Json(payload): Json<RevokeInvitationRequest>,
) -> Result<StatusCode, ApiError> {
    let pool = &deployment.db().pool;
    Invitation::revoke(pool, org_id, payload.invitation_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn accept_invitation(
    State(deployment): State<DeploymentImpl>,
    Path(invitation_token): Path<String>,
) -> Result<ResponseJson<ApiResponse<AcceptInvitationResponse>>, ApiError> {
    let pool = &deployment.db().pool;
    let user = synthetic::local_user(&deployment).await?;

    let accepted = Invitation::accept(pool, &invitation_token, user.id)
        .await
        .map_err(|err| match err {
            AcceptError::NotFound => ApiError::BadRequest("Invitation not found".to_string()),
            AcceptError::AlreadyResolved => {
                ApiError::BadRequest("Invitation already resolved".to_string())
            }
            AcceptError::Expired => ApiError::BadRequest("Invitation has expired".to_string()),
            AcceptError::Database(db) => ApiError::Database(db),
        })?;

    Ok(ResponseJson(ApiResponse::success(
        AcceptInvitationResponse {
            organization_id: accepted.organization_id.to_string(),
            organization_slug: accepted.organization_slug,
            role: accepted.role.into(),
        },
    )))
}

async fn list_members(
    State(deployment): State<DeploymentImpl>,
    Path(org_id): Path<Uuid>,
) -> Result<ResponseJson<ApiResponse<ListMembersResponse>>, ApiError> {
    let pool = &deployment.db().pool;
    let members = OrganizationMember::find_by_organization(pool, org_id).await?;

    let mut profiles: Vec<OrganizationMemberWithProfile> = Vec::with_capacity(members.len());
    for member in members {
        let user = User::find_by_id(pool, member.user_id).await?;
        profiles.push(OrganizationMemberWithProfile {
            user_id: member.user_id,
            role: member.role.into(),
            joined_at: member.joined_at,
            first_name: user.as_ref().and_then(|u| u.first_name.clone()),
            last_name: user.as_ref().and_then(|u| u.last_name.clone()),
            username: user.as_ref().and_then(|u| u.username.clone()),
            email: user.as_ref().map(|u| u.email.clone()),
            avatar_url: None,
        });
    }

    Ok(ResponseJson(ApiResponse::success(ListMembersResponse {
        members: profiles,
    })))
}

async fn remove_member(
    State(deployment): State<DeploymentImpl>,
    Path((org_id, user_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, ApiError> {
    let pool = &deployment.db().pool;
    let acting = synthetic::local_user(&deployment).await?;
    OrganizationMember::remove_with_guardrails(pool, org_id, user_id, acting.id)
        .await
        .map_err(|err| match err {
            RemoveMemberError::CannotRemoveSelf => {
                ApiError::BadRequest("Cannot remove yourself".to_string())
            }
            RemoveMemberError::OrganizationNotFound => {
                ApiError::BadRequest("Organization not found".to_string())
            }
            RemoveMemberError::PersonalOrganization => {
                ApiError::BadRequest("Cannot modify members of a personal organization".to_string())
            }
            RemoveMemberError::MemberNotFound => {
                ApiError::BadRequest("Member not found".to_string())
            }
            RemoveMemberError::LastAdmin => {
                ApiError::Conflict("Cannot remove the last admin".to_string())
            }
            RemoveMemberError::Database(db) => ApiError::Database(db),
        })?;
    Ok(StatusCode::NO_CONTENT)
}

async fn update_member_role(
    State(deployment): State<DeploymentImpl>,
    Path((org_id, user_id)): Path<(Uuid, Uuid)>,
    Json(request): Json<UpdateMemberRoleRequest>,
) -> Result<ResponseJson<ApiResponse<UpdateMemberRoleResponse>>, ApiError> {
    let pool = &deployment.db().pool;
    let acting = synthetic::local_user(&deployment).await?;
    let role = MemberRole::from(request.role);
    let updated =
        OrganizationMember::update_role_with_guardrails(pool, org_id, user_id, role, acting.id)
            .await
            .map_err(|err| match err {
                UpdateRoleError::CannotDemoteSelf => {
                    ApiError::BadRequest("Cannot demote yourself".to_string())
                }
                UpdateRoleError::OrganizationNotFound => {
                    ApiError::BadRequest("Organization not found".to_string())
                }
                UpdateRoleError::PersonalOrganization => ApiError::BadRequest(
                    "Cannot modify members of a personal organization".to_string(),
                ),
                UpdateRoleError::MemberNotFound => {
                    ApiError::BadRequest("Member not found".to_string())
                }
                UpdateRoleError::LastAdmin => {
                    ApiError::Conflict("Cannot demote the last admin".to_string())
                }
                UpdateRoleError::Database(db) => ApiError::Database(db),
            })?;
    Ok(ResponseJson(ApiResponse::success(
        UpdateMemberRoleResponse {
            user_id: updated.user_id,
            role: updated.role.into(),
        },
    )))
}
