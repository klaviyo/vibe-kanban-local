//! Local-mode auth surface.
//!
//! Local mode has no OAuth handoff, no remote login, and no relay. The
//! synthetic local user is always "logged in." We keep the URL surface
//! (`/auth/methods`, `/auth/status`, `/auth/token`, `/auth/user`) so that
//! existing clients receive well-formed responses, but everything is
//! synthesized from the local DB.

use api_types::{AuthMethodsResponse, StatusResponse};
use axum::{Router, extract::State, response::Json as ResponseJson, routing::get};
use chrono::{DateTime, Utc};
use serde::Serialize;
use ts_rs::TS;
use utils::response::ApiResponse;

use crate::{DeploymentImpl, error::ApiError, runtime::synthetic};

/// Response from GET /api/auth/token - returns the current access token
#[derive(Debug, Serialize, TS)]
pub struct TokenResponse {
    pub access_token: String,
    pub expires_at: Option<DateTime<Utc>>,
}

/// Response from GET /api/auth/user - returns the current user ID
#[derive(Debug, Serialize, TS)]
pub struct CurrentUserResponse {
    pub user_id: String,
}

pub fn router() -> Router<DeploymentImpl> {
    Router::new()
        .route("/auth/methods", get(auth_methods))
        .route("/auth/status", get(status))
        .route("/auth/token", get(get_token))
        .route("/auth/user", get(get_current_user))
}

/// Local mode advertises no auth providers — the user is always the synthetic
/// local user.
async fn auth_methods(
    State(_deployment): State<DeploymentImpl>,
) -> Result<ResponseJson<ApiResponse<AuthMethodsResponse>>, ApiError> {
    Ok(ResponseJson(ApiResponse::success(AuthMethodsResponse {
        local_auth_enabled: false,
        oauth_providers: Vec::new(),
    })))
}

/// Always reports the synthetic local user as logged in.
async fn status(
    State(deployment): State<DeploymentImpl>,
) -> Result<ResponseJson<ApiResponse<StatusResponse>>, ApiError> {
    let profile = synthetic::synthetic_profile(&deployment).await?;
    Ok(ResponseJson(ApiResponse::success(StatusResponse {
        logged_in: true,
        profile: Some(profile),
        degraded: None,
    })))
}

/// Local mode has no real access token. We return a stable sentinel so any
/// caller that still echoes a token header has something to send.
async fn get_token(
    State(_deployment): State<DeploymentImpl>,
) -> Result<ResponseJson<ApiResponse<TokenResponse>>, ApiError> {
    Ok(ResponseJson(ApiResponse::success(TokenResponse {
        access_token: "local".to_string(),
        expires_at: None,
    })))
}

/// Returns the synthetic user's id as a string.
async fn get_current_user(
    State(deployment): State<DeploymentImpl>,
) -> Result<ResponseJson<ApiResponse<CurrentUserResponse>>, ApiError> {
    let user = synthetic::local_user(&deployment).await?;
    Ok(ResponseJson(ApiResponse::success(CurrentUserResponse {
        user_id: user.id.to_string(),
    })))
}
