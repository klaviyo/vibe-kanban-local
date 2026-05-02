use api_types::{
    CreateIssueAssigneeRequest, IssueAssignee, ListIssueAssigneesResponse, MutationResponse,
};
use axum::{
    Router,
    extract::{Json, Path, Query, State},
    response::Json as ResponseJson,
    routing::get,
};
use db::models::issue_assignee::IssueAssignee as IssueAssigneeRow;
use deployment::Deployment;
use serde::Deserialize;
use utils::response::ApiResponse;
use uuid::Uuid;

use crate::{DeploymentImpl, error::ApiError, runtime::synthetic};

#[derive(Debug, Deserialize)]
pub(super) struct ListIssueAssigneesQuery {
    pub issue_id: Uuid,
}

pub(super) fn router() -> Router<DeploymentImpl> {
    Router::new()
        .route(
            "/issue-assignees",
            get(list_issue_assignees).post(create_issue_assignee),
        )
        .route(
            "/issue-assignees/{issue_assignee_id}",
            get(get_issue_assignee).delete(delete_issue_assignee),
        )
}

async fn list_issue_assignees(
    State(deployment): State<DeploymentImpl>,
    Query(query): Query<ListIssueAssigneesQuery>,
) -> Result<ResponseJson<ApiResponse<ListIssueAssigneesResponse>>, ApiError> {
    let pool = &deployment.db().pool;
    let rows = IssueAssigneeRow::find_by_issue(pool, query.issue_id).await?;
    let issue_assignees: Vec<IssueAssignee> = rows.into_iter().map(IssueAssignee::from).collect();
    Ok(ResponseJson(ApiResponse::success(
        ListIssueAssigneesResponse { issue_assignees },
    )))
}

async fn get_issue_assignee(
    State(deployment): State<DeploymentImpl>,
    Path(issue_assignee_id): Path<Uuid>,
) -> Result<ResponseJson<ApiResponse<IssueAssignee>>, ApiError> {
    let pool = &deployment.db().pool;
    let row = IssueAssigneeRow::find_by_id(pool, issue_assignee_id)
        .await?
        .ok_or_else(|| ApiError::BadRequest("Issue assignee not found".to_string()))?;
    Ok(ResponseJson(ApiResponse::success(IssueAssignee::from(row))))
}

async fn create_issue_assignee(
    State(deployment): State<DeploymentImpl>,
    Json(request): Json<CreateIssueAssigneeRequest>,
) -> Result<ResponseJson<ApiResponse<MutationResponse<IssueAssignee>>>, ApiError> {
    let pool = &deployment.db().pool;
    let id = request.id.unwrap_or_else(Uuid::new_v4);
    let row = IssueAssigneeRow::create(pool, id, &request).await?;
    Ok(ResponseJson(ApiResponse::success(MutationResponse {
        data: IssueAssignee::from(row),
        txid: synthetic::txid(),
    })))
}

async fn delete_issue_assignee(
    State(deployment): State<DeploymentImpl>,
    Path(issue_assignee_id): Path<Uuid>,
) -> Result<ResponseJson<ApiResponse<()>>, ApiError> {
    let pool = &deployment.db().pool;
    IssueAssigneeRow::delete(pool, issue_assignee_id).await?;
    Ok(ResponseJson(ApiResponse::success(())))
}
