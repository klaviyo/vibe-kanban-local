use api_types::{ListPullRequestIssuesResponse, ListPullRequestsResponse};
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

/// Pull-requests list query — accepts either `issue_id` (per-issue scope)
/// or `project_id` (project-scope, used by the kanban frontend to
/// populate the PR list across every visible issue at once). Exactly one
/// must be present; supplying both is rejected with 400. Defined
/// locally rather than reusing `api_types::ListPullRequestsQuery`
/// because the wire type's single-required-field shape doesn't model
/// the project-scoped variant.
#[derive(Debug, Deserialize)]
pub(super) struct ListPullRequestsQuery {
    #[serde(default)]
    pub issue_id: Option<Uuid>,
    #[serde(default)]
    pub project_id: Option<Uuid>,
}

/// Pull-request-issues junction list query — only `project_id` is
/// supported (the kanban frontend's only consumer is the
/// `PROJECT_PULL_REQUEST_ISSUES_SHAPE`, which loads every junction row
/// for the project at once).
#[derive(Debug, Deserialize)]
pub(super) struct ListPullRequestIssuesQuery {
    pub project_id: Uuid,
}

pub fn router() -> Router<DeploymentImpl> {
    Router::new()
        .route("/pull-requests", get(list_pull_requests))
        .route("/pull-requests/link", post(link_pr_to_issue))
        .route("/pull-request-issues", get(list_pull_request_issues))
}

async fn list_pull_requests(
    State(deployment): State<DeploymentImpl>,
    Query(query): Query<ListPullRequestsQuery>,
) -> Result<ResponseJson<ApiResponse<ListPullRequestsResponse>>, ApiError> {
    let pool = &deployment.db().pool;
    let pull_requests = match (query.issue_id, query.project_id) {
        (Some(issue_id), None) => {
            PullRequestIssueRepository::list_by_issue(pool, issue_id).await?
        }
        (None, Some(project_id)) => {
            PullRequestIssueRepository::list_by_project(pool, project_id).await?
        }
        (Some(_), Some(_)) => {
            return Err(ApiError::BadRequest(
                "issue_id and project_id are mutually exclusive".to_string(),
            ));
        }
        (None, None) => {
            return Err(ApiError::BadRequest(
                "issue_id or project_id is required".to_string(),
            ));
        }
    };
    Ok(ResponseJson(ApiResponse::success(
        ListPullRequestsResponse { pull_requests },
    )))
}

/// Lists `pull_request_issues` junction rows for the given project.
/// Backs `PROJECT_PULL_REQUEST_ISSUES_SHAPE` on the kanban side, which
/// reads `data["pull_request_issues"]` off the `ApiResponse` envelope
/// (see `extractRows` in `fetchShape.ts`) — this handler returns the
/// matching wire-shape `ListPullRequestIssuesResponse { pull_request_issues }`.
async fn list_pull_request_issues(
    State(deployment): State<DeploymentImpl>,
    Query(query): Query<ListPullRequestIssuesQuery>,
) -> Result<ResponseJson<ApiResponse<ListPullRequestIssuesResponse>>, ApiError> {
    let pool = &deployment.db().pool;
    let pull_request_issues =
        PullRequestIssueRepository::list_links_by_project(pool, query.project_id).await?;
    Ok(ResponseJson(ApiResponse::success(
        ListPullRequestIssuesResponse {
            pull_request_issues,
        },
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

#[cfg(test)]
mod tests {
    use api_types::{
        ListPullRequestIssuesResponse, ListPullRequestsResponse, PullRequest, PullRequestIssue,
    };
    use serde_json::json;
    use utils::response::ApiResponse;

    /// Both project- and issue-scoped pull-requests list responses must
    /// surface the table-keyed `pull_requests` field on the wire so
    /// `extractRows` (kanban side) can read `data["pull_requests"]` off
    /// the `ApiResponse` envelope.
    #[test]
    fn pull_requests_list_envelope_is_table_keyed() {
        let envelope: ApiResponse<ListPullRequestsResponse> =
            ApiResponse::success(ListPullRequestsResponse {
                pull_requests: Vec::<PullRequest>::new(),
            });
        let body = serde_json::to_value(&envelope).expect("serialize envelope");
        assert_eq!(
            body,
            json!({
                "success": true,
                "data": { "pull_requests": [] },
                "error_data": null,
                "message": null,
            }),
        );
    }

    /// `extractRows` reads `data["pull_request_issues"]` for the
    /// `pull_request_issues` shape — the wrapper must surface that
    /// field even when the project has no junction rows.
    #[test]
    fn pull_request_issues_list_envelope_is_table_keyed() {
        let envelope: ApiResponse<ListPullRequestIssuesResponse> =
            ApiResponse::success(ListPullRequestIssuesResponse {
                pull_request_issues: Vec::<PullRequestIssue>::new(),
            });
        let body = serde_json::to_value(&envelope).expect("serialize envelope");
        assert_eq!(
            body,
            json!({
                "success": true,
                "data": { "pull_request_issues": [] },
                "error_data": null,
                "message": null,
            }),
        );
    }
}
