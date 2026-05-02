use api_types::{CreateIssueTagRequest, IssueTag, ListIssueTagsResponse, MutationResponse};
use axum::{
    Router,
    extract::{Json, Path, Query, State},
    response::Json as ResponseJson,
    routing::get,
};
use db::models::issue_tag::IssueTag as IssueTagRow;
use deployment::Deployment;
use serde::Deserialize;
use utils::response::ApiResponse;
use uuid::Uuid;

use crate::{DeploymentImpl, error::ApiError, runtime::synthetic};

#[derive(Debug, Deserialize)]
pub(super) struct ListIssueTagsQuery {
    pub issue_id: Uuid,
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
    let pool = &deployment.db().pool;
    let rows = IssueTagRow::find_by_issue(pool, query.issue_id).await?;
    let issue_tags: Vec<IssueTag> = rows.into_iter().map(IssueTag::from).collect();
    Ok(ResponseJson(ApiResponse::success(ListIssueTagsResponse {
        issue_tags,
    })))
}

async fn get_issue_tag(
    State(deployment): State<DeploymentImpl>,
    Path(issue_tag_id): Path<Uuid>,
) -> Result<ResponseJson<ApiResponse<IssueTag>>, ApiError> {
    let pool = &deployment.db().pool;
    let row = IssueTagRow::find_by_id(pool, issue_tag_id)
        .await?
        .ok_or_else(|| ApiError::BadRequest("Issue tag not found".to_string()))?;
    Ok(ResponseJson(ApiResponse::success(IssueTag::from(row))))
}

async fn create_issue_tag(
    State(deployment): State<DeploymentImpl>,
    Json(request): Json<CreateIssueTagRequest>,
) -> Result<ResponseJson<ApiResponse<MutationResponse<IssueTag>>>, ApiError> {
    let pool = &deployment.db().pool;
    let id = request.id.unwrap_or_else(Uuid::new_v4);
    let row = IssueTagRow::create(pool, id, &request).await?;
    Ok(ResponseJson(ApiResponse::success(MutationResponse {
        data: IssueTag::from(row),
        txid: synthetic::txid(),
    })))
}

async fn delete_issue_tag(
    State(deployment): State<DeploymentImpl>,
    Path(issue_tag_id): Path<Uuid>,
) -> Result<ResponseJson<ApiResponse<()>>, ApiError> {
    let pool = &deployment.db().pool;
    IssueTagRow::delete(pool, issue_tag_id).await?;
    Ok(ResponseJson(ApiResponse::success(())))
}
