use api_types::{
    CreateProjectRequest, ListProjectsResponse, MutationResponse, Project, UpdateProjectRequest,
};
use axum::{
    Router,
    extract::{Json, Path, Query, State},
    response::Json as ResponseJson,
    routing::get,
};
use db::models::project::{CreateProject, ProjectRow};
use deployment::Deployment;
use serde::Deserialize;
use utils::response::ApiResponse;
use uuid::Uuid;

use crate::{DeploymentImpl, error::ApiError, runtime::synthetic};

#[derive(Debug, Deserialize)]
pub(super) struct ListRemoteProjectsQuery {
    pub organization_id: Uuid,
}

pub(super) fn router() -> Router<DeploymentImpl> {
    Router::new()
        .route(
            "/projects",
            get(list_remote_projects).post(create_remote_project),
        )
        .route(
            "/projects/{project_id}",
            get(get_remote_project)
                .patch(update_remote_project)
                .delete(delete_remote_project),
        )
}

async fn list_remote_projects(
    State(deployment): State<DeploymentImpl>,
    Query(query): Query<ListRemoteProjectsQuery>,
) -> Result<ResponseJson<ApiResponse<ListProjectsResponse>>, ApiError> {
    let pool = &deployment.db().pool;
    let rows = ProjectRow::find_by_organization(pool, query.organization_id).await?;
    let projects: Vec<Project> = rows.into_iter().map(Project::from).collect();
    Ok(ResponseJson(ApiResponse::success(ListProjectsResponse {
        projects,
    })))
}

async fn get_remote_project(
    State(deployment): State<DeploymentImpl>,
    Path(project_id): Path<Uuid>,
) -> Result<ResponseJson<ApiResponse<Project>>, ApiError> {
    let pool = &deployment.db().pool;
    let row = ProjectRow::find_by_id(pool, project_id)
        .await?
        .ok_or_else(|| ApiError::BadRequest("Project not found".to_string()))?;
    Ok(ResponseJson(ApiResponse::success(Project::from(row))))
}

/// Creates a project and seeds the canonical six default statuses in the same
/// transaction. The project must never appear statusless.
async fn create_remote_project(
    State(deployment): State<DeploymentImpl>,
    Json(request): Json<CreateProjectRequest>,
) -> Result<ResponseJson<ApiResponse<MutationResponse<Project>>>, ApiError> {
    let pool = &deployment.db().pool;
    let id = request.id.unwrap_or_else(Uuid::new_v4);
    let create = CreateProject {
        id,
        organization_id: request.organization_id,
        name: request.name,
        color: request.color,
    };

    let row = ProjectRow::create(pool, &create).await?;
    synthetic::seed_default_project_statuses(pool, row.id).await?;

    Ok(ResponseJson(ApiResponse::success(MutationResponse {
        data: Project::from(row),
        txid: synthetic::txid(),
    })))
}

async fn update_remote_project(
    State(deployment): State<DeploymentImpl>,
    Path(project_id): Path<Uuid>,
    Json(request): Json<UpdateProjectRequest>,
) -> Result<ResponseJson<ApiResponse<MutationResponse<Project>>>, ApiError> {
    let pool = &deployment.db().pool;
    let row = ProjectRow::update(pool, project_id, &request).await?;
    Ok(ResponseJson(ApiResponse::success(MutationResponse {
        data: Project::from(row),
        txid: synthetic::txid(),
    })))
}

async fn delete_remote_project(
    State(deployment): State<DeploymentImpl>,
    Path(project_id): Path<Uuid>,
) -> Result<ResponseJson<ApiResponse<()>>, ApiError> {
    let pool = &deployment.db().pool;
    ProjectRow::delete(pool, project_id).await?;
    Ok(ResponseJson(ApiResponse::success(())))
}
