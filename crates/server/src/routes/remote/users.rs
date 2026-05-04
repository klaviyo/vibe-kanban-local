//! Users endpoint — **read-only** in single-user local mode.
//!
//! The cloud product exposes full user CRUD (sign-up, profile editing,
//! account deletion). Single-user local mode has only the synthetic
//! user, provisioned lazily by `runtime::synthetic::local_user`, so
//! creates, updates, and deletes have no on-the-wire surface. The
//! kanban frontend resolves this entity through `localRouteResolver`
//! and reads the list via `extractRows`, which looks up
//! `data[<table>]` on the envelope — so `GET /users?organization_id={uuid}`
//! must return `ApiResponse<UsersResponse>` where `UsersResponse`
//! exposes a single `users: Vec<User>` field. This matches the
//! table-keyed convention the real-CRUD modules in this directory
//! already use (e.g. `ListIssueFollowersResponse { issue_followers: Vec<…> }`).
//!
//! Implementation: enumerate `OrganizationMember::find_by_organization`,
//! then load the corresponding `User` rows. In practice this returns
//! either a single-element list (the synthetic user, when the requested
//! org is the personal org) or an empty list (when the requested org
//! has no members — e.g. a not-yet-seeded org id).

use api_types::User;
use axum::{
    Router,
    extract::{Query, State},
    response::Json as ResponseJson,
    routing::get,
};
use db::models::{organization_member::OrganizationMember, user::User as UserRow};
use deployment::Deployment;
use serde::{Deserialize, Serialize};
use utils::response::ApiResponse;
use uuid::Uuid;

use crate::{DeploymentImpl, error::ApiError};

/// Table-keyed list envelope for `GET /users`. The wire field name
/// (`users`) matches the snake-case table name the kanban frontend's
/// `extractRows` looks up via `data[table]`. Defined locally rather
/// than in `api-types` because no `ListUsersResponse` wire type is
/// shared with the cloud surface — this is a local-only single-user
/// shape contract.
#[derive(Debug, Serialize)]
pub(super) struct UsersResponse {
    pub users: Vec<User>,
}

#[derive(Debug, Deserialize)]
pub(super) struct ListUsersQuery {
    pub organization_id: Uuid,
}

pub(super) fn router() -> Router<DeploymentImpl> {
    Router::new().route("/users", get(list_users))
}

/// Lists users that are members of the requested organization. In
/// single-user local mode this is the synthetic user as a single-element
/// list when the synthetic user belongs to the requested org, empty
/// otherwise. Membership is sourced from `organization_members` so the
/// shape mirrors the cloud contract even though the local product is
/// single-user.
async fn list_users(
    State(deployment): State<DeploymentImpl>,
    Query(query): Query<ListUsersQuery>,
) -> Result<ResponseJson<ApiResponse<UsersResponse>>, ApiError> {
    let pool = &deployment.db().pool;
    let members = OrganizationMember::find_by_organization(pool, query.organization_id).await?;

    let mut users = Vec::with_capacity(members.len());
    for member in members {
        if let Some(user) = UserRow::find_by_id(pool, member.user_id).await? {
            users.push(User::from(user));
        }
    }

    Ok(ResponseJson(ApiResponse::success(UsersResponse { users })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn empty_users_envelope_shape() {
        let envelope: ApiResponse<UsersResponse> =
            ApiResponse::success(UsersResponse { users: Vec::new() });
        let body = serde_json::to_value(&envelope).expect("serialize envelope");
        assert_eq!(
            body,
            json!({
                "success": true,
                "data": { "users": [] },
                "error_data": null,
                "message": null,
            }),
            "users list must use the table-keyed ApiResponse envelope; \
             extractRows reads data[\"users\"] and an org with no matching \
             members yields an empty users array, not a 404"
        );
    }
}
