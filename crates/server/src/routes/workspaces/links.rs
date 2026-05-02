use api_types::workspace_issue_link::CreateWorkspaceIssueLinkRequest;
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

/// Create a `workspace_issue_links` junction row linking the loaded workspace
/// to the requested issue. Idempotent: re-linking the same workspace/issue pair
/// is a no-op (returns the existing link's success without inserting a
/// duplicate).
pub async fn link_workspace(
    Extension(workspace): Extension<Workspace>,
    State(deployment): State<DeploymentImpl>,
    Json(payload): Json<LinkWorkspaceRequest>,
) -> Result<ResponseJson<ApiResponse<()>>, ApiError> {
    let pool = &deployment.db().pool;

    let existing = WorkspaceIssueLink::find_by_workspace(pool, workspace.id).await?;
    if existing
        .iter()
        .any(|link| link.issue_id == payload.issue_id)
    {
        return Ok(ResponseJson(ApiResponse::success(())));
    }

    WorkspaceIssueLink::create(
        pool,
        Uuid::new_v4(),
        &CreateWorkspaceIssueLinkRequest {
            id: None,
            workspace_id: workspace.id,
            issue_id: payload.issue_id,
            project_id: payload.project_id,
        },
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

    let links = WorkspaceIssueLink::find_by_workspace(pool, workspace_id).await?;
    for link in links {
        WorkspaceIssueLink::delete(pool, link.id).await?;
    }

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
