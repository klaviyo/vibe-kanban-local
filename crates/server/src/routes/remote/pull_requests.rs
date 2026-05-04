use api_types::{ListPullRequestIssuesResponse, ListPullRequestsResponse};
use axum::{
    Json, Router,
    extract::{Query, State},
    response::Json as ResponseJson,
    routing::{get, post},
};
use db::models::pull_request::PullRequest;
use deployment::Deployment;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use utils::response::ApiResponse;
use uuid::Uuid;

use crate::{DeploymentImpl, error::ApiError};

#[derive(Debug, Deserialize)]
pub(super) struct ListPullRequestsQuery {
    pub issue_id: Option<Uuid>,
    pub project_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub(super) struct ListPullRequestIssuesQuery {
    pub project_id: Uuid,
}

pub fn router() -> Router<DeploymentImpl> {
    Router::new()
        .route("/pull-requests", get(list_pull_requests))
        .route("/pull-request-issues", get(list_pull_request_issues))
        .route("/pull-requests/link", post(link_pr_to_issue))
}

async fn list_pull_requests(
    State(deployment): State<DeploymentImpl>,
    Query(query): Query<ListPullRequestsQuery>,
) -> Result<ResponseJson<ApiResponse<ListPullRequestsResponse>>, ApiError> {
    let client = deployment.remote_client()?;
    let response = match (query.project_id, query.issue_id) {
        (Some(project_id), _) => client.list_project_pull_requests(project_id).await?,
        (None, Some(issue_id)) => client.list_pull_requests(issue_id).await?,
        (None, None) => {
            return Err(ApiError::BadRequest(
                "issue_id or project_id query parameter is required".into(),
            ));
        }
    };
    Ok(ResponseJson(ApiResponse::success(response)))
}

async fn list_pull_request_issues(
    State(deployment): State<DeploymentImpl>,
    Query(query): Query<ListPullRequestIssuesQuery>,
) -> Result<ResponseJson<ApiResponse<ListPullRequestIssuesResponse>>, ApiError> {
    let client = deployment.remote_client()?;
    let response = client.list_project_pull_request_issues(query.project_id).await?;
    Ok(ResponseJson(ApiResponse::success(response)))
}

/// Tracks a PR in the local database so `pr_monitor` can poll for status
/// changes and sync them to the remote. No remote server call is made here;
/// the actual remote PR creation is handled by the Electric mutation system.
#[derive(Debug, Deserialize, Serialize, TS)]
pub struct LinkPrToIssueRequest {
    pub pr_url: String,
    pub pr_number: i32,
    pub base_branch: String,
}

async fn link_pr_to_issue(
    State(deployment): State<DeploymentImpl>,
    Json(request): Json<LinkPrToIssueRequest>,
) -> Result<ResponseJson<ApiResponse<()>>, ApiError> {
    PullRequest::create(
        &deployment.db().pool,
        None,
        None,
        &request.pr_url,
        request.pr_number as i64,
        &request.base_branch,
    )
    .await?;

    Ok(ResponseJson(ApiResponse::success(())))
}
