use api_types::{
    CreateIssueFollowerRequest, DeleteResponse, IssueFollower, ListIssueFollowersResponse,
    MutationResponse,
};
use axum::{
    Router,
    extract::{Json, Path, Query, State},
    response::Json as ResponseJson,
    routing::get,
};
use db::models::issue_follower::IssueFollower as IssueFollowerRow;
use deployment::Deployment;
use serde::Deserialize;
use utils::response::ApiResponse;
use uuid::Uuid;

use crate::{DeploymentImpl, error::ApiError};

/// Followers list query — accepts either `issue_id` (per-issue scope) or
/// `project_id` (project-scope, used by the kanban frontend to populate
/// followers across every visible issue at once). Exactly one of the
/// two must be present; supplying both is rejected with 400.
#[derive(Debug, Deserialize)]
pub(super) struct ListIssueFollowersQuery {
    #[serde(default)]
    pub issue_id: Option<Uuid>,
    #[serde(default)]
    pub project_id: Option<Uuid>,
}

pub(super) fn router() -> Router<DeploymentImpl> {
    Router::new()
        .route(
            "/issue-followers",
            get(list_issue_followers).post(create_issue_follower),
        )
        .route(
            "/issue-followers/{issue_follower_id}",
            axum::routing::delete(delete_issue_follower),
        )
}

async fn list_issue_followers(
    State(deployment): State<DeploymentImpl>,
    Query(query): Query<ListIssueFollowersQuery>,
) -> Result<ResponseJson<ApiResponse<ListIssueFollowersResponse>>, ApiError> {
    let pool = &deployment.db().pool;
    let rows = match (query.issue_id, query.project_id) {
        (Some(issue_id), None) => IssueFollowerRow::find_by_issue(pool, issue_id).await?,
        (None, Some(project_id)) => IssueFollowerRow::find_by_project(pool, project_id).await?,
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
    let issue_followers: Vec<IssueFollower> = rows.into_iter().map(IssueFollower::from).collect();
    Ok(ResponseJson(ApiResponse::success(
        ListIssueFollowersResponse { issue_followers },
    )))
}

async fn create_issue_follower(
    State(deployment): State<DeploymentImpl>,
    Json(request): Json<CreateIssueFollowerRequest>,
) -> Result<ResponseJson<ApiResponse<MutationResponse<IssueFollower>>>, ApiError> {
    let pool = &deployment.db().pool;
    let id = request.id.unwrap_or_else(Uuid::new_v4);
    let response = IssueFollowerRow::create(pool, id, &request).await?;
    Ok(ResponseJson(ApiResponse::success(MutationResponse {
        data: response.data.into(),
        txid: response.txid,
    })))
}

async fn delete_issue_follower(
    State(deployment): State<DeploymentImpl>,
    Path(issue_follower_id): Path<Uuid>,
) -> Result<ResponseJson<ApiResponse<DeleteResponse>>, ApiError> {
    let pool = &deployment.db().pool;
    let response = IssueFollowerRow::delete(pool, issue_follower_id).await?;
    Ok(ResponseJson(ApiResponse::success(response)))
}

#[cfg(test)]
mod tests {
    use api_types::{DeleteResponse, IssueFollower, MutationResponse};
    use serde_json::json;
    use utils::response::ApiResponse;
    use uuid::Uuid;

    /// Mirrors the shape the kanban frontend reads back when a create
    /// succeeds — `data` is the wire row, `txid` is the mutation-log
    /// txid. Returning `ApiResponse<IssueFollower>` (without the
    /// `MutationResponse` wrapper) would silently drop `txid`.
    #[test]
    fn create_envelope_preserves_txid_on_the_wire() {
        let id = Uuid::nil();
        let envelope: ApiResponse<MutationResponse<IssueFollower>> =
            ApiResponse::success(MutationResponse {
                data: IssueFollower {
                    id,
                    issue_id: id,
                    user_id: id,
                },
                txid: 7,
            });
        let body = serde_json::to_value(&envelope).expect("serialize envelope");
        assert_eq!(
            body,
            json!({
                "success": true,
                "data": {
                    "data": {
                        "id": "00000000-0000-0000-0000-000000000000",
                        "issue_id": "00000000-0000-0000-0000-000000000000",
                        "user_id": "00000000-0000-0000-0000-000000000000",
                    },
                    "txid": 7,
                },
                "error_data": null,
                "message": null,
            }),
        );
    }

    #[test]
    fn delete_envelope_preserves_txid_on_the_wire() {
        let envelope: ApiResponse<DeleteResponse> =
            ApiResponse::success(DeleteResponse { txid: 11 });
        let body = serde_json::to_value(&envelope).expect("serialize envelope");
        assert_eq!(
            body,
            json!({
                "success": true,
                "data": { "txid": 11 },
                "error_data": null,
                "message": null,
            }),
        );
    }
}
