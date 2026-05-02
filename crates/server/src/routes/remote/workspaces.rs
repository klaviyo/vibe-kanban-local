//! Cloud `/remote/workspaces/by-local-id/...` was a lookup against the cloud
//! workspace mirror. Local mode has no such mirror — the canonical mapping
//! between a local workspace and an issue lives in `workspace_issue_links`.
//!
//! We synthesize an `api_types::Workspace` shape from `workspace_issue_links`
//! and the synthetic local user so existing callers (e.g. `routes::workspaces::git`)
//! still receive a useful payload when probing this URL. If no link exists we
//! return a `BadRequest`, which callers already tolerate.

use api_types::Workspace as WireWorkspace;
use axum::{
    Router,
    extract::{Path, State},
    response::Json as ResponseJson,
    routing::get,
};
use db::models::{workspace::Workspace, workspace_issue_link::WorkspaceIssueLink};
use deployment::Deployment;
use utils::response::ApiResponse;
use uuid::Uuid;

use crate::{DeploymentImpl, error::ApiError, runtime::synthetic};

pub(super) fn router() -> Router<DeploymentImpl> {
    Router::new().route(
        "/workspaces/by-local-id/{local_workspace_id}",
        get(get_workspace_by_local_id),
    )
}

async fn get_workspace_by_local_id(
    State(deployment): State<DeploymentImpl>,
    Path(local_workspace_id): Path<Uuid>,
) -> Result<ResponseJson<ApiResponse<WireWorkspace>>, ApiError> {
    let pool = &deployment.db().pool;

    let workspace = Workspace::find_by_id(pool, local_workspace_id)
        .await?
        .ok_or_else(|| ApiError::BadRequest("Workspace not found".to_string()))?;

    let links = WorkspaceIssueLink::find_by_workspace(pool, workspace.id).await?;
    let link = links.into_iter().next();

    let user = synthetic::local_user(&deployment).await?;

    let wire_workspace = WireWorkspace {
        id: workspace.id,
        project_id: link.as_ref().map(|l| l.project_id).unwrap_or_default(),
        owner_user_id: user.id,
        issue_id: link.as_ref().map(|l| l.issue_id),
        local_workspace_id: Some(workspace.id),
        name: workspace.name,
        archived: workspace.archived,
        files_changed: None,
        lines_added: None,
        lines_removed: None,
        created_at: workspace.created_at,
        updated_at: workspace.updated_at,
    };

    Ok(ResponseJson(ApiResponse::success(wire_workspace)))
}
