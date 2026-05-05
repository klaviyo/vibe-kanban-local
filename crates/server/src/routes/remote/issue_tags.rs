use api_types::{
    CreateIssueTagRequest, DeleteResponse, IssueTag, ListIssueTagsResponse, MutationResponse,
};
use axum::{
    Router,
    extract::{Json, Path, Query, State},
    response::Json as ResponseJson,
    routing::get,
};
use serde::Deserialize;
use utils::response::ApiResponse;
use uuid::Uuid;

use crate::{DeploymentImpl, error::ApiError};

#[derive(Debug, Deserialize)]
pub(super) struct ListIssueTagsQuery {
    pub issue_id: Option<Uuid>,
    pub project_id: Option<Uuid>,
}

pub(super) fn router() -> Router<DeploymentImpl> {
    Router::new()
        .route("/issue-tags", get(list_issue_tags).post(create_issue_tag))
        .route(
            "/issue-tags/{issue_tag_id}",
            get(get_issue_tag).delete(delete_issue_tag),
        )
}

async fn list_issue_tags(
    State(deployment): State<DeploymentImpl>,
    Query(query): Query<ListIssueTagsQuery>,
) -> Result<ResponseJson<ApiResponse<ListIssueTagsResponse>>, ApiError> {
    let client = deployment.remote_client()?;
    let response = match (query.project_id, query.issue_id) {
        (Some(project_id), _) => client.list_project_issue_tags(project_id).await?,
        (None, Some(issue_id)) => client.list_issue_tags(issue_id).await?,
        (None, None) => {
            return Err(ApiError::BadRequest(
                "issue_id or project_id query parameter is required".into(),
            ));
        }
    };
    Ok(ResponseJson(ApiResponse::success(response)))
}

async fn get_issue_tag(
    State(deployment): State<DeploymentImpl>,
    Path(issue_tag_id): Path<Uuid>,
) -> Result<ResponseJson<ApiResponse<IssueTag>>, ApiError> {
    let client = deployment.remote_client()?;
    let response = client.get_issue_tag(issue_tag_id).await?;
    Ok(ResponseJson(ApiResponse::success(response)))
}

async fn create_issue_tag(
    State(deployment): State<DeploymentImpl>,
    Json(request): Json<CreateIssueTagRequest>,
) -> Result<ResponseJson<ApiResponse<MutationResponse<IssueTag>>>, ApiError> {
    let client = deployment.remote_client()?;
    let response = client.create_issue_tag(&request).await?;
    Ok(ResponseJson(ApiResponse::success(response)))
}

async fn delete_issue_tag(
    State(deployment): State<DeploymentImpl>,
    Path(issue_tag_id): Path<Uuid>,
) -> Result<ResponseJson<ApiResponse<DeleteResponse>>, ApiError> {
    let client = deployment.remote_client()?;
    let response = client.delete_issue_tag(issue_tag_id).await?;
    Ok(ResponseJson(ApiResponse::success(response)))
}
