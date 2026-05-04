use api_types::{
    CreateIssueAssigneeRequest, DeleteResponse, IssueAssignee, ListIssueAssigneesResponse,
    MutationResponse,
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

use crate::{DeploymentImpl, error::ApiError};

/// Assignees list query — accepts either `issue_id` (per-issue scope) or
/// `project_id` (project-scope, used by the kanban frontend to populate
/// assignees across every visible issue at once). Exactly one of the
/// two must be present; supplying both is rejected with 400. Mirrors
/// `issue_followers::ListIssueFollowersQuery`.
#[derive(Debug, Deserialize)]
pub(super) struct ListIssueAssigneesQuery {
    #[serde(default)]
    pub issue_id: Option<Uuid>,
    #[serde(default)]
    pub project_id: Option<Uuid>,
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
    let rows = match (query.issue_id, query.project_id) {
        (Some(issue_id), None) => IssueAssigneeRow::find_by_issue(pool, issue_id).await?,
        (None, Some(project_id)) => IssueAssigneeRow::find_by_project(pool, project_id).await?,
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
    let response = IssueAssigneeRow::create(pool, id, &request).await?;
    Ok(ResponseJson(ApiResponse::success(MutationResponse {
        data: response.data.into(),
        txid: response.txid,
    })))
}

async fn delete_issue_assignee(
    State(deployment): State<DeploymentImpl>,
    Path(issue_assignee_id): Path<Uuid>,
) -> Result<ResponseJson<ApiResponse<DeleteResponse>>, ApiError> {
    let pool = &deployment.db().pool;
    let response = IssueAssigneeRow::delete(pool, issue_assignee_id).await?;
    Ok(ResponseJson(ApiResponse::success(response)))
}

#[cfg(test)]
mod tests {
    use api_types::{IssueAssignee, ListIssueAssigneesResponse};
    use serde_json::json;
    use utils::response::ApiResponse;

    /// `extractRows` on the kanban side reads `data["issue_assignees"]`
    /// off the `ApiResponse` envelope — the table-keyed wrapper must
    /// surface that field even when the project has no assignees, or the
    /// shape subscription would treat the empty result as a missing
    /// table and surface a fetch error.
    #[test]
    fn list_envelope_is_table_keyed() {
        let envelope: ApiResponse<ListIssueAssigneesResponse> =
            ApiResponse::success(ListIssueAssigneesResponse {
                issue_assignees: Vec::<IssueAssignee>::new(),
            });
        let body = serde_json::to_value(&envelope).expect("serialize envelope");
        assert_eq!(
            body,
            json!({
                "success": true,
                "data": { "issue_assignees": [] },
                "error_data": null,
                "message": null,
            }),
        );
    }
}
