use api_types::{
    CreateProjectStatusRequest, ListProjectStatusesResponse, MutationResponse, ProjectStatus,
    UpdateProjectStatusRequest,
};
use axum::{
    Router,
    extract::{Json, Path, Query, State},
    response::Json as ResponseJson,
    routing::get,
};
use db::models::project_status::ProjectStatus as ProjectStatusRow;
use deployment::Deployment;
use serde::Deserialize;
use utils::response::ApiResponse;
use uuid::Uuid;

use crate::{DeploymentImpl, error::ApiError};

#[derive(Debug, Deserialize)]
pub(super) struct ListProjectStatusesQuery {
    pub project_id: Uuid,
}

pub(super) fn router() -> Router<DeploymentImpl> {
    Router::new()
        .route(
            "/project-statuses",
            get(list_project_statuses).post(create_project_status),
        )
        .route(
            "/project-statuses/{status_id}",
            get(get_project_status)
                .patch(update_project_status)
                .delete(delete_project_status),
        )
}

async fn list_project_statuses(
    State(deployment): State<DeploymentImpl>,
    Query(query): Query<ListProjectStatusesQuery>,
) -> Result<ResponseJson<ApiResponse<ListProjectStatusesResponse>>, ApiError> {
    let pool = &deployment.db().pool;
    let rows = ProjectStatusRow::find_by_project(pool, query.project_id).await?;
    let project_statuses: Vec<ProjectStatus> = rows.into_iter().map(ProjectStatus::from).collect();
    Ok(ResponseJson(ApiResponse::success(
        ListProjectStatusesResponse { project_statuses },
    )))
}

async fn get_project_status(
    State(deployment): State<DeploymentImpl>,
    Path(status_id): Path<Uuid>,
) -> Result<ResponseJson<ApiResponse<ProjectStatus>>, ApiError> {
    let pool = &deployment.db().pool;
    let row = ProjectStatusRow::find_by_id(pool, status_id)
        .await?
        .ok_or_else(|| ApiError::BadRequest("Project status not found".to_string()))?;
    Ok(ResponseJson(ApiResponse::success(ProjectStatus::from(row))))
}

async fn create_project_status(
    State(deployment): State<DeploymentImpl>,
    Json(request): Json<CreateProjectStatusRequest>,
) -> Result<ResponseJson<ApiResponse<MutationResponse<ProjectStatus>>>, ApiError> {
    let pool = &deployment.db().pool;
    let id = request.id.unwrap_or_else(Uuid::new_v4);
    let response = ProjectStatusRow::create(pool, id, &request).await?;
    Ok(ResponseJson(ApiResponse::success(MutationResponse {
        data: response.data.into(),
        txid: response.txid,
    })))
}

async fn update_project_status(
    State(deployment): State<DeploymentImpl>,
    Path(status_id): Path<Uuid>,
    Json(request): Json<UpdateProjectStatusRequest>,
) -> Result<ResponseJson<ApiResponse<MutationResponse<ProjectStatus>>>, ApiError> {
    let pool = &deployment.db().pool;
    let response = ProjectStatusRow::update(pool, status_id, &request).await?;
    Ok(ResponseJson(ApiResponse::success(MutationResponse {
        data: response.data.into(),
        txid: response.txid,
    })))
}

async fn delete_project_status(
    State(deployment): State<DeploymentImpl>,
    Path(status_id): Path<Uuid>,
) -> Result<ResponseJson<ApiResponse<()>>, ApiError> {
    let pool = &deployment.db().pool;
    ProjectStatusRow::delete(pool, status_id).await?;
    Ok(ResponseJson(ApiResponse::success(())))
}
