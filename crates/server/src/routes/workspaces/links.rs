use axum::{
    Extension, Json, Router,
    extract::{Path as AxumPath, State},
    middleware::from_fn_with_state,
    response::Json as ResponseJson,
    routing::{delete, post},
};
use db::models::{workspace::Workspace, workspace_issue_link::WorkspaceIssueLink};
use deployment::Deployment;
use serde::Deserialize;
use utils::response::ApiResponse;
use uuid::Uuid;

use crate::{DeploymentImpl, error::ApiError, middleware::load_workspace_middleware};

#[derive(Debug, Deserialize)]
pub struct LinkWorkspaceRequest {
    pub project_id: Uuid,
    pub issue_id: Uuid,
}

/// Replace any existing `workspace_issue_links` rows for the workspace with a
/// single row pointing at the requested issue. A workspace must resolve to
/// exactly one linked issue: `get_workspace_by_local_id()` and the cloud
/// contract both treat the relationship as singular, so a relink to a
/// different issue must not leave the prior row behind. The model performs
/// the delete + insert inside one transaction so callers never observe two
/// active links.
pub async fn link_workspace(
    Extension(workspace): Extension<Workspace>,
    State(deployment): State<DeploymentImpl>,
    Json(payload): Json<LinkWorkspaceRequest>,
) -> Result<ResponseJson<ApiResponse<()>>, ApiError> {
    let pool = &deployment.db().pool;

    WorkspaceIssueLink::replace_for_workspace(
        pool,
        workspace.id,
        payload.issue_id,
        payload.project_id,
    )
    .await?;

    Ok(ResponseJson(ApiResponse::success(())))
}

/// Remove every `workspace_issue_links` row for the given workspace. The
/// historical cloud route returned 204 even when the workspace had no link, so
/// we mirror that idempotency by ignoring the affected-row count.
pub async fn unlink_workspace(
    AxumPath(workspace_id): AxumPath<Uuid>,
    State(deployment): State<DeploymentImpl>,
) -> Result<ResponseJson<ApiResponse<()>>, ApiError> {
    let pool = &deployment.db().pool;

    WorkspaceIssueLink::delete_by_workspace(pool, workspace_id).await?;

    Ok(ResponseJson(ApiResponse::success(())))
}

pub fn router(deployment: &DeploymentImpl) -> Router<DeploymentImpl> {
    let post_router = Router::new()
        .route("/", post(link_workspace))
        .layer(from_fn_with_state(
            deployment.clone(),
            load_workspace_middleware,
        ));

    let delete_router = Router::new().route("/", delete(unlink_workspace));

    post_router.merge(delete_router)
}
