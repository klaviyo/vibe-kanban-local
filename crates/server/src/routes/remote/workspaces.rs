use api_types::{ListWorkspacesResponse, Workspace};
use axum::{
    Router,
    extract::{Path, Query, State},
    response::Json as ResponseJson,
    routing::get,
};
use serde::Deserialize;
use utils::response::ApiResponse;
use uuid::Uuid;

use crate::{DeploymentImpl, error::ApiError};

#[derive(Debug, Deserialize)]
pub(super) struct ListWorkspacesQuery {
    pub project_id: Option<Uuid>,
    pub owner_user_id: Option<Uuid>,
}

pub(super) fn router() -> Router<DeploymentImpl> {
    Router::new()
        .route("/workspaces", get(list_workspaces))
        .route(
            "/workspaces/by-local-id/{local_workspace_id}",
            get(get_workspace_by_local_id),
        )
}

async fn list_workspaces(
    State(deployment): State<DeploymentImpl>,
    Query(query): Query<ListWorkspacesQuery>,
) -> Result<ResponseJson<ApiResponse<ListWorkspacesResponse>>, ApiError> {
    let client = deployment.remote_client()?;
    let response = match (query.project_id, query.owner_user_id) {
        (Some(project_id), _) => client.list_project_workspaces(project_id).await?,
        // The remote already scopes /v1/fallback/user_workspaces to the
        // authenticated user; the owner_user_id query param exists on the
        // shape so the resolver can forward it but the remote ignores it.
        (None, Some(_)) | (None, None) => client.list_user_workspaces().await?,
    };
    Ok(ResponseJson(ApiResponse::success(response)))
}

async fn get_workspace_by_local_id(
    State(deployment): State<DeploymentImpl>,
    Path(local_workspace_id): Path<Uuid>,
) -> Result<ResponseJson<ApiResponse<Workspace>>, ApiError> {
    let client = deployment.remote_client()?;
    let workspace = client.get_workspace_by_local_id(local_workspace_id).await?;
    Ok(ResponseJson(ApiResponse::success(workspace)))
}
