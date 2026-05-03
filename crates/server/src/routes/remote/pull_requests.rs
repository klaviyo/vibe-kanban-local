use api_types::{ListPullRequestsQuery, ListPullRequestsResponse};
use axum::{
    Json, Router,
    extract::{Query, State},
    response::Json as ResponseJson,
    routing::{get, post},
};
use db::models::{pull_request::PullRequest, pull_request_issue::PullRequestIssueRepository};
use deployment::Deployment;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use utils::response::ApiResponse;
use uuid::Uuid;

use crate::{DeploymentImpl, error::ApiError};

pub fn router() -> Router<DeploymentImpl> {
    Router::new()
        .route("/pull-requests", get(list_pull_requests))
        .route("/pull-requests/link", post(link_pr_to_issue))
}

async fn list_pull_requests(
    State(deployment): State<DeploymentImpl>,
    Query(query): Query<ListPullRequestsQuery>,
) -> Result<ResponseJson<ApiResponse<ListPullRequestsResponse>>, ApiError> {
    let pull_requests =
        PullRequestIssueRepository::list_by_issue(&deployment.db().pool, query.issue_id).await?;
    Ok(ResponseJson(ApiResponse::success(
        ListPullRequestsResponse { pull_requests },
    )))
}

/// Tracks a PR in the local database so `pr_monitor` can poll for status
/// changes, and links it to the supplied issue via the local
/// `pull_request_issues` junction (mirroring the cloud's join shape).
#[derive(Debug, Deserialize, Serialize, TS)]
pub struct LinkPrToIssueRequest {
    pub pr_url: String,
    pub pr_number: i32,
    pub base_branch: String,
    pub issue_id: Uuid,
}

async fn link_pr_to_issue(
    State(deployment): State<DeploymentImpl>,
    Json(request): Json<LinkPrToIssueRequest>,
) -> Result<ResponseJson<ApiResponse<()>>, ApiError> {
    let pool = &deployment.db().pool;
    let pr = PullRequest::create(
        pool,
        None,
        None,
        &request.pr_url,
        request.pr_number as i64,
        &request.base_branch,
    )
    .await?;

    PullRequestIssueRepository::link(pool, &pr.id, request.issue_id).await?;

    Ok(ResponseJson(ApiResponse::success(())))
}
